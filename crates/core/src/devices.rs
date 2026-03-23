use matrix_sdk::Client;
use matrix_sdk::ruma::api::client::device::Device;
use matrix_sdk::ruma::OwnedDeviceId;
use crate::error::AppError;

/// Info about a device/session, combining REST API + crypto verification status.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ts: Option<u64>,
    pub is_verified: bool,
    pub is_current: bool,
}

fn device_to_info(d: &Device, current_id: &str, verified_ids: &[String]) -> DeviceInfo {
    DeviceInfo {
        device_id: d.device_id.to_string(),
        display_name: d.display_name.clone(),
        last_seen_ip: d.last_seen_ip.clone(),
        last_seen_ts: d.last_seen_ts.map(|ts| ts.as_secs().into()),
        is_verified: verified_ids.contains(&d.device_id.to_string()),
        is_current: d.device_id.as_str() == current_id,
    }
}

/// List all devices for the current user, with verification status.
pub async fn list_devices(client: &Client) -> Result<Vec<DeviceInfo>, AppError> {
    let current_device_id = client.device_id()
        .ok_or(AppError::Other("No device ID".into()))?.to_string();
    let user_id = client.user_id()
        .ok_or(AppError::Other("No user ID".into()))?;

    // Get crypto verification status for all our devices
    let verified_ids: Vec<String> = if let Ok(user_devices) = client.encryption()
        .get_user_devices(user_id).await
    {
        user_devices.devices()
            .filter(|d| d.is_verified_with_cross_signing())
            .map(|d| d.device_id().to_string())
            .collect()
    } else {
        vec![]
    };

    // Get device list from REST API (has last_seen info)
    let response = client.devices().await
        .map_err(|e| AppError::Other(format!("Failed to list devices: {e}")))?;

    let mut devices: Vec<DeviceInfo> = response.devices.iter()
        .map(|d| device_to_info(d, &current_device_id, &verified_ids))
        .collect();

    // Sort: current first, then by last_seen descending
    devices.sort_by(|a, b| {
        b.is_current.cmp(&a.is_current)
            .then_with(|| b.last_seen_ts.cmp(&a.last_seen_ts))
    });

    Ok(devices)
}

/// Rename a device.
pub async fn rename_device(client: &Client, device_id: &str, name: &str) -> Result<(), AppError> {
    let id: OwnedDeviceId = device_id.into();
    client.rename_device(&id, name).await
        .map_err(|e| AppError::Other(format!("Failed to rename device: {e}")))?;
    Ok(())
}

/// Delete (sign out) devices. Requires password for UIAA.
pub async fn delete_devices(client: &Client, device_ids: &[String], password: &str) -> Result<(), AppError> {
    use matrix_sdk::ruma::api::client::uiaa;

    let ids: Vec<OwnedDeviceId> = device_ids.iter().map(|s| OwnedDeviceId::from(s.as_str())).collect();

    // First attempt — will fail with UIAA challenge
    let first_err = match client.delete_devices(&ids, None).await {
        Ok(_) => return Ok(()),
        Err(e) => e,
    };

    // Extract UIAA session from the error
    let uiaa_info = first_err.as_uiaa_response()
        .ok_or(AppError::Other(format!("Unexpected error: {first_err}")))?;

    let user_id = client.user_id()
        .ok_or(AppError::Other("No user ID".into()))?;

    let mut pw_auth = uiaa::Password::new(
        uiaa::UserIdentifier::UserIdOrLocalpart(user_id.localpart().to_string()),
        password.to_string(),
    );
    pw_auth.session = uiaa_info.session.clone();

    client.delete_devices(&ids, Some(uiaa::AuthData::Password(pw_auth))).await
        .map_err(|e| AppError::Other(format!("Failed to delete devices: {e}")))?;

    Ok(())
}
