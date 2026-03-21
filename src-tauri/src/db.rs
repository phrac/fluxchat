use fluxdb::database::Database;
use serde_json::{json, Value};

/// Save a Matrix session to FluxDB.
/// We store exactly one session doc with a well-known _id = "current".
pub fn save_session(db: &Database, homeserver: &str, session_json: Value) -> anyhow::Result<()> {
    let doc = json!({
        "homeserver": homeserver,
        "session": session_json,
    });

    match db.get("sessions", "current") {
        Ok(_) => {
            db.update("sessions", "current", doc)?;
        }
        Err(_) => {
            let mut doc = doc;
            doc["_id"] = json!("current");
            db.insert("sessions", doc)?;
        }
    }
    Ok(())
}

/// Load the saved session from FluxDB. Returns None if no session exists.
pub fn load_session(db: &Database) -> Option<Value> {
    db.get("sessions", "current").ok()
}

/// Delete the saved session.
pub fn delete_session(db: &Database) {
    let _ = db.delete("sessions", "current");
}

/// Cache a media data URI keyed by MXC URI.
/// Uses a sanitized version of the MXC URI as the document ID.
pub fn cache_media(db: &Database, mxc_uri: &str, data_uri: &str) {
    let doc_id = sanitize_mxc_id(mxc_uri);
    let doc = json!({
        "_id": doc_id,
        "mxc": mxc_uri,
        "data": data_uri,
    });

    match db.get("media_cache", &doc_id) {
        Ok(_) => {} // Already cached
        Err(_) => {
            let _ = db.insert("media_cache", doc);
        }
    }
}

/// Look up a cached media data URI by MXC URI.
pub fn get_cached_media(db: &Database, mxc_uri: &str) -> Option<String> {
    let doc_id = sanitize_mxc_id(mxc_uri);
    db.get("media_cache", &doc_id)
        .ok()
        .and_then(|doc| doc["data"].as_str().map(|s| s.to_string()))
}

/// Sanitize MXC URI into a safe document ID.
/// "mxc://server.com/AbCdEf" -> "server.com_AbCdEf"
fn sanitize_mxc_id(mxc_uri: &str) -> String {
    mxc_uri
        .strip_prefix("mxc://")
        .unwrap_or(mxc_uri)
        .replace('/', "_")
}

/// Initialize FluxDB collections needed by the app.
pub fn init_collections(db: &Database) {
    let _ = db.create_collection("sessions");
    let _ = db.create_collection("settings");
    let _ = db.create_collection("media_cache");
}
