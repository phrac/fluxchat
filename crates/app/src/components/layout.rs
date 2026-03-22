use dioxus::prelude::*;

use crate::state::UiState;
use crate::components::sidebar::space_sidebar::SpaceSidebar;
use crate::components::sidebar::room_list::RoomList;
use crate::components::chat::message_area::MessageArea;
use crate::components::members::member_list::MemberList;

#[component]
pub fn MainLayout() -> Element {
    let mut ui = use_context::<UiState>();

    let user_id = ui.user_id.read().clone();
    let server = user_id.split(':').nth(1).unwrap_or("matrix.org");
    let selected_room = ui.selected_room_id.read().clone();
    let room_name = selected_room.as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).map(|r| r.name.clone()))
        .unwrap_or_default();
    let member_count = selected_room.as_ref()
        .and_then(|id| ui.members_by_room.read().get(id).map(|m| m.len()))
        .unwrap_or(0);
    let is_encrypted = selected_room.as_ref()
        .and_then(|id| ui.rooms.read().iter().find(|r| r.room_id == *id).map(|r| r.is_encrypted))
        .unwrap_or(false);

    rsx! {
        // Main content area
        div { style: "display: flex; flex: 1; min-height: 0;",
            // Space sidebar
            SpaceSidebar {}

            // Room list
            RoomList {}

            // Message area (with channel tabs)
            MessageArea {}

            // Member list
            if selected_room.is_some() {
                MemberList {}
            }
        }

        // Status bar — IRC style
        div { class: "status-bar",
            span { style: "font-family: var(--font-mono); color: var(--green);", "{user_id}" }
            span { class: "sep", "│" }
            if let Some(_) = &selected_room {
                span { "#{room_name}" }
                span { class: "sep", "│" }
                span { "{member_count} nicks" }
                span { class: "sep", "│" }
                if is_encrypted {
                    span { style: "color: var(--green);", "🔒 encrypted" }
                } else {
                    span { "unencrypted" }
                }
            } else {
                span { "no channel selected" }
            }
            span { class: "sep", "│" }
            span { style: "color: var(--green);", "synced" }
        }
    }
}
