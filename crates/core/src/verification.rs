use crate::error::AppError;
use crate::models::{VerificationInfo, EmojiInfo};
use futures_util::StreamExt;
use matrix_sdk::encryption::verification::VerificationRequestState;

/// Request interactive (SAS) verification with a user.
/// Returns the flow_id so the UI can track the verification.
pub async fn request_verification(
    client: &matrix_sdk::Client,
    user_id: &str,
    on_emojis: impl Fn(VerificationInfo) + Send + 'static,
    on_cancelled: impl Fn() + Send + 'static,
) -> Result<String, AppError> {
    let uid = matrix_sdk::ruma::UserId::parse(user_id)
        .map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))?;
    let identity = client.encryption().get_user_identity(&uid).await
        .map_err(|e| AppError::Other(format!("Failed to get identity: {e}")))?
        .ok_or_else(|| AppError::Other(format!("No identity found for {user_id}")))?;

    let request = identity.request_verification().await
        .map_err(|e| AppError::Other(format!("Failed to request verification: {e}")))?;

    let flow_id = request.flow_id().to_string();
    let other_user = user_id.to_string();
    let flow_id_ret = flow_id.clone();

    // Spawn a task to drive the verification through state transitions
    tokio::spawn(async move {
        let mut stream = request.changes();
        while let Some(state) = stream.next().await {
            match state {
                VerificationRequestState::Ready { .. } => {
                    tracing::info!("Outgoing verification request Ready, starting SAS...");
                    match request.start_sas().await {
                        Ok(Some(sas)) => {
                            for _ in 0..60 {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                if sas.can_be_presented() {
                                    if let Some(emojis) = sas.emoji() {
                                        on_emojis(VerificationInfo {
                                            flow_id: flow_id.clone(),
                                            other_user_id: other_user.clone(),
                                            is_self_verification: false,
                                            state: "emojis_ready".into(),
                                            emojis: Some(emojis.iter().map(|e| EmojiInfo {
                                                symbol: e.symbol.to_string(),
                                                description: e.description.to_string(),
                                            }).collect()),
                                        });
                                    }
                                    return;
                                }
                                if sas.is_cancelled() {
                                    tracing::warn!("SAS cancelled");
                                    on_cancelled();
                                    return;
                                }
                            }
                            tracing::warn!("Emoji polling timed out");
                        }
                        Ok(None) => tracing::error!("start_sas() returned None"),
                        Err(e) => tracing::error!("start_sas() failed: {e}"),
                    }
                    return;
                }
                VerificationRequestState::Transitioned { verification } => {
                    if let Some(sas) = verification.sas() {
                        if let Err(e) = sas.accept().await {
                            tracing::error!("Failed to accept SAS: {e}");
                            return;
                        }
                        for _ in 0..60 {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            if sas.can_be_presented() {
                                if let Some(emojis) = sas.emoji() {
                                    on_emojis(VerificationInfo {
                                        flow_id: flow_id.clone(),
                                        other_user_id: other_user.clone(),
                                        is_self_verification: false,
                                        state: "emojis_ready".into(),
                                        emojis: Some(emojis.iter().map(|e| EmojiInfo {
                                            symbol: e.symbol.to_string(),
                                            description: e.description.to_string(),
                                        }).collect()),
                                    });
                                }
                                return;
                            }
                            if sas.is_cancelled() { on_cancelled(); return; }
                        }
                    }
                    return;
                }
                VerificationRequestState::Cancelled(_) => {
                    on_cancelled();
                    return;
                }
                VerificationRequestState::Done => return,
                _ => {}
            }
        }
    });

    Ok(flow_id_ret)
}

