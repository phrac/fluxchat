use matrix_sdk::ruma::api::client::account::change_password::v3::Request as ChangePasswordRequest;
use crate::error::AppError;
use crate::models::UserProfile;

/// Fetch the current user's profile (name, avatar, banner).
pub async fn get_profile(client: &matrix_sdk::Client) -> Result<UserProfile, AppError> {
    let user_id = client.user_id().ok_or(AppError::NotLoggedIn)?.to_string();
    let display_name = client.account().get_display_name().await.ok().flatten();
    let avatar_url = client.account().get_avatar_url().await.ok().flatten().map(|u| u.to_string());
    let banner_url = client.account()
        .account_data_raw(matrix_sdk::ruma::events::GlobalAccountDataEventType::from("io.element.banner"))
        .await.ok().flatten()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw.json().get()).ok())
        .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(String::from));
    Ok(UserProfile { user_id, display_name, avatar_url, banner_url })
}

/// Update the current user's display name.
pub async fn set_display_name(client: &matrix_sdk::Client, name: &str) -> Result<(), AppError> {
    client.account().set_display_name(Some(name)).await.map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(())
}

/// Upload a new avatar image for the current user.
pub async fn upload_avatar(client: &matrix_sdk::Client, mime_type: &str, data: Vec<u8>) -> Result<String, AppError> {
    let ct: mime::Mime = mime_type.parse().unwrap_or(mime::IMAGE_PNG);
    Ok(client.account().upload_avatar(&ct, data).await.map_err(|e| AppError::Other(format!("{e}")))?.to_string())
}

/// Remove the current user's avatar.
pub async fn remove_avatar(client: &matrix_sdk::Client) -> Result<(), AppError> {
    client.account().set_avatar_url(None).await.map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(())
}

/// Upload a new banner image for the current user.
pub async fn upload_banner(client: &matrix_sdk::Client, mime_type: &str, data: Vec<u8>) -> Result<String, AppError> {
    let ct: mime::Mime = mime_type.parse().unwrap_or(mime::IMAGE_PNG);
    let mxc = client.media().upload(&ct, data, None).await.map_err(|e| AppError::Other(format!("{e}")))?.content_uri.to_string();
    let raw = matrix_sdk::ruma::serde::Raw::from_json(
        serde_json::value::to_raw_value(&serde_json::json!({ "url": mxc })).map_err(|e| AppError::Other(format!("{e}")))?
    );
    client.account().set_account_data_raw(matrix_sdk::ruma::events::GlobalAccountDataEventType::from("io.element.banner"), raw)
        .await.map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(mxc)
}

/// Remove the current user's banner image.
pub async fn remove_banner(client: &matrix_sdk::Client) -> Result<(), AppError> {
    let raw = matrix_sdk::ruma::serde::Raw::from_json(
        serde_json::value::to_raw_value(&serde_json::json!({})).map_err(|e| AppError::Other(format!("{e}")))?
    );
    client.account().set_account_data_raw(matrix_sdk::ruma::events::GlobalAccountDataEventType::from("io.element.banner"), raw)
        .await.map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(())
}

/// Change the current user's password.
pub async fn change_password(client: &matrix_sdk::Client, old_password: &str, new_password: &str) -> Result<(), AppError> {
    let mut request = ChangePasswordRequest::new(new_password.to_owned());
    // Provide old password as auth for UIAA
    request.auth = Some(matrix_sdk::ruma::api::client::uiaa::AuthData::Password(
        matrix_sdk::ruma::api::client::uiaa::Password::new(
            matrix_sdk::ruma::api::client::uiaa::UserIdentifier::Matrix(matrix_sdk::ruma::api::client::uiaa::MatrixUserIdentifier::new(client.user_id().ok_or(AppError::NotLoggedIn)?.localpart().to_owned())),
            old_password.to_owned(),
        ),
    ));
    client.send(request).await.map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(())
}

/// Deactivate the current user's account.
pub async fn deactivate_account(client: &matrix_sdk::Client, password: &str) -> Result<(), AppError> {
    use matrix_sdk::ruma::api::client::account::deactivate::v3::Request as DeactivateRequest;
    let mut request = DeactivateRequest::new();
    request.auth = Some(matrix_sdk::ruma::api::client::uiaa::AuthData::Password(
        matrix_sdk::ruma::api::client::uiaa::Password::new(
            matrix_sdk::ruma::api::client::uiaa::UserIdentifier::Matrix(matrix_sdk::ruma::api::client::uiaa::MatrixUserIdentifier::new(client.user_id().ok_or(AppError::NotLoggedIn)?.localpart().to_owned())),
            password.to_owned(),
        ),
    ));
    client.send(request).await.map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(())
}

/// Users on the account's ignore list (`m.ignored_user_list`), sorted.
pub async fn ignored_users(client: &matrix_sdk::Client) -> Result<Vec<String>, AppError> {
    use matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent;
    let content = client.account().account_data::<IgnoredUserListEventContent>().await
        .map_err(|e| AppError::Other(format!("{e}")))?
        .and_then(|raw| raw.deserialize().ok());
    let mut users: Vec<String> = content.map(|c| c.ignored_users.into_keys().map(|u| u.to_string()).collect()).unwrap_or_default();
    users.sort();
    Ok(users)
}

/// Add a user to, or remove them from, the account's ignore list. The server
/// then stops sending us their events; other clients pick the change up too.
pub async fn set_ignored(client: &matrix_sdk::Client, user_id: &str, ignore: bool) -> Result<(), AppError> {
    let user_id = crate::rooms::parse_user_id(user_id)?;
    let account = client.account();
    let result = if ignore { account.ignore_user(&user_id).await } else { account.unignore_user(&user_id).await };
    result.map_err(|e| AppError::Other(format!("{e}")))
}
