use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

/// Wrapper around a SQLite connection for app-level persistence.
pub struct AppDb {
    conn: Mutex<Connection>,
}

/// Largest media cache entry stored (base64 bytes, ~12 MB of media).
const MAX_CACHED_ENTRY_BYTES: usize = 16 * 1024 * 1024;

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
        Ok(Self { conn: Mutex::new(conn) })
    }

    // ── Sessions ──

    /// Save the current session data to the database.
    pub fn save_session(&self, data: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO sessions (key, data) VALUES ('current', ?1)",
            params![data],
        )?;
        Ok(())
    }

    /// Load the saved session data, if any.
    pub fn load_session(&self) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT data FROM sessions WHERE key = 'current'",
            [],
            |row| row.get(0),
        ).ok()
    }

    /// Delete the saved session.
    pub fn delete_session(&self) {
        let conn = self.conn.lock().unwrap();
        if let Err(e) = conn.execute("DELETE FROM sessions WHERE key = 'current'", []) {
            tracing::warn!("Failed to delete session from database: {e}");
        }
    }

    // ── Media cache ──

    /// Store a media data URI in the SQLite cache.
    pub fn cache_media(&self, key: &str, data_uri: &str) {
        // Whole videos ended up here (80 MB+ rows); they're re-fetched on play.
        if data_uri.len() > MAX_CACHED_ENTRY_BYTES {
            return;
        }
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO media_cache (key, data, stamp) \
             VALUES (?1, ?2, (SELECT COALESCE(MAX(stamp), 0) + 1 FROM media_cache))",
            params![key, data_uri],
        );
    }

    /// Look up a cached media data URI by key.
    pub fn get_cached_media(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT data FROM media_cache WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    // ── Settings ──

    /// Store a key-value setting.
    pub fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Retrieve a setting value by key.
    pub fn get_setting(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).ok()
    }

    /// Drop a cached media entry (e.g. corrupt data from a failed fetch).
    pub fn remove_cached_media(&self, key: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM media_cache WHERE key = ?1", params![key]);
    }

    /// Keep the media cache bounded: drop everything but the `keep` most
    /// recently written entries, then reclaim the file space.
    /// Keep the newest `keep` media entries and at most `max_bytes` of
    /// them (entries are base64, often several MB). VACUUM only when a
    /// quarter of the file is free pages: it rewrites the whole database
    /// while holding the connection.
    pub fn prune_media_cache(&self, keep: usize, max_bytes: u64) {
        let conn = self.conn.lock().unwrap();
        let oversized = conn.execute(
            "DELETE FROM media_cache WHERE octet_length(data) > ?1",
            params![MAX_CACHED_ENTRY_BYTES as i64],
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

    #[test]
    fn media_cache_prunes_by_size_and_skips_huge_entries() {
        let dir = std::env::temp_dir().join(format!("fluxchat-db-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        let _ = std::fs::remove_file(&path);
        let db = AppDb::open(&path).unwrap();
        let mb = "x".repeat(1024 * 1024);
        for i in 0..10 {
            db.cache_media(&format!("k{i}"), &mb);
        }
        db.cache_media("huge", &"x".repeat(MAX_CACHED_ENTRY_BYTES + 1));
        assert!(db.get_cached_media("huge").is_none());
        // Keep 4 MB: the newest four entries survive.
        db.prune_media_cache(1000, 4 * 1024 * 1024);
        assert!(db.get_cached_media("k9").is_some() && db.get_cached_media("k6").is_some());
        assert!(db.get_cached_media("k5").is_none());
        db.prune_media_cache(2, u64::MAX);
        assert!(db.get_cached_media("k9").is_some() && db.get_cached_media("k7").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
