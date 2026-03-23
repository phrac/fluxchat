use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::{init_core, UiState};
use crate::components::login::Login;
use crate::components::layout::MainLayout;

const STYLE: Asset = asset!("assets/style.css");

#[derive(Clone)]
pub enum SyncEvent {
    Message(String, fluxchat_core::MessageInfo),
    Edit(String, String, String, Option<String>),
    Reaction(String, String, String, String),
    Typing(String, Vec<String>),
    Membership(String, fluxchat_core::MessageInfo),
    RoomListUpdate,
    Verification(fluxchat_core::VerificationInfo),
}

// Global channel for sync events
pub static SYNC_TX: std::sync::OnceLock<std::sync::mpsc::Sender<SyncEvent>> = std::sync::OnceLock::new();
static SYNC_RX: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Receiver<SyncEvent>>> = std::sync::OnceLock::new();
static CORE: std::sync::OnceLock<Arc<AppState>> = std::sync::OnceLock::new();

fn init_sync_channel() {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = SYNC_TX.set(tx);
    let _ = SYNC_RX.set(std::sync::Mutex::new(rx));
}

#[component]
pub fn App() -> Element {
    let core = use_context_provider(|| {
        let c = init_core();
        let _ = CORE.set(c.clone());
        c
    });
    let ui = use_context_provider(|| {
        init_sync_channel();
        UiState::new()
    });

    // Restore session on mount
    use_effect(move || {
        let core = core.clone();
        spawn(async move {
            let mut ui = consume_context::<UiState>();
            match fluxchat_core::auth::restore_session(&core).await {
                Ok(Some((client, result))) => {
                    ui.user_id.set(result.user_id);
                    ui.display_name.set(result.display_name);
                    ui.logged_in.set(true);
                    start_sync_and_load(&core, &mut ui, &client).await;
                }
                _ => {}
            }
            ui.loading.set(false);
        });
    });

    // Poll sync events
    use_future(move || async move {
        loop {
            if let Some(rx) = SYNC_RX.get() {
                if let Ok(guard) = rx.try_lock() {
                    while let Ok(event) = guard.try_recv() {
                        let mut ui = consume_context::<UiState>();
                        apply_sync_event(&mut ui, event);
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });

    rsx! {
        document::Stylesheet { href: STYLE }
        div { id: "main",
            if *ui.loading.read() {
                div {
                    style: "display: flex; align-items: center; justify-content: center; height: 100vh; color: var(--text-muted);",
                    "Loading..."
                }
            } else if *ui.logged_in.read() {
                MainLayout {}
            } else {
                Login {}
            }
        }
    }
}

fn apply_sync_event(ui: &mut UiState, event: SyncEvent) {
    match event {
        SyncEvent::Message(room_id, mut msg) => {
            // Pre-calculate transformed_body with user_id → avatar map
            if let Some(ref html) = msg.formatted_body {
                if let Some(core) = CORE.get() {
                    let media_cache = core.media_cache.lock().unwrap();
                    let mention_map: std::collections::HashMap<String, String> = ui.members_by_room.read()
                        .get(&room_id)
                        .map(|members| members.iter().filter_map(|m| {
                            let mxc = m.avatar_url.as_ref()?;
                            Some((m.user_id.clone(), media_cache.get(mxc)?.clone()))
                        }).collect())
                        .unwrap_or_default();
                    msg.transformed_body = Some(crate::state::transform_mentions(html, &mention_map));
                }
            }

            let mut messages = ui.messages_by_room.write();
            let room_msgs = messages.entry(room_id.clone()).or_insert_with(Vec::new);
            room_msgs.retain(|m| !(m.event_id.starts_with("~local-") && m.sender == msg.sender && m.body == msg.body));
            if !room_msgs.iter().any(|m| m.event_id == msg.event_id) {
                room_msgs.push(msg);
                if room_msgs.len() > 200 {
                    let drain_count = room_msgs.len() - 200;
                    room_msgs.drain(..drain_count);
                }
            }
            drop(messages);
            // Mark as read if this is the active room
            // (scroll handled by CSS column-reverse — browser auto-shows new content at bottom)
            if ui.selected_room_id.read().as_ref() == Some(&room_id) {
                // Send read receipt for the new message
                let last_eid = ui.messages_by_room.read().get(&room_id)
                    .and_then(|msgs| msgs.last())
                    .map(|m| m.event_id.clone())
                    .filter(|eid| !eid.starts_with("~"));
                if let Some(eid) = last_eid {
                    // Zero unread locally
                    let mut rooms = ui.rooms.write();
                    if let Some(r) = rooms.iter_mut().find(|r| r.room_id == room_id) {
                        r.unread_count = 0;
                    }
                    drop(rooms);
                    if let Some(core) = CORE.get() {
                        let core = core.clone();
                        let room_id = room_id.clone();
                        spawn(async move {
                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                            let _ = fluxchat_core::messages::send_read_receipt(&client, &room_id, &eid).await;
                        });
                    }
                }
            }
        }
        SyncEvent::Edit(room_id, event_id, body, formatted) => {
            let mut messages = ui.messages_by_room.write();
            if let Some(room_msgs) = messages.get_mut(&room_id) {
                if let Some(msg) = room_msgs.iter_mut().find(|m| m.event_id == event_id) {
                    msg.body = body; msg.formatted_body = formatted;
                }
            }
        }
        SyncEvent::Reaction(room_id, event_id, emoji, sender) => {
            let mut messages = ui.messages_by_room.write();
            if let Some(room_msgs) = messages.get_mut(&room_id) {
                if let Some(msg) = room_msgs.iter_mut().find(|m| m.event_id == event_id) {
                    if let Some(r) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                        if !r.users.contains(&sender) { r.count += 1; r.users.push(sender); }
                    } else { msg.reactions.push(fluxchat_core::ReactionInfo { emoji, count: 1, users: vec![sender] }); }
                }
            }
        }
        SyncEvent::Typing(room_id, user_ids) => { ui.typing_by_room.write().insert(room_id, user_ids); }
        SyncEvent::Membership(room_id, msg) => { ui.messages_by_room.write().entry(room_id).or_insert_with(Vec::new).push(msg); }
        SyncEvent::Verification(info) => {
            ui.verification_request.set(Some(info));
        }
        SyncEvent::RoomListUpdate => {
            if let Some(core) = CORE.get() {
                let core = core.clone();
                spawn(async move {
                    let mut ui = consume_context::<UiState>();
                    if let Ok(client) = core.get_client().await {
                        if let Ok(mut rooms) = fluxchat_core::rooms::get_rooms(&client).await {
                            // Zero unread for all rooms we've marked as read this session
                            let read = ui.read_rooms.read().clone();
                            for r in rooms.iter_mut() {
                                if read.contains(&r.room_id) {
                                    r.unread_count = 0;
                                }
                            }
                            ui.rooms.set(rooms);
                        }
                        if let Ok(spaces) = fluxchat_core::rooms::get_spaces(&client).await { ui.spaces.set(spaces); }
                    }
                });
            }
        }
    }
}

pub async fn start_sync_and_load(core: &Arc<AppState>, ui: &mut UiState, client: &fluxchat_core::Client) {
    // Load persisted read rooms
    if let Some(json) = core.db.get_setting("read_rooms") {
        if let Ok(ids) = serde_json::from_str::<Vec<String>>(&json) {
            *ui.read_rooms.write() = ids.into_iter().collect();
        }
    }

    // Load rooms before sync for immediate UI, but they'll be refreshed after initial sync
    if let Ok(mut rooms) = fluxchat_core::rooms::get_rooms(client).await {
        let read = ui.read_rooms.read().clone();
        for r in rooms.iter_mut() {
            if read.contains(&r.room_id) {
                r.unread_count = 0;
            }
        }
        ui.rooms.set(rooms);
    } else if let Ok(rooms) = fluxchat_core::rooms::get_rooms(client).await { ui.rooms.set(rooms); }
    if let Ok(spaces) = fluxchat_core::rooms::get_spaces(client).await { ui.spaces.set(spaces); }

    if let Some(tx) = SYNC_TX.get() {
        let bridge = ChannelSyncBridge { tx: tx.clone() };
        fluxchat_core::sync::spawn_sync(client.clone(), bridge);
    }
}

struct ChannelSyncBridge { tx: std::sync::mpsc::Sender<SyncEvent> }

impl fluxchat_core::sync::SyncCallbacks for ChannelSyncBridge {
    fn on_message(&self, room_id: String, msg: fluxchat_core::MessageInfo) { let _ = self.tx.send(SyncEvent::Message(room_id, msg)); }
    fn on_edit(&self, room_id: String, event_id: String, body: String, formatted: Option<String>) { let _ = self.tx.send(SyncEvent::Edit(room_id, event_id, body, formatted)); }
    fn on_reaction(&self, room_id: String, event_id: String, emoji: String, sender: String) { let _ = self.tx.send(SyncEvent::Reaction(room_id, event_id, emoji, sender)); }
    fn on_typing(&self, room_id: String, user_ids: Vec<String>) { let _ = self.tx.send(SyncEvent::Typing(room_id, user_ids)); }
    fn on_membership_change(&self, room_id: String, msg: fluxchat_core::MessageInfo) { let _ = self.tx.send(SyncEvent::Membership(room_id, msg)); }
    fn on_room_list_update(&self) { let _ = self.tx.send(SyncEvent::RoomListUpdate); }
    fn on_verification_request(&self, info: fluxchat_core::VerificationInfo) { let _ = self.tx.send(SyncEvent::Verification(info)); }
}
