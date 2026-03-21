use fluxdb::database::Database;
use matrix_sdk::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub client: Arc<Mutex<Option<Client>>>,
    pub db: Database,
    pub media_cache: std::sync::Mutex<HashMap<String, String>>,
}

impl AppState {
    pub fn new(db: Database) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            db,
            media_cache: std::sync::Mutex::new(HashMap::new()),
        }
    }
}
