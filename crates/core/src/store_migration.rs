//! One-time move from the unencrypted SDK store of older versions to an
//! encrypted one (see `vault`).
//!
//! matrix-sdk-sqlite can't add a key to an existing store, so the crypto
//! store is copied, through the SDK's own store API, into a new keyed store
//! in `matrix_store.migrating/`: this device's identity (account), the
//! cross-signing keys, room keys (with their backed-up flags), backup keys,
//! Olm sessions, known devices / identities, room encryption settings and
//! the to-device sync token. The state, event cache and media stores are
//! caches; they start empty and refill from the server. The sliding sync
//! position isn't copied, since the state it belongs to isn't.
//!
//! Crash safety, together with `auth::restore_session`: the vault (with the
//! key) is saved only after the copy finished, the directories are swapped
//! after that (`finish`), and the old session row in `fluxchat.db` is
//! removed last. Every step can be redone after an interruption.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use matrix_sdk_crypto::store::types::{Changes, DeviceChanges, IdentityChanges, PendingChanges};
use matrix_sdk_crypto::store::CryptoStore;
use matrix_sdk::SqliteCryptoStore;

use crate::error::AppError;

/// Room keys saved per transaction.
const BATCH: usize = 500;

fn migrating_dir(store_dir: &Path) -> PathBuf {
    store_dir.with_extension("migrating")
}

fn old_dir(store_dir: &Path) -> PathBuf {
    store_dir.with_extension("unencrypted-old")
}

fn err<E: std::fmt::Display>(what: &str) -> impl Fn(E) -> AppError + '_ {
    move |e| AppError::Other(format!("Encrypting the local store failed ({what}): {e}"))
}

/// Copy the unencrypted crypto store in `store_dir` into a new store
/// encrypted with `key`, in `matrix_store.migrating/`. Leaves `store_dir`
/// untouched; `finish` swaps them once the key is safely saved.
pub async fn copy_crypto_store(store_dir: &Path, key: &[u8; 32]) -> Result<(), AppError> {
    let target = migrating_dir(store_dir);
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(err("clean up"))?;
    }
    let old = SqliteCryptoStore::open(store_dir, None).await.map_err(err("open"))?;
    let new = SqliteCryptoStore::open_with_key(&target, Some(key)).await.map_err(err("open"))?;
    copy(&old, &new).await
}

async fn copy(old: &SqliteCryptoStore, new: &SqliteCryptoStore) -> Result<(), AppError> {
    let Some(account) = old.load_account().await.map_err(err("account"))? else {
        // Nothing was ever stored: the new store simply starts empty.
        return Ok(());
    };
    let own_user = account.user_id().to_owned();
    new.save_pending_changes(PendingChanges { account: Some(account) })
        .await.map_err(err("account"))?;

    let backup = old.load_backup_keys().await.map_err(err("backup keys"))?;
    let tracked = old.load_tracked_users().await.map_err(err("tracked users"))?;
    let mut users: BTreeSet<_> = tracked.iter().map(|t| t.user_id.clone()).collect();
    users.insert(own_user);

    // Devices, identities and the Olm sessions with those devices.
    let mut devices = Vec::new();
    let mut identities = Vec::new();
    let mut sessions = Vec::new();
    for user in &users {
        let user_devices = old.get_user_devices(user).await.map_err(err("devices"))?;
        for device in user_devices.into_values() {
            if let Some(curve) = device.curve25519_key() {
                if let Some(s) = old.get_sessions(&curve.to_base64()).await.map_err(err("sessions"))? {
                    sessions.extend(s);
                }
            }
            devices.push(device);
        }
        if let Some(identity) = old.get_user_identity(user).await.map_err(err("identities"))? {
            identities.push(identity);
        }
    }

    let room_keys = old.get_inbound_group_sessions().await.map_err(err("room keys"))?;
    let rooms: BTreeSet<_> = room_keys.iter().map(|s| s.room_id().to_owned()).collect();
    let mut room_settings = std::collections::HashMap::new();
    for room in rooms {
        if let Some(settings) = old.get_room_settings(&room).await.map_err(err("room settings"))? {
            room_settings.insert(room, settings);
        }
    }

    new.save_changes(Changes {
        private_identity: old.load_identity().await.map_err(err("cross-signing keys"))?,
        backup_version: backup.backup_version,
        backup_decryption_key: backup.decryption_key,
        dehydrated_device_pickle_key: old.load_dehydrated_device_pickle_key().await
            .map_err(err("dehydration key"))?,
        sessions,
        identities: IdentityChanges { new: identities, ..Default::default() },
        devices: DeviceChanges { new: devices, ..Default::default() },
        room_settings,
        next_batch_token: old.next_batch_token().await.map_err(err("sync token"))?,
        ..Default::default()
    }).await.map_err(err("keys"))?;

    let tracked: Vec<_> = tracked.iter().map(|t| (t.user_id.as_ref(), t.dirty)).collect();
    new.save_tracked_users(&tracked).await.map_err(err("tracked users"))?;

    let count = room_keys.len();
    let mut room_keys = room_keys;
    while !room_keys.is_empty() {
        let rest = room_keys.split_off(room_keys.len().min(BATCH));
        new.save_changes(Changes { inbound_group_sessions: room_keys, ..Default::default() })
            .await.map_err(err("room keys"))?;
        room_keys = rest;
    }
    tracing::info!("Copied the crypto store ({count} room keys, {} users) into an encrypted store", users.len());
    Ok(())
}

