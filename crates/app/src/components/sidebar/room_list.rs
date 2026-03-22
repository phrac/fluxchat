use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;

#[component]
pub fn RoomList() -> Element {
    let core = use_context::<Arc<AppState>>();
    let mut ui = use_context::<UiState>();
    let rooms = ui.rooms.read().clone();
    let selected = ui.selected_room_id.read().clone();
    let selected_space = ui.selected_space_id.read().clone();

    // Filter rooms by space
    let filtered: Vec<_> = if selected_space.is_some() {
        rooms.iter().filter(|r| r.space_name == selected_space.as_ref().and_then(|sid| {
            ui.spaces.read().iter().find(|s| s.room_id == *sid).map(|s| s.name.clone())
        })).cloned().collect()
    } else {
        rooms.clone()
    };

    let channels: Vec<_> = filtered.iter().filter(|r| !r.is_direct).cloned().collect();
    let dms: Vec<_> = filtered.iter().filter(|r| r.is_direct).cloned().collect();

    let header = selected_space.as_ref()
        .and_then(|sid| ui.spaces.read().iter().find(|s| s.room_id == *sid).map(|s| s.name.clone()))
        .unwrap_or_else(|| "fluxchat".to_string());

    rsx! {
        div { style: "width: 200px; flex-shrink: 0; background: var(--bg-dark); display: flex; flex-direction: column; border-right: 1px solid var(--border);",
            // Header
            div { style: "height: 40px; display: flex; align-items: center; padding: 0 12px; font-weight: 600; font-size: 14px; color: var(--text-bright); border-bottom: 1px solid var(--border); flex-shrink: 0;",
                "{header}"
            }

            // Room list
            div { style: "flex: 1; overflow-y: auto; padding: 8px 6px;",
                if !channels.is_empty() {
                    div { style: "color: var(--text-dim); font-size: 11px; font-weight: 700; text-transform: uppercase; padding: 4px 8px; letter-spacing: 0.5px;",
                        "Channels"
                    }
                    for room in channels.iter() {
                        { let room_id = room.room_id.clone();
                          let is_selected = selected.as_ref() == Some(&room.room_id);
                          let name = room.name.to_lowercase().replace(' ', "-");
                          let has_unread = room.unread_count > 0;
                          let bg = if is_selected { "var(--bg-light)" } else { "transparent" };
                          let color = if is_selected || has_unread { "var(--text-bright)" } else { "var(--text-muted)" };
                          let weight = if has_unread { "600" } else { "400" };
                          let style = format!("padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 13px; display: flex; align-items: center; gap: 4px; background: {bg}; color: {color}; font-weight: {weight};");
                          rsx! {
                            div {
                                style: "{style}",
                                onclick: {
                                    let core = core.clone();
                                    let ui = ui.clone();
                                    let rid = room_id.clone();
                                    let rname = room.name.clone();
                                    move |_| {
                                        select_room(core.clone(), ui.clone(), rid.clone(), rname.clone());
                                    }
                                },
                                span { style: "font-family: var(--font-mono); opacity: 0.5;",
                                    if room.is_encrypted { "🔒" } else { "#" }
                                }
                                span { "{name}" }
                                if room.unread_count > 0 {
                                    span { style: "margin-left: auto; background: var(--red); color: white; font-size: 10px; padding: 0 5px; border-radius: 8px; font-weight: 700;",
                                        "{room.unread_count}"
                                    }
                                }
                            }
                          }
                        }
                    }
                }

                if !dms.is_empty() {
                    div { style: "color: var(--text-dim); font-size: 11px; font-weight: 700; text-transform: uppercase; padding: 12px 8px 4px; letter-spacing: 0.5px;",
                        "Direct Messages"
                    }
                    for room in dms.iter() {
                        { let room_id = room.room_id.clone();
                          let is_selected = selected.as_ref() == Some(&room.room_id);
                          let bg = if is_selected { "var(--bg-light)" } else { "transparent" };
                          let color = if is_selected { "var(--text-bright)" } else { "var(--text-muted)" };
                          let style = format!("padding: 3px 8px; border-radius: 4px; cursor: pointer; font-size: 13px; background: {bg}; color: {color};");
                          rsx! {
                            div {
                                style: "{style}",
                                onclick: {
                                    let core = core.clone();
                                    let ui = ui.clone();
                                    let rid = room_id.clone();
                                    let rname = room.name.clone();
                                    move |_| {
                                        select_room(core.clone(), ui.clone(), rid.clone(), rname.clone());
                                    }
                                },
                                "{room.name}"
                            }
                          }
                        }
                    }
                }
            }

            // User panel
            div { style: "height: 48px; display: flex; align-items: center; gap: 8px; padding: 0 8px; border-top: 1px solid var(--border); flex-shrink: 0;",
                // Status dot
                div { style: "width: 8px; height: 8px; border-radius: 50%; background: var(--green); flex-shrink: 0;" }
                div { style: "flex: 1; min-width: 0;",
                    div { style: "font-size: 13px; font-weight: 500; color: var(--text-bright); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        {ui.display_name.read().as_ref().unwrap_or(&ui.user_id.read().split(':').next().unwrap_or("user").trim_start_matches('@').to_string()).clone()}
                    }
                    div { style: "font-size: 11px; color: var(--text-dim); font-family: var(--font-mono);",
                        {ui.user_id.read().split(':').nth(1).unwrap_or("").to_string()}
                    }
                }
            }
        }
    }
}

fn select_room(core: Arc<AppState>, mut ui: UiState, room_id: String, room_name: String) {
    ui.selected_room_id.set(Some(room_id.clone()));

    // Find space info for this room
    let space_name = ui.rooms.read().iter()
        .find(|r| r.room_id == room_id)
        .and_then(|r| r.space_name.clone());
    let space_avatar = space_name.as_ref().and_then(|sn| {
        ui.spaces.read().iter().find(|s| s.name == *sn).and_then(|s| s.avatar_url.clone())
    });

    // Add to tabs if not already there
    let mut tabs = ui.open_tabs.write();
    if !tabs.iter().any(|(id, _, _, _)| id == &room_id) {
        if tabs.len() >= 8 { tabs.remove(0); }
        tabs.push((room_id.clone(), room_name, space_name, space_avatar));
    }
    drop(tabs);

    // Load messages if needed
    let loaded = ui.messages_by_room.read().contains_key(&room_id);
    if !loaded {
        spawn(async move {
            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
            if let Ok(paginated) = fluxchat_core::messages::get_room_messages(&client, &room_id, None).await {
                let msgs: Vec<_> = paginated.messages.into_iter().rev().collect();
                ui.messages_by_room.write().insert(room_id.clone(), msgs);
            }
            if let Ok(members) = fluxchat_core::messages::get_room_members(&client, &room_id).await {
                ui.members_by_room.write().insert(room_id, members);
            }
        });
    }
}
