use matrix_sdk_store_encryption::{EncryptedValueBase64, StoreCipher};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::{Mutex, RwLock};

/// Wrapper around a SQLite connection for app-level persistence.
pub struct AppDb {
    conn: Mutex<Connection>,
    /// Encrypts the media cache (it holds decrypted media from encrypted
    /// rooms). Set from the vault's store key at login / restore
    /// (`unlock_media_cache`); while unset nothing is cached.
    media_cipher: RwLock<Option<StoreCipher>>,
}

/// Largest media cache entry stored (bytes of media, before encryption).
const MAX_CACHED_ENTRY_BYTES: usize = 12 * 1024 * 1024;
/// Largest stored row: the encrypted entry, base64 inside JSON.
const MAX_CACHED_ROW_BYTES: usize = MAX_CACHED_ENTRY_BYTES / 3 * 4 + 1024;
/// Settings key of the media cache cipher, exported with the store key.
const MEDIA_CIPHER_SETTING: &str = "media_cipher";
/// `hash_key` table name for media cache keys.
const MEDIA_TABLE: &str = "media_cache";

impl AppDb {
    /// Open or create the database at the given path.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                key TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS media_cache (
                key TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS private_data (
                key TEXT PRIMARY KEY,
                data TEXT NOT NULL
            );"
        )?;
        // Recency for pruning: rowids can't be trusted for that (VACUUM may
        // renumber them in a table without an INTEGER PRIMARY KEY).
        let has_stamp = |c: &Connection| c.prepare("SELECT stamp FROM media_cache LIMIT 0").is_ok();
        if !has_stamp(&conn) {
            // Another instance may add it at the same moment: fine if it's there now.
            if let Err(e) = conn.execute_batch("ALTER TABLE media_cache ADD COLUMN stamp INTEGER NOT NULL DEFAULT 0") {
                if !has_stamp(&conn) {
                    return Err(e.into());
                }
            }
        }
        // stamp sits after the (large) data column: without an index, MAX()
        // and ORDER BY would read every entry's overflow pages.
        conn.execute_batch("CREATE INDEX IF NOT EXISTS media_cache_stamp ON media_cache(stamp)")?;
        Ok(Self { conn: Mutex::new(conn), media_cipher: RwLock::new(None) })
    }

    // ── Sessions ──

    /// The session row older versions saved (migrated into `vault`).
    pub fn load_session(&self) -> Option<String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            "SELECT data FROM sessions WHERE key = 'current'",
            [],
            |row| row.get(0),
        ).ok()
    }

    /// Delete the saved session (older versions kept it here, token
    /// included; it now lives in `vault`). Overwritten on disk, not just
    /// unlinked.
    pub fn delete_session(&self) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute_batch("PRAGMA secure_delete = ON");
        if let Err(e) = conn.execute("DELETE FROM sessions WHERE key = 'current'", []) {
            tracing::warn!("Failed to delete session from database: {e}");
        }
        let _ = conn.execute_batch("PRAGMA secure_delete = OFF");
    }

    // ── Media cache ──

    /// Start using the media cache, encrypted with a cipher kept (exported
    /// with `store_key`) in the settings. Entries from before encryption,
    /// or under another key (a new login), are dropped.
    pub fn unlock_media_cache(&self, store_key: &[u8; 32]) -> anyhow::Result<()> {
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let existing = self.get_setting(MEDIA_CIPHER_SETTING)
            .and_then(|b64| engine.decode(b64).ok())
            .and_then(|blob| StoreCipher::import_with_key(store_key, &blob).ok());
        let cipher = match existing {
            Some(cipher) => cipher,
            None => {
                let cipher = StoreCipher::new()?;
                let export = cipher.export_with_key(store_key)?;
                let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
                // Private data under another login's key can't be read.
                conn.execute("DELETE FROM private_data", [])?;
                // Plaintext media from before (or another login's): gone,
                // including the freed pages.
                if conn.execute("DELETE FROM media_cache", [])? > 0 {
                    if let Err(e) = conn.execute_batch("VACUUM") {
                        tracing::warn!("VACUUM failed: {e}");
                    }
                }
                conn.execute(
                    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                    params![MEDIA_CIPHER_SETTING, engine.encode(export)],
                )?;
                cipher
            }
        };
        *self.media_cipher.write().unwrap_or_else(|e| e.into_inner()) = Some(cipher);
        Ok(())
    }

    /// Stop caching and delete the cache and its cipher (logout).
    pub fn forget_media_cache(&self) {
        *self.media_cipher.write().unwrap_or_else(|e| e.into_inner()) = None;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute("DELETE FROM media_cache", []);
        let _ = conn.execute("DELETE FROM private_data", []);
        let _ = conn.execute("DELETE FROM settings WHERE key = ?1", params![MEDIA_CIPHER_SETTING]);
    }

    // ── Private data ──
    //
    // Small values that hold what you wrote (prompt history, snippets, agent
    // run history): encrypted with the media cache's cipher, keys hashed,
    // never pruned. Unavailable (None / not stored) until the cache is
    // unlocked at sign-in; cleared at logout and when the key changes.

    /// Store a private value; false while locked.
    pub fn set_private(&self, key: &str, data: &[u8]) -> bool {
        let row = {
            let cipher = self.media_cipher.read().unwrap_or_else(|e| e.into_inner());
            let Some(cipher) = cipher.as_ref() else { return false };
            let Ok(value) = cipher.encrypt_value_base64_data(data.to_vec()) else { return false };
            let Ok(json) = serde_json::to_string(&value) else { return false };
            (cipher.hash_key("private_data", key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect::<String>(), json)
        };
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute("INSERT OR REPLACE INTO private_data (key, data) VALUES (?1, ?2)", params![row.0, row.1]).is_ok()
    }

    /// Read a private value (None while locked or unset).
    pub fn get_private(&self, key: &str) -> Option<Vec<u8>> {
        let cipher = self.media_cipher.read().unwrap_or_else(|e| e.into_inner());
        let cipher = cipher.as_ref()?;
        let row_key: String = cipher.hash_key("private_data", key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let json: String = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            conn.query_row("SELECT data FROM private_data WHERE key = ?1", params![row_key], |row| row.get(0)).ok()?
        };
        let value: EncryptedValueBase64 = serde_json::from_str(&json).ok()?;
        cipher.decrypt_value_base64_data(value).ok()
    }

    /// Delete a private value.
    pub fn remove_private(&self, key: &str) {
        let row_key: String = {
            let cipher = self.media_cipher.read().unwrap_or_else(|e| e.into_inner());
            let Some(cipher) = cipher.as_ref() else { return };
            cipher.hash_key("private_data", key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
        };
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute("DELETE FROM private_data WHERE key = ?1", params![row_key]);
    }

    /// Delete a plaintext setting (after moving it into private data).
    pub fn remove_setting(&self, key: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute("DELETE FROM settings WHERE key = ?1", params![key]);
    }

    fn media_row_key(cipher: &StoreCipher, key: &str) -> String {
        cipher.hash_key(MEDIA_TABLE, key.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Store media bytes in the (encrypted) SQLite cache.
    pub fn cache_media(&self, key: &str, data: &[u8]) {
        // Whole videos ended up here (80 MB+ rows); they're re-fetched on play.
        if data.len() > MAX_CACHED_ENTRY_BYTES {
            return;
        }
        let row = {
            let cipher = self.media_cipher.read().unwrap_or_else(|e| e.into_inner());
            let Some(cipher) = cipher.as_ref() else { return };
            let Ok(value) = cipher.encrypt_value_base64_data(data.to_vec()) else { return };
            let Ok(json) = serde_json::to_string(&value) else { return };
            (Self::media_row_key(cipher, key), json)
        };
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute(
            "INSERT OR REPLACE INTO media_cache (key, data, stamp) \
             VALUES (?1, ?2, (SELECT COALESCE(MAX(stamp), 0) + 1 FROM media_cache))",
            params![row.0, row.1],
        );
    }

    /// Look up cached media bytes by key.
    pub fn get_cached_media(&self, key: &str) -> Option<Vec<u8>> {
        let cipher = self.media_cipher.read().unwrap_or_else(|e| e.into_inner());
        let cipher = cipher.as_ref()?;
        let row_key = Self::media_row_key(cipher, key);
        let json: String = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            conn.query_row("SELECT data FROM media_cache WHERE key = ?1", params![row_key], |row| row.get(0)).ok()?
        };
        let value: EncryptedValueBase64 = serde_json::from_str(&json).ok()?;
        cipher.decrypt_value_base64_data(value).ok()
    }

    // ── Settings ──

    /// Store a key-value setting.
    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Retrieve a setting value by key.
    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    /// Drop a cached media entry (e.g. corrupt data from a failed fetch).
    pub fn remove_cached_media(&self, key: &str) {
        let cipher = self.media_cipher.read().unwrap_or_else(|e| e.into_inner());
        let Some(cipher) = cipher.as_ref() else { return };
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let _ = conn.execute("DELETE FROM media_cache WHERE key = ?1", params![Self::media_row_key(cipher, key)]);
    }

    /// Keep the media cache bounded: drop everything but the `keep` most
    /// recently written entries, then reclaim the file space.
    /// Keep the newest `keep` media entries and at most `max_bytes` of
    /// them (entries are base64, often several MB). VACUUM only when a
    /// quarter of the file is free pages: it rewrites the whole database
    /// while holding the connection.
    pub fn prune_media_cache(&self, keep: usize, max_bytes: u64) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let oversized = conn.execute(
            "DELETE FROM media_cache WHERE octet_length(data) > ?1",
            params![MAX_CACHED_ROW_BYTES as i64],
        ).unwrap_or(0);
        let by_count = conn.execute(
            "DELETE FROM media_cache WHERE key NOT IN \
             (SELECT key FROM media_cache ORDER BY stamp DESC LIMIT ?1)",
            params![keep as i64],
        ).unwrap_or(0);
        let by_size = conn.execute(
            "DELETE FROM media_cache WHERE key IN (\
               SELECT key FROM (\
                 SELECT key, SUM(octet_length(data)) OVER (ORDER BY stamp DESC, key) AS total FROM media_cache\
               ) WHERE total > ?1)",
            // Saturate: `u64::MAX as i64` is -1, which would delete everything.
            params![i64::try_from(max_bytes).unwrap_or(i64::MAX)],
        ).unwrap_or(0);
        // Message snapshots from an old version were stored under settings.
        let removed_msgs = conn.execute("DELETE FROM settings WHERE key LIKE 'msgs:%'", []).unwrap_or(0);
        if oversized + by_count + by_size + removed_msgs > 0 {
            tracing::info!("Pruned {} media cache entries, {removed_msgs} stale message caches", oversized + by_count + by_size);
        }
        let pages = |pragma: &str| conn.query_row(pragma, [], |r| r.get::<_, i64>(0)).unwrap_or(0);
        let (total, free) = (pages("PRAGMA page_count"), pages("PRAGMA freelist_count"));
        if total > 0 && free * 4 > total {
            tracing::info!("Compacting app database ({free} of {total} pages free)");
            if let Err(e) = conn.execute_batch("VACUUM") {
                tracing::warn!("VACUUM failed: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> (AppDb, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("fluxchat-db-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (AppDb::open(&dir.join("t.db")).unwrap(), dir)
    }

    #[test]
    fn media_cache_prunes_by_size_and_skips_huge_entries() {
        let (db, dir) = temp_db("prune");
        db.unlock_media_cache(&[1; 32]).unwrap();
        let mb = vec![b'x'; 1024 * 1024];
        for i in 0..10 {
            db.cache_media(&format!("k{i}"), &mb);
        }
        db.cache_media("huge", &vec![b'x'; MAX_CACHED_ENTRY_BYTES + 1]);
        assert!(db.get_cached_media("huge").is_none());
        // Rows are ~1.33 MB (base64): 6 MB keeps the newest four.
        db.prune_media_cache(1000, 6 * 1024 * 1024);
        assert!(db.get_cached_media("k9").is_some() && db.get_cached_media("k6").is_some());
        assert!(db.get_cached_media("k5").is_none());
        db.prune_media_cache(2, u64::MAX);
        assert!(db.get_cached_media("k9").is_some() && db.get_cached_media("k7").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn media_cache_is_encrypted_and_tied_to_the_store_key() {
        let (db, dir) = temp_db("crypt");
        let secret = b"decrypted image from an encrypted room".to_vec();
        // Locked: nothing is written.
        db.cache_media("mxc://a/b", &secret);
        assert!(db.get_cached_media("mxc://a/b").is_none());

        db.unlock_media_cache(&[1; 32]).unwrap();
        db.cache_media("mxc://a/b", &secret);
        assert_eq!(db.get_cached_media("mxc://a/b"), Some(secret.clone()));
        // Neither the bytes nor the key are stored in the clear.
        let raw: (String, String) = db.conn.lock().unwrap()
            .query_row("SELECT key, data FROM media_cache", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert!(!raw.0.contains("mxc") && !raw.1.contains("decrypted"));

        // Same key (restart): still readable. Another key (new login): dropped.
        let again = AppDb::open(&dir.join("t.db")).unwrap();
        again.unlock_media_cache(&[1; 32]).unwrap();
        assert_eq!(again.get_cached_media("mxc://a/b"), Some(secret));
        again.unlock_media_cache(&[2; 32]).unwrap();
        assert!(again.get_cached_media("mxc://a/b").is_none());
        again.forget_media_cache();
        assert!(again.get_setting(MEDIA_CIPHER_SETTING).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn private_data_is_encrypted_and_needs_the_key() {
        let dir = std::env::temp_dir().join(format!("fluxchat-private-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
        let path = dir.join("t.db");
        let db = AppDb::open(&path).unwrap();
        assert!(!db.set_private("prompts", b"secret plan"), "locked: nothing stored");
        db.unlock_media_cache(&[1; 32]).unwrap();
        assert!(db.set_private("prompts", b"secret plan"));
        assert_eq!(db.get_private("prompts").as_deref(), Some(&b"secret plan"[..]));
        drop(db);
        // Not readable on disk.
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(11).any(|w| w == b"secret plan"));
        // Same key: still there; another key (new login): gone.
        let again = AppDb::open(&path).unwrap();
        again.unlock_media_cache(&[1; 32]).unwrap();
        assert!(again.get_private("prompts").is_some());
        again.forget_media_cache();
        again.unlock_media_cache(&[2; 32]).unwrap();
        assert!(again.get_private("prompts").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
