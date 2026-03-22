use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::components::chat::message_input::MessageInput;

/// Nick color from a string hash.
pub fn nick_color_class(name: &str) -> String {
    let mut hash: u32 = 0;
    for b in name.bytes() { hash = b as u32 + ((hash << 5).wrapping_sub(hash)); }
    format!("nick-{}", (hash % 15))
}

#[component]
pub fn MessageArea() -> Element {
    let mut ui = use_context::<UiState>();
    let selected = ui.selected_room_id.read().clone();
    let tabs = ui.open_tabs.read().clone();

    rsx! {
        div { style: "flex: 1; display: flex; flex-direction: column; min-width: 0; background: var(--bg-mid);",

            // Channel tabs
            div { style: "display: flex; align-items: center; height: 36px; border-bottom: 1px solid var(--border); flex-shrink: 0; padding: 0 8px; gap: 0; overflow-x: auto; background: var(--bg-dark);",
                for (id, name, space, space_mxc) in tabs.iter() {
                    ChannelTab {
                        room_id: id.clone(),
                        name: name.clone(),
                        space_name: space.clone(),
                        space_avatar_mxc: space_mxc.clone(),
                        is_active: selected.as_ref() == Some(id),
                    }
                }
            }

            // Messages
            if let Some(room_id) = &selected {
                { let messages = ui.messages_by_room.read().get(room_id).cloned().unwrap_or_default();
                  let msg_count = messages.len();

                  // Auto-scroll to bottom when messages change
                  use_effect(move || {
                      let _ = msg_count; // track dependency
                      document::eval(r#"
                          setTimeout(() => {
                              let el = document.getElementById('msg-scroll');
                              if (el) el.scrollTop = el.scrollHeight;
                          }, 10);
                      "#);
                  });

                  rsx! {
                    div { id: "msg-scroll", style: "flex: 1; overflow-y: auto; padding: 8px 12px;",
                        if messages.is_empty() {
                            div { style: "display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-dim);",
                                "No messages yet"
                            }
                        }
                        for msg in messages.iter() {
                            { let is_system = msg.event_id.starts_with("~state-") || msg.event_id.starts_with("~system-");
                              if is_system {
                                rsx! {
                                    div { style: "padding: 2px 0; font-size: 12px; color: var(--text-dim); font-family: var(--font-mono);",
                                        span { style: "color: var(--green);", "--> " }
                                        "{msg.body}"
                                    }
                                }
                              } else {
                                let nick_class = nick_color_class(&msg.sender);
                                let time = {
                                    let ts = msg.timestamp;
                                    let secs = ts % 86400;
                                    let h = secs / 3600;
                                    let m = (secs % 3600) / 60;
                                    format!("{:02}:{:02}", h, m)
                                };
                                rsx! {
                                    div { style: "display: flex; gap: 6px; padding: 1px 0; align-items: baseline;",
                                        // Timestamp
                                        span { class: "msg-time", "{time}" }
                                        // Nick
                                        span { class: "{nick_class}", style: "font-weight: 600; font-family: var(--font-mono); font-size: 13px; flex-shrink: 0;",
                                            "{msg.sender_name}"
                                        }
                                        // Message body
                                        if let Some(ref html) = msg.formatted_body {
                                            span { class: "msg-body", dangerous_inner_html: "{html}" }
                                        } else {
                                            span { class: "msg-body", "{msg.body}" }
                                        }
                                        // Reactions inline
                                        for r in msg.reactions.iter() {
                                            span { style: "font-size: 11px; background: rgba(59,130,246,0.15); border-radius: 8px; padding: 0 4px; color: var(--text-muted); flex-shrink: 0;",
                                                "{r.emoji} {r.count}"
                                            }
                                        }
                                    }
                                }
                              }
                            }
                        }
                    }
                  }
                }
            } else {
                div { style: "flex: 1; display: flex; align-items: center; justify-content: center; color: var(--text-dim);",
                    "Select a channel to start chatting"
                }
            }

            // Typing indicator
            { let typing = selected.as_ref()
                .and_then(|id| ui.typing_by_room.read().get(id).cloned())
                .unwrap_or_default();
              if !typing.is_empty() {
                let names: Vec<_> = typing.iter().map(|u| u.split(':').next().unwrap_or(u).trim_start_matches('@').to_string()).collect();
                let text = if names.len() == 1 { format!("{} is typing...", names[0]) }
                    else { format!("{} are typing...", names.join(", ")) };
                rsx! {
                    div { style: "padding: 2px 12px; font-size: 12px; color: var(--text-dim); font-family: var(--font-mono); flex-shrink: 0;",
                        "{text}"
                    }
                }
              } else { rsx! {} }
            }

            // Message input
            if selected.is_some() {
                MessageInput {}
            }
        }
    }
}

#[component]
fn ChannelTab(room_id: String, name: String, space_name: Option<String>, space_avatar_mxc: Option<String>, is_active: bool) -> Element {
    let mut ui = use_context::<UiState>();
    let core = use_context::<Arc<AppState>>();

    let tab_class = if is_active { "channel-tab active" } else { "channel-tab" };
    let label = if let Some(ref s) = space_name {
        format!("#{name}")
    } else {
        format!("#{name}")
    };

    // Resolve space avatar
    let avatar_src = use_resource({
        let mxc = space_avatar_mxc.clone();
        let core = core.clone();
        move || {
            let mxc = mxc.clone();
            let core = core.clone();
            async move {
                if let Some(mxc) = mxc {
                    let client = core.get_client().await.ok()?;
                    fluxchat_core::media::get_media_url(&client, &core.db, &core.media_cache, &mxc).await.ok()
                } else { None }
            }
        }
    });
    let has_space_avatar = avatar_src.read().as_ref().and_then(|v| v.as_ref()).is_some();

    let tab_id = room_id.clone();
    let close_id = room_id.clone();

    rsx! {
        div {
            class: "{tab_class}",
            style: "display: flex; align-items: center; gap: 4px;",
            onclick: move |_| { ui.selected_room_id.set(Some(tab_id.clone())); },

            // Space avatar
            if has_space_avatar {
                img {
                    src: "{avatar_src.read().as_ref().and_then(|v| v.as_ref()).unwrap()}",
                    style: "width: 14px; height: 14px; border-radius: 3px; object-fit: cover;",
                }
            } else if space_name.is_some() {
                span { style: "font-size: 10px; opacity: 0.5; font-family: var(--font-mono);",
                    {space_name.as_ref().map(|s| s.chars().take(2).collect::<String>().to_uppercase()).unwrap_or_default()}
                }
            }

            "{label}"

            span {
                style: "font-size: 10px; opacity: 0.4; cursor: pointer; padding: 0 2px;",
                onclick: {
                    let cid = close_id.clone();
                    move |e: Event<MouseData>| {
                        e.stop_propagation();
                        let mut tabs = ui.open_tabs.write();
                        tabs.retain(|(id, _, _, _)| id != &cid);
                        if ui.selected_room_id.read().as_ref() == Some(&cid) {
                            let next = tabs.last().map(|(id, _, _, _)| id.clone());
                            drop(tabs);
                            ui.selected_room_id.set(next);
                        }
                    }
                },
                "×"
            }
        }
    }
}