pub async fn accept_verification(
    client: &matrix_sdk::Client,
    user_id: &str,
    flow_id: &str,
    on_emojis: impl Fn(VerificationInfo) + Send + 'static,
    on_cancelled: impl Fn() + Send + 'static,
) -> Result<(), AppError> {
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let request = client.encryption().get_verification_request(&user_id, flow_id).await
        .ok_or(AppError::Other("Verification request not found".into()))?;

    let state = request.state();
    tracing::info!("Accepting verification, current state: {}", match &state {
        VerificationRequestState::Created { .. } => "Created",
        VerificationRequestState::Requested { .. } => "Requested",
        VerificationRequestState::Ready { .. } => "Ready",
        VerificationRequestState::Transitioned { .. } => "Transitioned",
        VerificationRequestState::Done => "Done",
        VerificationRequestState::Cancelled(_) => "Cancelled",
    });

    // If already Ready (e.g. self-verification), skip accept and go straight to SAS
    if matches!(state, VerificationRequestState::Ready { .. }) {
        tracing::info!("Already in Ready state, starting SAS directly...");
        let sas = request.start_sas().await?.ok_or(AppError::Other("Failed to start SAS".into()))?;
        let flow_id = flow_id.to_string();
        let other_user = user_id.to_string();
        tokio::spawn(async move {
            for i in 0..60 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                if sas.can_be_presented() {
                    tracing::info!("SAS can be presented after {}ms", (i + 1) * 500);
                    if let Some(emojis) = sas.emoji() {
                        tracing::info!("Got {} emojis", emojis.len());
                        on_emojis(VerificationInfo {
                            flow_id: flow_id.clone(), other_user_id: other_user.clone(),
                            is_self_verification: true, state: "emojis_ready".into(),
                            emojis: Some(emojis.iter().map(|e| EmojiInfo { symbol: e.symbol.to_string(), description: e.description.to_string() }).collect()),
                        });
                    }
                    return;
                }
                if sas.is_cancelled() { tracing::warn!("SAS cancelled"); on_cancelled(); return; }
            }
            tracing::warn!("Emoji polling timed out");
        });
        return Ok(());
    }

    request.accept().await?;
    tracing::info!("Accept sent, waiting for state transitions...");

    let flow_id = flow_id.to_string();
    let other_user = user_id.to_string();

    // Use the changes stream to wait for state transitions instead of polling
    tokio::spawn(async move {
        let mut stream = request.changes();

        // Wait for Ready state, then start SAS
        while let Some(state) = stream.next().await {
            match state {
                VerificationRequestState::Ready { .. } => {
                    tracing::info!("Verification request is Ready, starting SAS...");
                    match request.start_sas().await {
                        Ok(Some(sas)) => {
                            tracing::info!("SAS started, waiting for emojis...");
                            // Now poll for emojis on the SAS object
                            for i in 0..60 {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                if sas.can_be_presented() {
                                    tracing::info!("SAS can be presented after {}ms", (i + 1) * 500);
                                    if let Some(emojis) = sas.emoji() {
                                        tracing::info!("Got {} emojis for verification", emojis.len());
                                        on_emojis(VerificationInfo {
                                            flow_id: flow_id.clone(), other_user_id: other_user.clone(),
                                            is_self_verification: true, state: "emojis_ready".into(),
                                            emojis: Some(emojis.iter().map(|e| EmojiInfo { symbol: e.symbol.to_string(), description: e.description.to_string() }).collect()),
                                        });
                                    } else {
                                        tracing::warn!("SAS can be presented but no emojis available");
                                    }
                                    return;
                                }
                                if sas.is_cancelled() { tracing::warn!("SAS cancelled"); on_cancelled(); return; }
                            }
                            tracing::warn!("Emoji polling timed out after 30s");
                        }
                        Ok(None) => { tracing::error!("start_sas() returned None even in Ready state"); }
                        Err(e) => { tracing::error!("start_sas() failed: {e}"); }
                    }
                    return;
                }
                VerificationRequestState::Transitioned { verification } => {
                    // The other side started SAS — we need to accept it
                    tracing::info!("Verification transitioned (other side started SAS)");
                    if let Some(sas) = verification.sas() {
                        tracing::info!("Accepting SAS...");
                        if let Err(e) = sas.accept().await {
                            tracing::error!("Failed to accept SAS: {e}");
                            return;
                        }
                        tracing::info!("SAS accepted, waiting for emojis...");
                        for i in 0..60 {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            if sas.can_be_presented() {
                                tracing::info!("SAS can be presented after {}ms", (i + 1) * 500);
                                if let Some(emojis) = sas.emoji() {
                                    tracing::info!("Got {} emojis for verification", emojis.len());
                                    on_emojis(VerificationInfo {
                                        flow_id: flow_id.clone(), other_user_id: other_user.clone(),
                                        is_self_verification: true, state: "emojis_ready".into(),
                                        emojis: Some(emojis.iter().map(|e| EmojiInfo { symbol: e.symbol.to_string(), description: e.description.to_string() }).collect()),
                                    });
                                }
                                return;
                            }
                            if sas.is_cancelled() { tracing::warn!("SAS cancelled"); on_cancelled(); return; }
                        }
                    }
                    return;
                }
                VerificationRequestState::Cancelled(info) => {
                    tracing::warn!("Verification cancelled: {:?}", info);
                    on_cancelled();
                    return;
                }
                VerificationRequestState::Done => {
                    tracing::info!("Verification done");
                    return;
                }
                other => {
                    tracing::info!("Verification state: {:?}", std::mem::discriminant(&other));
                }
            }
        }
        tracing::warn!("Verification changes stream ended");
    });
    Ok(())
}

pub async fn confirm_verification(client: &matrix_sdk::Client, user_id: &str, flow_id: &str) -> Result<(), AppError> {
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    let verification = client.encryption().get_verification(&user_id, flow_id).await
        .ok_or(AppError::Other("Verification not found".into()))?;
    if let Some(sas) = verification.sas() { sas.confirm().await?; } else { return Err(AppError::Other("Not a SAS verification".into())); }
    Ok(())
}

