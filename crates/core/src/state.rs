use matrix_sdk::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::AppDb;

pub struct AppState {
    pub client: Arc<Mutex<Option<Client>>>,
    pub db: AppDb,
    /// URL previews (`preview::get_url_preview`), as JSON by URL.
    pub preview_cache: std::sync::Mutex<HashMap<String, String>>,
    pub timelines: crate::timeline::RoomTimelines,
    pub sync_service: Mutex<Option<matrix_sdk_ui::sync_service::SyncService>>,
}

impl AppState {
    pub fn new(db: AppDb) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            db,
            preview_cache: std::sync::Mutex::new(HashMap::new()),
            timelines: Default::default(),
            sync_service: Mutex::new(None),
        }
    }

    pub async fn get_client(&self) -> Result<Client, crate::error::AppError> {
        let lock = self.client.lock().await;
        lock.as_ref().cloned().ok_or(crate::error::AppError::NotLoggedIn)
    }

    pub async fn set_client(&self, client: Client) {
        let mut lock = self.client.lock().await;
        *lock = Some(client);
    }

    pub async fn clear_client(&self) {
        let mut lock = self.client.lock().await;
        *lock = None;
    }
}
