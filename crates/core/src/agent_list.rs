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
pub async fn get(client: &Client) -> Result<Option<AgentList>, AppError> {
    let raw = client.account().account_data::<AgentListEventContent>().await
        .map_err(|e| AppError::Other(format!("{e}")))?;
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
}