/// Manually verify a user's identity by signing their master key.
/// Useful for bots that can't do interactive SAS verification.
pub async fn manual_verify_user(client: &matrix_sdk::Client, user_id: &str) -> Result<(), AppError> {
    let uid = matrix_sdk::ruma::UserId::parse(user_id)
        .map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))?;
    let identity = client.encryption().get_user_identity(&uid).await
        .map_err(|e| AppError::Other(format!("Failed to get identity: {e}")))?
        .ok_or_else(|| AppError::Other(format!("No identity found for {user_id}")))?;
    identity.verify().await
        .map_err(|e| AppError::Other(format!("Failed to verify user: {e}")))?;
    Ok(())
}

/// Check if a user's identity is verified.
pub async fn is_user_verified(client: &matrix_sdk::Client, user_id: &str) -> Result<bool, AppError> {
    let uid = matrix_sdk::ruma::UserId::parse(user_id)
        .map_err(|_| AppError::Other(format!("Invalid user ID: {user_id}")))?;
    let identity = client.encryption().get_user_identity(&uid).await
        .map_err(|e| AppError::Other(format!("Failed to get identity: {e}")))?;
    Ok(identity.map(|i| i.is_verified()).unwrap_or(false))
}

/// Request interactive (SAS) verification with a specific device.
/// Used from the sessions settings tab to verify an unverified session.
pub async fn request_device_verification(
    client: &matrix_sdk::Client,
    device_id: &str,
    on_emojis: impl Fn(VerificationInfo) + Send + 'static,
    on_cancelled: impl Fn() + Send + 'static,
) -> Result<String, AppError> {
    let own_user_id = client.user_id()
        .ok_or_else(|| AppError::Other("Not logged in".into()))?;
    let did: &matrix_sdk::ruma::DeviceId = device_id.into();
    let device = client.encryption().get_device(own_user_id, did).await
        .map_err(|e| AppError::Other(format!("Failed to get device: {e}")))?
        .ok_or_else(|| AppError::Other(format!("Device {device_id} not found")))?;

    let request = device.request_verification().await
        .map_err(|e| AppError::Other(format!("Failed to request device verification: {e}")))?;

    let flow_id = request.flow_id().to_string();
    let flow_id_ret = flow_id.clone();
    let other_user = own_user_id.to_string();

    tokio::spawn(async move {
        let mut stream = request.changes();
        while let Some(state) = stream.next().await {
            match state {
                VerificationRequestState::Ready { .. } => {
                    tracing::info!("Device verification request Ready, starting SAS...");
                    match request.start_sas().await {
                        Ok(Some(sas)) => {
                            for _ in 0..60 {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                if sas.can_be_presented() {
                                    if let Some(emojis) = sas.emoji() {
                                        on_emojis(VerificationInfo {
                                            flow_id: flow_id.clone(),
                                            other_user_id: other_user.clone(),
                                            is_self_verification: true,
                                            state: "emojis_ready".into(),
                                            emojis: Some(emojis.iter().map(|e| EmojiInfo {
                                                symbol: e.symbol.to_string(),
                                                description: e.description.to_string(),
                                            }).collect()),
                                        });
                                    }
                                    return;
                                }
                                if sas.is_cancelled() {
                                    tracing::warn!("Device SAS cancelled");
                                    on_cancelled();
                                    return;
                                }
                            }
                            tracing::warn!("Device emoji polling timed out");
                        }
                        Ok(None) => tracing::error!("Device start_sas() returned None"),
                        Err(e) => tracing::error!("Device start_sas() failed: {e}"),
                    }
                    return;
                }
                VerificationRequestState::Transitioned { verification } => {
                    if let Some(sas) = verification.sas() {
                        if let Err(e) = sas.accept().await {
                            tracing::error!("Failed to accept device SAS: {e}");
                            return;
                        }
                        for _ in 0..60 {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            if sas.can_be_presented() {
                                if let Some(emojis) = sas.emoji() {
                                    on_emojis(VerificationInfo {
                                        flow_id: flow_id.clone(),
                                        other_user_id: other_user.clone(),
                                        is_self_verification: true,
                                        state: "emojis_ready".into(),
                                        emojis: Some(emojis.iter().map(|e| EmojiInfo {
                                            symbol: e.symbol.to_string(),
                                            description: e.description.to_string(),
                                        }).collect()),
                                    });
                                }
                                return;
                            }
                            if sas.is_cancelled() { on_cancelled(); return; }
                        }
                    }
                    return;
                }
                VerificationRequestState::Cancelled(_) => {
                    on_cancelled();
                    return;
                }
                VerificationRequestState::Done => return,
                _ => {}
            }
        }
    });

    Ok(flow_id_ret)
}

pub async fn cancel_verification(client: &matrix_sdk::Client, user_id: &str, flow_id: &str) -> Result<(), AppError> {
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    if let Some(v) = client.encryption().get_verification(&user_id, flow_id).await {
        if let Some(sas) = v.sas() { sas.cancel().await?; }
    } else if let Some(req) = client.encryption().get_verification_request(&user_id, flow_id).await {
        req.cancel().await?;
    }
    Ok(())
}
