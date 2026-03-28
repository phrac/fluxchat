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
            if let Ok(Some((client, result))) = fluxchat_core::auth::restore_session(&core).await {
                ui.user_id.set(result.user_id);
                ui.display_name.set(result.display_name);
                ui.logged_in.set(true);
                start_sync_and_load(&core, &mut ui, &client).await;
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

    // Apply zoom level
    use_effect(move || {
        let zoom = *ui.zoom_level.read();
        let pct = zoom as f64 / 100.0;
        document::eval(&format!("document.body.style.zoom = '{pct}'"));
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
                                    r.highlight_count = 0;
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

    // Load highlight words
    if let Some(words_json) = core.db.get_setting("highlight_words") {
        if let Ok(words) = serde_json::from_str::<Vec<String>>(&words_json) {
            ui.highlight_words.set(words);
        }
    }

    // Load display settings
    if let Some(fmt) = core.db.get_setting("timestamp_format") {
        ui.timestamp_format.set(fmt);
    }
    if let Some(val) = core.db.get_setting("compact_mode") {
        ui.compact_mode.set(val == "true");
    }
    if let Some(val) = core.db.get_setting("show_joinpart") {
        ui.show_joinpart.set(val == "true");
    }
    if let Some(key) = core.db.get_setting("giphy_api_key") {
        ui.giphy_api_key.set(key);
    }
    if let Some(val) = core.db.get_setting("zoom_level") {
        if let Ok(level) = val.parse::<u32>() {
            ui.zoom_level.set(level);
        }
    }

    // Load ignored users from Matrix account data
    {
        use matrix_sdk::ruma::events::ignored_user_list::IgnoredUserListEventContent;
        if let Ok(Some(raw)) = client.account().account_data::<IgnoredUserListEventContent>().await {
            if let Ok(content) = raw.deserialize() {
                let ignored: std::collections::HashSet<String> = content.ignored_users.keys()
                    .map(|uid| uid.to_string())
                    .collect();
                tracing::info!("Loaded {} ignored users", ignored.len());
                *ui.ignored_users.write() = ignored;
            }
        }
    }

    // Try SyncService (Sliding Sync) first — gives us live room list, typing, etc.
    match fluxchat_core::sync_service::create_sync_service(client).await {
        Ok(sync_service) => {
            tracing::info!("Using SyncService (Sliding Sync)");

            // Load initial room list
            if let Ok(mut rooms) = fluxchat_core::rooms::get_rooms(client).await {
                let read = ui.read_rooms.read().clone();
                for r in rooms.iter_mut() {
                    if read.contains(&r.room_id) { r.unread_count = 0; r.highlight_count = 0; }
                }
                ui.rooms.set(rooms);
            }
            if let Ok(spaces) = fluxchat_core::rooms::get_spaces(client).await { ui.spaces.set(spaces); }
            if let Ok(invited) = fluxchat_core::rooms::get_invited_rooms(client).await { ui.invited_rooms.set(invited); }

            // Spawn a task to periodically refresh room list (SyncService keeps SDK state updated)
            {
                let core = core.clone();
                spawn(async move {
                    // Periodically refresh room list from the SDK (the sliding sync keeps it updated)
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        let mut ui = consume_context::<UiState>();
                        if let Ok(client) = core.get_client().await {
                            if let Ok(mut rooms) = fluxchat_core::rooms::get_rooms(&client).await {
                                let read = ui.read_rooms.read().clone();
                                for r in rooms.iter_mut() {
                                    if read.contains(&r.room_id) { r.unread_count = 0; r.highlight_count = 0; }
                                }
                                ui.rooms.set(rooms);
                            }
                            if let Ok(spaces) = fluxchat_core::rooms::get_spaces(&client).await {
                                ui.spaces.set(spaces);
                            }
                            if let Ok(invited) = fluxchat_core::rooms::get_invited_rooms(&client).await {
                                ui.invited_rooms.set(invited);
                            }
                        }
                    }
                });
            }

            // Store the sync service
            *core.sync_service.lock().await = Some(sync_service);

            // Still use old sync for typing + verification callbacks
            if let Some(tx) = SYNC_TX.get() {
                let bridge = ChannelSyncBridge { tx: tx.clone() };
                fluxchat_core::sync::spawn_sync(client.clone(), bridge);
            }
        }
        Err(e) => {
            tracing::warn!("SyncService failed ({e}), falling back to regular sync");

            // Fall back to regular sync
            if let Ok(mut rooms) = fluxchat_core::rooms::get_rooms(client).await {
                let read = ui.read_rooms.read().clone();
                for r in rooms.iter_mut() {
                    if read.contains(&r.room_id) { r.unread_count = 0; r.highlight_count = 0; }
                }
                ui.rooms.set(rooms);
            }
            if let Ok(spaces) = fluxchat_core::rooms::get_spaces(client).await { ui.spaces.set(spaces); }
            if let Ok(invited) = fluxchat_core::rooms::get_invited_rooms(client).await { ui.invited_rooms.set(invited); }

            if let Some(tx) = SYNC_TX.get() {
                let bridge = ChannelSyncBridge { tx: tx.clone() };
                fluxchat_core::sync::spawn_sync(client.clone(), bridge);
            }
        }
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
