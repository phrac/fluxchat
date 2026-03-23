use matrix_sdk::Client;
use matrix_sdk_ui::sync_service::SyncService;
use tracing::info;

use crate::error::AppError;

/// Create and start the SyncService (uses Sliding Sync).
pub async fn create_sync_service(client: &Client) -> Result<SyncService, AppError> {
    let service = SyncService::builder(client.clone())
        .build()
        .await
        .map_err(|e| AppError::Other(format!("Failed to build SyncService: {e}")))?;

    info!("SyncService created, starting...");
    service.start().await;
    info!("SyncService started");

    Ok(service)
}
