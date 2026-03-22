use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::{init_core, UiState};
use crate::components::login::Login;
use crate::components::layout::MainLayout;

const STYLE: Asset = asset!("assets/style.css");

/// Sync events sent from the background sync task to the UI.
#[derive(Clone)]
pub enum SyncEvent {
    Message(String, fluxchat_core::MessageInfo),
    Edit(String, String, String, Option<String>),
    Reaction(String, String, String, String),
    Typing(String, Vec<String>),
    Membership(String, fluxchat_core::MessageInfo),
    RoomListUpdate,
}

#[component]
pub fn App() -> Element {
    let core = use_signal(|| init_core());
    let ui = use_context_provider(|| UiState::new());
    use_context_provider(|| core());

    // Channel for sync events
    let (tx, rx) = std::sync::mpsc::channel::<SyncEvent>();
    let tx = use_signal(|| Arc::new(std::sync::Mutex::new(tx)));
    let rx = use_signal(|| Arc::new(std::sync::Mutex::new(Some(rx))));

    // Restore session on mount
    let core_clone = core();
    use_effect(move || {
        let core = core_clone.clone();
        let tx = tx().clone();
        spawn(async move {
            match fluxchat_core::auth::restore_session(&core).await {
                Ok(Some((client, result))) => {
                    ui.user_id.set(result.user_id);
                    ui.display_name.set(result.display_name);
                    ui.logged_in.set(true);
                    load_data_and_sync(&core, &ui, tx).await;
                }
                _ => {}
            }
            ui.loading.set(false);
        });
    });

    // Poll sync events and apply to UI signals
    let _ = use_resource(move || {
        let rx = rx();
        async move {
            let receiver = {
                let mut guard = rx.lock().unwrap();
                guard.take()
            };
            if let Some(receiver) = receiver {
                loop {
                    match receiver.try_recv() {
                        Ok(event) => apply_sync_event(&ui, event),
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            }
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

fn apply_sync_event(ui: &UiState, event: SyncEvent) {
    match event {
        SyncEvent::Message(room_id, msg) => {
            let mut messages = ui.messages_by_room.write();
            let room_msgs = messages.entry(room_id).or_insert_with(Vec::new);
            room_msgs.retain(|m| !(m.event_id.starts_with("~local-") && m.sender == msg.sender && m.body == msg.body));
            if !room_msgs.iter().any(|m| m.event_id == msg.event_id) {
                room_msgs.push(msg);
            }
        }
        SyncEvent::Edit(room_id, event_id, body, formatted) => {
            let mut messages = ui.messages_by_room.write();
            if let Some(room_msgs) = messages.get_mut(&room_id) {
                if let Some(msg) = room_msgs.iter_mut().find(|m| m.event_id == event_id) {
                    msg.body = body;
                    msg.formatted_body = formatted;
                }
            }
        }
        SyncEvent::Reaction(room_id, event_id, emoji, sender) => {
            let mut messages = ui.messages_by_room.write();
            if let Some(room_msgs) = messages.get_mut(&room_id) {
                if let Some(msg) = room_msgs.iter_mut().find(|m| m.event_id == event_id) {
                    if let Some(r) = msg.reactions.iter_mut().find(|r| r.emoji == emoji) {
                        if !r.users.contains(&sender) { r.count += 1; r.users.push(sender); }
                    } else {
                        msg.reactions.push(fluxchat_core::ReactionInfo { emoji, count: 1, users: vec![sender] });
                    }
                }
            }
        }
        SyncEvent::Typing(room_id, user_ids) => {
            ui.typing_by_room.write().insert(room_id, user_ids);
        }
        SyncEvent::Membership(room_id, msg) => {
            ui.messages_by_room.write().entry(room_id).or_insert_with(Vec::new).push(msg);
        }
        SyncEvent::RoomListUpdate => {
            // Will be handled by periodic refresh
        }
    }
}

pub async fn load_data_and_sync(core: &Arc<AppState>, ui: &UiState, tx: Arc<std::sync::Mutex<std::sync::mpsc::Sender<SyncEvent>>>) {
    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };

    if let Ok(rooms) = fluxchat_core::rooms::get_rooms(&client).await { ui.rooms.set(rooms); }
    if let Ok(spaces) = fluxchat_core::rooms::get_spaces(&client).await { ui.spaces.set(spaces); }

    // Spawn sync with channel-based bridge
    let bridge = ChannelSyncBridge { tx: tx.lock().unwrap().clone() };
    fluxchat_core::sync::spawn_sync(client, bridge);
}

/// Send + Sync bridge that forwards events via mpsc channel.
struct ChannelSyncBridge {
    tx: std::sync::mpsc::Sender<SyncEvent>,
}

impl fluxchat_core::sync::SyncCallbacks for ChannelSyncBridge {
    fn on_message(&self, room_id: String, msg: fluxchat_core::MessageInfo) {
        let _ = self.tx.send(SyncEvent::Message(room_id, msg));
    }
    fn on_edit(&self, room_id: String, event_id: String, body: String, formatted: Option<String>) {
        let _ = self.tx.send(SyncEvent::Edit(room_id, event_id, body, formatted));
    }
    fn on_reaction(&self, room_id: String, event_id: String, emoji: String, sender: String) {
        let _ = self.tx.send(SyncEvent::Reaction(room_id, event_id, emoji, sender));
    }
    fn on_typing(&self, room_id: String, user_ids: Vec<String>) {
        let _ = self.tx.send(SyncEvent::Typing(room_id, user_ids));
    }
    fn on_membership_change(&self, room_id: String, msg: fluxchat_core::MessageInfo) {
        let _ = self.tx.send(SyncEvent::Membership(room_id, msg));
    }
    fn on_room_list_update(&self) {
        let _ = self.tx.send(SyncEvent::RoomListUpdate);
    }
    fn on_verification_request(&self, _info: fluxchat_core::VerificationInfo) {}
}
