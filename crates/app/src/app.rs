use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::{init_core, UiState};
use crate::components::login::Login;
use crate::components::layout::MainLayout;

const STYLE: Asset = asset!("assets/style.css");

#[derive(Clone)]
pub enum SyncEvent {
    // Message, Edit, Reaction, Membership — now handled by Timeline diff stream
    Typing(String, Vec<String>),
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
        // Message, Edit, Reaction, Membership are now handled by Timeline diff stream
        SyncEvent::Typing(room_id, user_ids) => { ui.typing_by_room.write().insert(room_id, user_ids); }
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
    // Message, Edit, Reaction, Membership — now handled by Timeline diff stream, no-op here
    fn on_message(&self, _room_id: String, _msg: fluxchat_core::MessageInfo) {}
    fn on_edit(&self, _room_id: String, _event_id: String, _body: String, _formatted: Option<String>) {}
    fn on_reaction(&self, _room_id: String, _event_id: String, _emoji: String, _sender: String) {}
    fn on_typing(&self, room_id: String, user_ids: Vec<String>) { let _ = self.tx.send(SyncEvent::Typing(room_id, user_ids)); }
    fn on_membership_change(&self, _room_id: String, _msg: fluxchat_core::MessageInfo) {}
    fn on_room_list_update(&self) { let _ = self.tx.send(SyncEvent::RoomListUpdate); }
    fn on_verification_request(&self, info: fluxchat_core::VerificationInfo) { let _ = self.tx.send(SyncEvent::Verification(info)); }
}
