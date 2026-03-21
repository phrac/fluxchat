use fluxdb::database::Database;
use serde_json::{json, Value};

/// Save a Matrix session to FluxDB.
/// We store exactly one session doc with a well-known _id = "current".
pub fn save_session(db: &Database, homeserver: &str, session_json: Value) -> anyhow::Result<()> {
    let doc = json!({
        "homeserver": homeserver,
        "session": session_json,
    });

    // Try to update existing; if not found, insert with known id
    match db.get("sessions", "current") {
        Ok(_) => {
            db.update("sessions", "current", doc)?;
        }
        Err(_) => {
            // Insert with custom _id by including it in the document
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

/// Initialize FluxDB collections needed by the app.
pub fn init_collections(db: &Database) {
    // create_collection is idempotent if it already exists (returns error which we ignore)
    let _ = db.create_collection("sessions");
    let _ = db.create_collection("settings");
}
