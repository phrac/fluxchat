use matrix_sdk::{
    authentication::{matrix::MatrixSession, SessionTokens},
    ruma::OwnedUserId,
    Client, SessionMeta,
};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::LoginResult;
use crate::state::AppState;
use crate::vault::{self, Protection, SavedSession, Vault};

fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("fluxchat")
}

fn store_dir() -> PathBuf {
    data_dir().join("matrix_store")
}

/// A client on the SDK store in `matrix_store/`, encrypted with `key`.
async fn build_client(homeserver: &str, key: &[u8; 32]) -> Result<Client, AppError> {
    Ok(Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store_with_config_and_cache_path(
            matrix_sdk::SqliteStoreConfig::new(store_dir()).key(Some(key)),
            None::<PathBuf>,
        )
        // Without this the event cache never routes live sync events into
        // thread caches, so open thread views stop updating (and thread
        // summaries never refresh).
        .with_threading_support(matrix_sdk::ThreadingSupport::Enabled { with_subscriptions: false })
        .build()
        .await?)
}

/// Run a blocking vault call (the credential store may show a prompt).
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f).await
        .map_err(|e| AppError::Other(e.to_string()))?
        .map_err(|e| AppError::Other(e.to_string()))
}

/// Start a fresh store for a new login: the old one belongs to another
/// session (or none), and its key is about to be replaced.
fn clear_store() {
    let dir = store_dir();
    if dir.exists() {
        info!("Clearing old matrix store for fresh login");
        let _ = std::fs::remove_dir_all(&dir);
    }
    crate::store_migration::discard(&dir);
}

/// After a login: keep the session and store key in the vault, start the
/// encrypted media cache, and finish setting up the client.
async fn complete_login(
    state: &AppState,
    client: Client,
    homeserver: &str,
    key: [u8; 32],
) -> Result<(Client, LoginResult), AppError> {
    let user_id = client.user_id()
        .ok_or(AppError::Other("No user ID after login".into()))?.to_string();
    let session = client.matrix_auth().session()
        .ok_or(AppError::Other("No session after login".into()))?;
    let vault = Vault::with_key(SavedSession {
        homeserver: homeserver.to_string(),
        user_id: session.meta.user_id.to_string(),
        device_id: session.meta.device_id.to_string(),
        access_token: session.tokens.access_token,
        refresh_token: session.tokens.refresh_token,
    }, &key);
    let dir = data_dir();
    let protection = blocking(move || vault::save(&dir, &vault)).await?;
    state.db.delete_session();
    info!("Session saved for {user_id} ({protection:?})");
    if let Err(e) = state.db.unlock_media_cache(&key) {
        warn!("Media cache unavailable: {e}");
    }
    start_client(state, client, user_id, protection).await
}

async fn start_client(
    state: &AppState,
    client: Client,
    user_id: String,
    protection: Protection,
) -> Result<(Client, LoginResult), AppError> {
    // Enable event cache for Timeline API
    if let Err(e) = client.event_cache().subscribe() {
        warn!("Failed to subscribe event cache: {e}");
    }
    let display_name = client.account().get_display_name().await.ok().flatten();
    state.set_client(client.clone()).await;
    Ok((client, LoginResult { user_id, display_name, protection }))
}

/// Log in with username and password, saving the session to the database.
pub async fn login(
    state: &AppState,
    homeserver: &str,
    username: &str,
    password: &str,
) -> Result<(Client, LoginResult), AppError> {
    clear_store();
    let key = vault::new_key().map_err(|e| AppError::Other(e.to_string()))?;
    let client = build_client(homeserver, &key).await?;

    client.matrix_auth()
        .login_username(username, password)
        .initial_device_display_name("FluxChat")
        .await?;

    complete_login(state, client, homeserver, key).await
}

/// Log out, clearing the saved session and matrix store.
pub async fn logout(state: &AppState) -> Result<(), AppError> {
    // Stop syncing first, or the reconnect watcher would try to restart a
    // client whose token is about to be invalidated.
    if let Some(service) = state.sync_service.lock().await.take() {
        service.stop().await;
    }
    state.timelines.clear().await;
    let mut lock = state.client.lock().await;
    if let Some(client) = lock.take() {
        let _ = client.logout().await;
    }
    let dir = data_dir();
    let _ = tokio::task::spawn_blocking(move || vault::delete(&dir)).await;
    state.db.delete_session();
    state.db.forget_media_cache();
    clear_store();
    Ok(())
}

/// SSO identity provider info.
#[derive(Clone, Debug)]
pub struct SsoProvider {
    pub id: String,
    pub name: String,
    pub brand: Option<String>,
}

/// Login capabilities discovered from the homeserver.
#[derive(Clone, Debug, Default)]
pub struct LoginCapabilities {
    pub has_password: bool,
    pub has_sso: bool,
    pub sso_providers: Vec<SsoProvider>,
}

