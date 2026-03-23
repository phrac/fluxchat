use matrix_sdk::{
    config::SyncSettings,
    ruma::events::{
        reaction::OriginalSyncReactionEvent,
        room::{
            member::OriginalSyncRoomMemberEvent,
            message::{MessageType, OriginalSyncRoomMessageEvent, Relation},
        },
        typing::SyncTypingEvent,
    },
    Client, Room,
};
use std::time::Duration;
use tracing::{error, info};

use crate::models::{MessageInfo, VerificationInfo};

/// Trait for receiving sync events. Implement this to bridge events to your UI framework.
pub trait SyncCallbacks: Send + Sync + 'static {
    fn on_message(&self, room_id: String, message: MessageInfo);
    fn on_edit(&self, room_id: String, event_id: String, new_body: String, new_formatted_body: Option<String>);
    fn on_reaction(&self, room_id: String, event_id: String, emoji: String, sender: String);
    fn on_typing(&self, room_id: String, user_ids: Vec<String>);
    fn on_membership_change(&self, room_id: String, message: MessageInfo);
    fn on_room_list_update(&self);
    fn on_verification_request(&self, info: VerificationInfo);
}

pub fn spawn_sync<C: SyncCallbacks>(client: Client, callbacks: C) {
    let callbacks = std::sync::Arc::new(callbacks);

    tokio::spawn(async move {
        let own_user_id = client.user_id().map(|u| u.to_owned());

        // Message handler
        client.add_event_handler({
            let cb = callbacks.clone();
            move |ev: OriginalSyncRoomMessageEvent, room: Room| {
                let cb = cb.clone();
                async move {
                    let (body, formatted_body, image_mxc) = match &ev.content.msgtype {
                        MessageType::Text(t) => (
                            t.body.clone(),
                            t.formatted.as_ref().map(|f| f.body.clone()),
                            None,
                        ),
                        MessageType::Image(img) => {
                            let mxc = match &img.source {
                                matrix_sdk::ruma::events::room::MediaSource::Plain(uri) => Some(uri.to_string()),
                                matrix_sdk::ruma::events::room::MediaSource::Encrypted(enc) => Some(enc.url.to_string()),
                            };
                            (img.body.clone(), None, mxc)
                        }
                        MessageType::Notice(t) => (
                            t.body.clone(),
                            t.formatted.as_ref().map(|f| f.body.clone()),
                            None,
                        ),
                        MessageType::Emote(t) => (
                            format!("* {}", t.body),
                            t.formatted.as_ref().map(|f| format!("* {}", f.body)),
                            None,
                        ),
                        _ => ("[unsupported message]".into(), None, None),
                    };

                    // Check for edit
                    if let Some(Relation::Replacement(replacement)) = &ev.content.relates_to {
                        let (new_body, new_formatted) = match &replacement.new_content.msgtype {
                            MessageType::Text(t) => (t.body.clone(), t.formatted.as_ref().map(|f| f.body.clone())),
                            _ => (body.clone(), formatted_body.clone()),
                        };
                        cb.on_edit(
                            room.room_id().to_string(),
                            replacement.event_id.to_string(),
                            new_body,
                            new_formatted,
                        );
                        return;
                    }

                    let member = room.get_member_no_sync(&ev.sender).await.ok().flatten();
                    let sender_name = member.as_ref()
                        .and_then(|m| m.display_name().map(|s| s.to_string()))
                        .unwrap_or_else(|| ev.sender.to_string());
                    let avatar_url = member.as_ref()
                        .and_then(|m| m.avatar_url())
                        .map(|mxc| mxc.to_string());

                    let reply_to_id = match &ev.content.relates_to {
                        Some(Relation::Reply { in_reply_to }) => Some(in_reply_to.event_id.to_string()),
                        Some(Relation::Thread(thread)) => thread.in_reply_to.as_ref().map(|r| r.event_id.to_string()),
                        _ => None,
                    };

                    let msg = MessageInfo {
                        event_id: ev.event_id.to_string(),
                        sender: ev.sender.to_string(),
                        sender_name,
                        body,
                        formatted_body,
                        transformed_body: None,
                        image_mxc,
                        reactions: Vec::new(),
                        reply_to_id,
                        reply_to_sender: None,
                        reply_to_body: None,
                        timestamp: ev.origin_server_ts.as_secs().into(),
                        avatar_url,
                    };

                    cb.on_message(room.room_id().to_string(), msg);
                }
            }
        });

        // Reaction handler
        client.add_event_handler({
            let cb = callbacks.clone();
            move |ev: OriginalSyncReactionEvent, room: Room| {
                let cb = cb.clone();
                async move {
                    cb.on_reaction(
                        room.room_id().to_string(),
                        ev.content.relates_to.event_id.to_string(),
                        ev.content.relates_to.key.clone(),
                        ev.sender.to_string(),
                    );
                }
            }
        });

        // Typing handler
        client.add_event_handler({
            let cb = callbacks.clone();
            let own_id = own_user_id.clone();
            move |ev: SyncTypingEvent, room: Room| {
                let cb = cb.clone();
                let own_id = own_id.clone();
                async move {
                    let user_ids: Vec<String> = ev.content.user_ids.iter()
                        .filter(|u| own_id.as_ref().map_or(true, |own| *u != own))
                        .map(|u| u.to_string())
                        .collect();
                    cb.on_typing(room.room_id().to_string(), user_ids);
                }
            }
        });

        // Membership handler
        client.add_event_handler({
            let cb = callbacks.clone();
            move |ev: OriginalSyncRoomMemberEvent, room: Room| {
                let cb = cb.clone();
                async move {
                    use matrix_sdk::ruma::events::room::member::MembershipState;

                    cb.on_room_list_update();

                    let action = match ev.content.membership {
                        MembershipState::Join => {
                            if ev.prev_content().map(|p| &p.membership) == Some(&MembershipState::Invite) {
                                "accepted the invitation"
                            } else { "joined the room" }
                        }
                        MembershipState::Leave => {
                            if ev.state_key.as_str() == ev.sender.as_str() { "left the room" }
                            else { "was removed from the room" }
                        }
                        MembershipState::Invite => "was invited",
                        MembershipState::Ban => "was banned",
                        _ => "",
                    };

                    if !action.is_empty() {
                        let name = ev.content.displayname.as_deref()
                            .unwrap_or(ev.state_key.as_str());
                        let body = if ev.state_key.as_str() != ev.sender.as_str()
                            && !action.starts_with("accepted")
                            && !action.starts_with("joined")
                        {
                            format!("{} {} {}", ev.sender, action.trim_start_matches("was "), name)
                        } else {
                            format!("{} {}", name, action)
                        };

                        let msg = MessageInfo {
                            event_id: format!("~state-{}", ev.event_id),
                            sender: String::new(),
                            sender_name: String::new(),
                            body,
                            formatted_body: None,
                            transformed_body: None,
                            image_mxc: None,
                            reactions: Vec::new(),
                            reply_to_id: None,
                            reply_to_sender: None,
                            reply_to_body: None,
                            timestamp: ev.origin_server_ts.as_secs().into(),
                            avatar_url: None,
                        };

                        cb.on_membership_change(room.room_id().to_string(), msg);
                    }
                }
            }
        });

        // Verification handler
        {
            use matrix_sdk::ruma::events::key::verification::request::ToDeviceKeyVerificationRequestEvent;
            client.add_event_handler({
                let cb = callbacks.clone();
                let client = client.clone();
                move |ev: ToDeviceKeyVerificationRequestEvent| {
                    let cb = cb.clone();
                    let client = client.clone();
                    async move {
                        let request = client.encryption()
                            .get_verification_request(&ev.sender, &ev.content.transaction_id)
                            .await;
                        if let Some(request) = request {
                            cb.on_verification_request(VerificationInfo {
                                flow_id: request.flow_id().to_string(),
                                other_user_id: request.other_user_id().to_string(),
                                is_self_verification: request.is_self_verification(),
                                state: "requested".into(),
                                emojis: None,
                            });
                        }
                    }
                }
            });
        }

        info!("Starting initial sync...");
        let sync_settings = SyncSettings::default().timeout(Duration::from_secs(10));

        match client.sync_once(sync_settings.clone()).await {
            Ok(_) => {
                info!("Initial sync complete");
                callbacks.on_room_list_update();
            }
            Err(e) => {
                error!("Initial sync failed: {e}");
                return;
            }
        }

        info!("Starting continuous sync...");
        client.sync(sync_settings).await.unwrap_or_else(|e| {
            error!("Sync loop ended with error: {e}");
        });
    });
}