/// Whether a finished copy is waiting to replace the store.
pub fn pending(store_dir: &Path) -> bool {
    migrating_dir(store_dir).exists()
}

/// Drop an unfinished copy (its key was never saved).
pub fn discard(store_dir: &Path) {
    let _ = std::fs::remove_dir_all(migrating_dir(store_dir));
}

/// Put the encrypted copy in place of the old store and delete the old one.
/// Call only after the vault holding its key is saved; safe to repeat.
pub fn finish(store_dir: &Path) -> std::io::Result<()> {
    let target = migrating_dir(store_dir);
    let old = old_dir(store_dir);
    if target.exists() {
        if store_dir.exists() {
            if old.exists() {
                std::fs::remove_dir_all(&old)?;
            }
            std::fs::rename(store_dir, &old)?;
        }
        std::fs::rename(&target, store_dir)?;
    }
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk_crypto::olm::{Account, SenderData};
    use matrix_sdk_crypto::EncryptionSettings;
    use matrix_sdk::ruma::{device_id, room_id, user_id};

    #[tokio::test]
    async fn copies_identity_and_room_keys_into_an_encrypted_store() {
        // Unique per run: a pid alone can be reused, and a leftover store
        // from an earlier run would be opened instead of a fresh one.
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("fluxchat-migrate-test-{}-{nanos}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store_dir = dir.join("matrix_store");
        let key = [7u8; 32];

        let account = Account::with_device_id(user_id!("@a:example.org"), device_id!("DEV"));
        let identity = account.identity_keys();
        let (_, room_key) = account.create_group_session_pair(
            room_id!("!r:example.org"), EncryptionSettings::default(), SenderData::unknown(),
        ).await.unwrap();
        let session_id = room_key.session_id().to_owned();
        {
            let old = SqliteCryptoStore::open(&store_dir, None).await.unwrap();
            old.save_pending_changes(PendingChanges { account: Some(account) }).await.unwrap();
            old.save_changes(Changes { inbound_group_sessions: vec![room_key], ..Default::default() }).await.unwrap();
        }

        copy_crypto_store(&store_dir, &key).await.unwrap();
        assert!(pending(&store_dir));
        finish(&store_dir).unwrap();
        assert!(!pending(&store_dir) && !old_dir(&store_dir).exists());

        let new = SqliteCryptoStore::open_with_key(&store_dir, Some(&key)).await.unwrap();
        let account = new.load_account().await.unwrap().expect("account copied");
        assert_eq!(account.identity_keys(), identity);
        assert!(new.get_inbound_group_session(room_id!("!r:example.org"), &session_id).await.unwrap().is_some());
        drop(new);
        // Encrypted: the wrong key can't open it.
        assert!(SqliteCryptoStore::open_with_key(&store_dir, Some(&[8u8; 32])).await.is_err());

        // Best effort: SQLite may still be closing its files (WAL / shm) in
        // the background, and leftovers in the temp dir aren't what's tested.
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Run against a *copy* of a real store (it's migrated in place):
    /// `FLUXCHAT_MIGRATE_FROM=/path/to/copy/matrix_store cargo test -p fluxchat-core -- --ignored real_store`
    #[tokio::test]
    #[ignore = "needs FLUXCHAT_MIGRATE_FROM, a copy of a real matrix_store"]
    async fn migrates_a_real_store() {
        let src = std::path::PathBuf::from(std::env::var("FLUXCHAT_MIGRATE_FROM").unwrap());
        let key = [9u8; 32];
        let summary = |store: SqliteCryptoStore| async move {
            let account = store.load_account().await.unwrap().expect("account");
            let user = account.user_id().to_owned();
            (
                account.identity_keys(),
                store.get_inbound_group_sessions().await.unwrap().len(),
                store.load_tracked_users().await.unwrap().len(),
                store.load_backup_keys().await.unwrap().backup_version,
                store.load_identity().await.unwrap().is_some(),
                store.get_user_devices(&user).await.unwrap().len(),
                store.next_batch_token().await.unwrap().is_some(),
            )
        };
        let before = summary(SqliteCryptoStore::open(&src, None).await.unwrap()).await;
        copy_crypto_store(&src, &key).await.unwrap();
        finish(&src).unwrap();
        let after = summary(SqliteCryptoStore::open_with_key(&src, Some(&key)).await.unwrap()).await;
        println!("before: {before:?}\nafter:  {after:?}");
        assert_eq!(before, after);
    }
}
