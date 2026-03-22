use crate::error::AppError;
use crate::models::{VerificationInfo, EmojiInfo};

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
    request.accept().await?;
    let sas = request.start_sas().await?.ok_or(AppError::Other("Failed to start SAS".into()))?;

    let flow_id = flow_id.to_string();
    let other_user = user_id.to_string();
    tokio::spawn(async move {
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if sas.can_be_presented() {
                if let Some(emojis) = sas.emoji() {
                    on_emojis(VerificationInfo {
                        flow_id: flow_id.clone(), other_user_id: other_user.clone(),
                        is_self_verification: false, state: "emojis_ready".into(),
                        emojis: Some(emojis.iter().map(|e| EmojiInfo { symbol: e.symbol.to_string(), description: e.description.to_string() }).collect()),
                    });
                }
                break;
            }
            if sas.is_cancelled() { on_cancelled(); return; }
        }
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

pub async fn cancel_verification(client: &matrix_sdk::Client, user_id: &str, flow_id: &str) -> Result<(), AppError> {
    let user_id = matrix_sdk::ruma::UserId::parse(user_id).map_err(|_| AppError::Other("Invalid user ID".into()))?;
    if let Some(v) = client.encryption().get_verification(&user_id, flow_id).await {
        if let Some(sas) = v.sas() { sas.cancel().await?; }
    } else if let Some(req) = client.encryption().get_verification_request(&user_id, flow_id).await {
        req.cancel().await?;
    }
    Ok(())
}
