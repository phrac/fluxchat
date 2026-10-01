//! Login session + store key, kept in the OS credential store (macOS
//! Keychain, Windows Credential Manager, Secret Service on Linux).
//!
//! The vault holds the access token and the 32-byte key the SDK store
//! (`matrix_store/`: crypto keys, sync state, message and media caches) and
//! the app's media cache are encrypted with. Without a usable credential
//! store it falls back to `secrets.json` in the (0700) data folder, and
//! `Protection::File` says so in Settings → Security.
//!
//! A credential store *error* is never treated as "no session": that would
//! show the login screen, and a new login replaces the store (and this
//! device's keys).

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "fluxchat";
const ACCOUNT: &str = "session";
const FALLBACK_FILE: &str = "secrets.json";

/// A saved login. Also the format of the pre-vault session row in
/// `fluxchat.db` (`AppDb::load_session`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SavedSession {
    pub homeserver: String,
    pub user_id: String,
    pub device_id: String,
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Vault {
    #[serde(flatten)]
    pub session: SavedSession,
    /// Base64 of the 32-byte store key.
    store_key: String,
}

/// Where the vault is kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protection {
    /// The OS credential store.
    Keyring,
    /// `secrets.json` in the data folder: no credential store was available.
    File,
}

impl Vault {
    /// A vault for a new login, with a fresh random store key.
    pub fn new(session: SavedSession) -> anyhow::Result<Self> {
        Ok(Self::with_key(session, &new_key()?))
    }

    pub fn with_key(session: SavedSession, key: &[u8; 32]) -> Self {
        Self { session, store_key: base64::engine::general_purpose::STANDARD.encode(key) }
    }

    pub fn store_key(&self) -> anyhow::Result<[u8; 32]> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.store_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("store key has the wrong length"))
    }
}

/// A random 32-byte store key.
pub fn new_key() -> anyhow::Result<[u8; 32]> {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
    Ok(key)
}

fn entry() -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, ACCOUNT)
}

fn fallback_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FALLBACK_FILE)
}

fn parse(json: &str) -> anyhow::Result<Vault> {
    serde_json::from_str(json).map_err(|e| anyhow::anyhow!("saved session is unreadable: {e}"))
}

fn read_file(data_dir: &Path) -> anyhow::Result<Option<Vault>> {
    match std::fs::read_to_string(fallback_path(data_dir)) {
        Ok(json) => parse(&json).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Load the vault. `Ok(None)` only when there definitely is none. Blocking
/// (the credential store may prompt): call from `spawn_blocking`.
pub fn load(data_dir: &Path) -> anyhow::Result<Option<(Vault, Protection)>> {
    let keyring_err = match entry().and_then(|e| e.get_password()) {
        Ok(json) => return parse(&json).map(|v| Some((v, Protection::Keyring))),
        Err(keyring::Error::NoEntry) => None,
        Err(e) => Some(e),
    };
    match (read_file(data_dir)?, keyring_err) {
        (Some(vault), _) => Ok(Some((vault, Protection::File))),
        (None, None) => Ok(None),
        (None, Some(e)) => Err(anyhow::anyhow!("couldn't read the saved session from the system keyring: {e}")),
    }
}

/// Save the vault, in the credential store when possible. Blocking.
pub fn save(data_dir: &Path, vault: &Vault) -> anyhow::Result<Protection> {
    let json = serde_json::to_string(vault)?;
    match save_keyring(&json) {
        Ok(()) => {
            let _ = std::fs::remove_file(fallback_path(data_dir));
            Ok(Protection::Keyring)
        }
        Err(e) => {
            tracing::warn!("System keyring unavailable ({e}); keeping the session key in the data folder");
            write_file(data_dir, &json)?;
            Ok(Protection::File)
        }
    }
}

fn save_keyring(json: &str) -> anyhow::Result<()> {
    entry()?.set_password(json)?;
    // Read it back through a new entry: a store that accepts writes but
    // doesn't keep them (keyring's mock store on unsupported platforms)
    // must not count.
    if entry()?.get_password()? != json {
        anyhow::bail!("the keyring didn't keep the session");
    }
    Ok(())
}

fn write_file(data_dir: &Path, json: &str) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!("{FALLBACK_FILE}.tmp"));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(json.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, fallback_path(data_dir))?;
    Ok(())
}

/// Forget the session everywhere. Blocking.
pub fn delete(data_dir: &Path) {
    match entry().and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(e) => tracing::warn!("Couldn't remove the session from the system keyring: {e}"),
    }
    let _ = std::fs::remove_file(fallback_path(data_dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SavedSession {
        SavedSession {
            homeserver: "https://example.org".into(),
            user_id: "@a:example.org".into(),
            device_id: "DEV".into(),
            access_token: "syt_secret".into(),
            refresh_token: None,
        }
    }

    #[test]
    fn vault_roundtrips_and_reads_legacy_rows() {
        let vault = Vault::new(session()).unwrap();
        let key = vault.store_key().unwrap();
        let back = parse(&serde_json::to_string(&vault).unwrap()).unwrap();
        assert_eq!(back, vault);
        assert_eq!(back.store_key().unwrap(), key);
        // The old session row in fluxchat.db is a SavedSession.
        let legacy = r#"{"homeserver":"https://example.org","user_id":"@a:example.org","device_id":"DEV","access_token":"syt_secret","refresh_token":null}"#;
        assert_eq!(serde_json::from_str::<SavedSession>(legacy).unwrap(), session());
    }

    #[test]
    fn fallback_file_is_private() {
        let dir = std::env::temp_dir().join(format!("fluxchat-vault-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let vault = Vault::new(session()).unwrap();
        write_file(&dir, &serde_json::to_string(&vault).unwrap()).unwrap();
        assert_eq!(read_file(&dir).unwrap(), Some(vault));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(fallback_path(&dir)).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
