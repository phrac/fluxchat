use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

/// Wrapper around a SQLite connection for app-level persistence.
pub struct AppDb {
    conn: Mutex<Connection>,
}

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
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO media_cache (key, data) VALUES (?1, ?2)",
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

    // ── Message cache ──

    /// Cache serialized room messages for offline access.
    pub fn cache_room_messages(&self, room_id: &str, messages_json: &str) {
        let _ = self.set_setting(&format!("msgs:{room_id}"), messages_json);
    }

    /// Load cached room messages, if available.
    pub fn get_cached_room_messages(&self, room_id: &str) -> Option<String> {
        self.get_setting(&format!("msgs:{room_id}"))
    }

    /// Remove cached messages for a room.
    pub fn clear_cached_room_messages(&self, room_id: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM settings WHERE key = ?1", params![format!("msgs:{room_id}")]);
    }
}
