//! Which users are AI agents, shared between your devices.
//!
//! Desktop fluxchat and the Android companion keep the list in global
//! account data, `org.fluxchat.agents`:
//!
//! ```json
//! {"agents": [{"user_id": "@hermes:example.org", "harness": "hermes"}],
//!  "not_agents": ["@friend:example.org"]}
//! ```
//!
//! `harness` is a harness profile id (`hermes`, `openclaw`, …); left out, the
//! client's default applies. `not_agents`: users you said aren't agents.

use matrix_sdk::ruma::events::macros::EventContent;
use matrix_sdk::ruma::events::GlobalAccountDataEvent;
use matrix_sdk::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, EventContent)]
#[ruma_event(type = "org.fluxchat.agents", kind = GlobalAccountData)]
pub struct AgentListEventContent {
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_agents: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEntry {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
}

pub type AgentList = AgentListEventContent;

/// The account's list, `None` if no device has saved one yet.
///
/// The local store only has it once sync has brought the account data in,
/// and this is read at start, before the first sync response: on a fresh
/// store (a new sign-in) "not here" is not "none saved". Taken as that, the
/// caller uploaded this device's list over the one the account already had.
/// So when the store has nothing the server is asked, and only its "not
/// found" is `None`; a request that fails is an error. (Requests retry on
/// their own for a while, so offline this waits for the server.)
pub async fn get(client: &Client) -> Result<Option<AgentList>, AppError> {
    let stored = client.account().account_data::<AgentListEventContent>().await
        .map_err(|e| AppError::Other(format!("{e}")))?;
    let raw = match stored {
        Some(raw) => Some(raw),
        None => client.account().fetch_account_data_static::<AgentListEventContent>().await
            .map_err(|e| AppError::Other(format!("{e}")))?,
    };
    match raw {
        Some(raw) => raw.deserialize().map(Some).map_err(|e| AppError::Other(format!("{e}"))),
        None => Ok(None),
    }
}

pub async fn set(client: &Client, list: AgentList) -> Result<(), AppError> {
    client.account().set_account_data(list).await
        .map_err(|e| AppError::Other(format!("{e}")))?;
    Ok(())
}

/// Call `f` with the list whenever it changes (from any device, this one
/// included).
pub fn watch(client: &Client, f: impl Fn(AgentList) + Send + Sync + Clone + 'static) {
    client.add_event_handler(move |ev: GlobalAccountDataEvent<AgentListEventContent>| {
        let f = f.clone();
        async move { f(ev.content) }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_missing_fields() {
        let list: AgentList = serde_json::from_value(serde_json::json!({
            "agents": [{"user_id": "@h:x", "harness": "hermes"}, {"user_id": "@o:x"}]
        })).unwrap();
        assert_eq!(list.agents[1].harness, None);
        assert!(list.not_agents.is_empty());
        let v = serde_json::to_value(&list).unwrap();
        assert_eq!(v, serde_json::json!({"agents": [{"user_id": "@h:x", "harness": "hermes"}, {"user_id": "@o:x"}]}));
        let empty: AgentList = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.agents.is_empty());
    }

    use crate::test_http::{json, nowhere, serve};
    use matrix_sdk::authentication::{matrix::MatrixSession, SessionTokens};
    use matrix_sdk::ruma::{device_id, user_id};
    use matrix_sdk::SessionMeta;

    /// A signed-in client that hasn't synced (its store is empty), talking
    /// to the server at `base`.
    async fn client(base: &str) -> Client {
        let client = Client::builder()
            .homeserver_url(base)
            .request_config(matrix_sdk::config::RequestConfig::new().disable_retry())
            .build()
            .await
            .unwrap();
        client
            .restore_session(MatrixSession {
                meta: SessionMeta { user_id: user_id!("@me:example.org").to_owned(), device_id: device_id!("DEV").to_owned() },
                tokens: SessionTokens { access_token: "token".into(), refresh_token: None },
            })
            .await
            .unwrap();
        client
    }

    /// A homeserver whose account has `list` saved (or none).
    fn homeserver(list: Option<&'static str>) -> impl Fn(&str) -> Vec<u8> + Send + Sync + 'static {
        move |request| {
            if request.contains("/_matrix/client/versions") {
                json("200 OK", r#"{"versions":["v1.11"]}"#)
            } else if request.contains("/account_data/org.fluxchat.agents") {
                match list {
                    Some(content) => json("200 OK", content),
                    None => json("404 Not Found", r#"{"errcode":"M_NOT_FOUND","error":"Account data not found"}"#),
                }
            } else {
                json("404 Not Found", r#"{"errcode":"M_UNRECOGNIZED","error":"Unrecognized request"}"#)
            }
        }
    }

    #[tokio::test]
    async fn a_list_the_store_has_not_synced_yet_is_read_from_the_server() {
        let base = serve(homeserver(Some(r#"{"agents":[{"user_id":"@hermes:example.org","harness":"hermes"}]}"#))).await;
        let list = get(&client(&base).await).await.unwrap().expect("the account's list");
        assert_eq!(list.agents, vec![AgentEntry { user_id: "@hermes:example.org".into(), harness: Some("hermes".into()) }]);
    }

    #[tokio::test]
    async fn only_the_servers_not_found_means_no_list_was_saved() {
        let base = serve(homeserver(None)).await;
        assert_eq!(get(&client(&base).await).await.unwrap(), None);
        // A server that can't be reached hasn't said so: answering "none"
        // here is what made the caller upload its own list over the account's.
        assert!(get(&client(&nowhere().await).await).await.is_err());
    }
}