/// Discover login types supported by a homeserver.
pub async fn get_login_capabilities(homeserver: &str) -> Result<LoginCapabilities, AppError> {
    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .build()
        .await?;

    let response = client.matrix_auth().get_login_types().await
        .map_err(|e| AppError::Other(format!("Failed to get login types: {e}")))?;

    use matrix_sdk::ruma::api::client::session::get_login_types::v3::LoginType;
    let mut caps = LoginCapabilities::default();
    for flow in &response.flows {
        match flow {
            LoginType::Password(_) => caps.has_password = true,
            LoginType::Sso(sso) => {
                caps.has_sso = true;
                for provider in &sso.identity_providers {
                    caps.sso_providers.push(SsoProvider {
                        id: provider.id.clone(),
                        name: provider.name.clone(),
                        brand: provider.brand.as_ref().map(|b| format!("{b:?}").to_lowercase()),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(caps)
}

/// SSO login — opens browser, waits for callback, completes login.
pub async fn login_sso(
    state: &AppState,
    homeserver: &str,
) -> Result<(Client, LoginResult), AppError> {
    clear_store();
    let key = vault::new_key().map_err(|e| AppError::Other(e.to_string()))?;
    let client = build_client(homeserver, &key).await?;

    client.matrix_auth()
        .login_sso(|url| async move {
            // Open the SSO URL in the system browser
            if let Err(e) = open::that(&url) {
                warn!("Failed to open browser: {e}");
            }
            Ok(())
        })
        .initial_device_display_name("FluxChat")
        .await
        .map_err(|e| AppError::Other(format!("SSO login failed: {e}")))?;

    complete_login(state, client, homeserver, key).await
}

/// Restore the saved session: from the vault, or once from the session
/// row older versions kept in `fluxchat.db`, encrypting their store
/// (`store_migration`).
pub async fn restore_session(state: &AppState) -> Result<Option<(Client, LoginResult)>, AppError> {
    let dir = data_dir();
    let store = store_dir();
    let vault = {
        let dir = dir.clone();
        blocking(move || vault::load(&dir)).await?
    };
    let legacy = state.db.load_session()
        .and_then(|json| serde_json::from_str::<SavedSession>(&json).ok());

    // A pre-vault session with no vault yet, or newer than the vault's (an
    // older version signed in again): encrypt its store, then keep it in
    // the vault. Same device: an interrupted migration that got this far.
    let migrate = legacy.filter(|l| vault.as_ref().is_none_or(|(v, _)| v.session.device_id != l.device_id));
    let (vault, protection) = match (vault, migrate) {
        (_, Some(legacy)) => {
            info!("Encrypting the local store of {} (device {})", legacy.user_id, legacy.device_id);
            let key = vault::new_key().map_err(|e| AppError::Other(e.to_string()))?;
            crate::store_migration::copy_crypto_store(&store, &key).await?;
            let vault = Vault::with_key(legacy, &key);
            let protection = {
                let (dir, vault) = (dir.clone(), vault.clone());
                blocking(move || vault::save(&dir, &vault)).await?
            };
            (vault, protection)
        }
        (Some(found), None) => found,
        (None, None) => {
            // A copy whose key was never saved can't be used.
            crate::store_migration::discard(&store);
            info!("No saved session");
            return Ok(None);
        }
    };
    // Swap in an encrypted copy (also after an interrupted migration), then
    // drop the plaintext session row.
    crate::store_migration::finish(&store)
        .map_err(|e| AppError::Other(format!("Couldn't replace the local store: {e}")))?;
    state.db.delete_session();

    let session = &vault.session;
    let user_id: OwnedUserId = session.user_id.as_str().try_into()
        .map_err(|_| AppError::Other("Invalid user_id".into()))?;
    let device_id: matrix_sdk::ruma::OwnedDeviceId = session.device_id.as_str().into();
    let key = vault.store_key().map_err(|e| AppError::Other(e.to_string()))?;

    info!("Restoring session for {} (device {}, {protection:?})", user_id, device_id);
    let client = build_client(&session.homeserver, &key).await?;

    let matrix_session = MatrixSession {
        meta: SessionMeta { user_id: user_id.clone(), device_id },
        tokens: SessionTokens {
            access_token: session.access_token.clone(),
            refresh_token: session.refresh_token.clone(),
        },
    };

    // Never delete the session or crypto store here: restore can fail for
    // transient reasons (e.g. another FluxChat instance holding the store
    // lock), and wiping would destroy this device's keys. Logging in again
    // replaces the store explicitly; an invalid token is handled by sync.
    client.restore_session(matrix_session).await
        .map_err(|e| AppError::Other(format!("Session restore failed: {e}")))?;
    info!("Session restored");
    if let Err(e) = state.db.unlock_media_cache(&key) {
        warn!("Media cache unavailable: {e}");
    }
    start_client(state, client, user_id.to_string(), protection).await.map(Some)
}
