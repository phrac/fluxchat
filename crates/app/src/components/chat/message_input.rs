use dioxus::prelude::*;
use std::sync::Arc;
use fluxchat_core::AppState;

use crate::state::UiState;
use crate::components::chat::emoji_picker::EmojiPicker;
use crate::components::chat::commands::{self, CommandAction};

/// Build a RoomMessageEventContent, resolving display names to Matrix mention links.
fn build_message_content(body: &str, member_map: &std::collections::HashMap<String, String>) -> matrix_sdk::ruma::events::room::message::RoomMessageEventContent {
    use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;

    // Check if any member names appear in the body
    let mut has_mentions = false;
    let mut html = body.to_string();
    let mut plain = body.to_string();

    // Sort by name length descending to replace longer names first (avoid partial matches)
    let mut names: Vec<_> = member_map.iter().collect();
    names.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (display_name, user_id) in &names {
        if display_name.len() > 1 && body.contains(display_name.as_str()) {
            has_mentions = true;
            let mention_html = format!(
                r#"<a href="https://matrix.to/#/{user_id}">{display_name}</a>"#
            );
            html = html.replace(display_name.as_str(), &mention_html);
        }
    }

    if has_mentions {
        RoomMessageEventContent::text_html(&plain, &html)
    } else {
        RoomMessageEventContent::text_plain(body)
    }
}

const ICO_PLUS: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;
const ICO_BOLD: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 4h8a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"/><path d="M6 12h9a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"/></svg>"#;
const ICO_ITALIC: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="19" y1="4" x2="10" y2="4"/><line x1="14" y1="20" x2="5" y2="20"/><line x1="15" y1="4" x2="9" y2="20"/></svg>"#;
const ICO_STRIKE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4H9a3 3 0 0 0-3 3c0 2 1.5 3 3 3h6"/><path d="M4 12h16"/><path d="M15 12a3 3 0 0 1-3 3H8"/></svg>"#;
const ICO_CODE: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>"#;
const ICO_CODEBLOCK: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><polyline points="10 8 6 12 10 16"/><polyline points="14 8 18 12 14 16"/></svg>"#;
const ICO_LINK: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>"#;
const ICO_EMOJI: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2"/><line x1="9" y1="9" x2="9.01" y2="9"/><line x1="15" y1="9" x2="15.01" y2="9"/></svg>"#;

#[component]
pub fn MessageInput() -> Element {
    let core = use_context::<Arc<AppState>>();
    let core_media = core.clone();
    let mut ui = use_context::<UiState>();
    let mut text = use_signal(|| String::new());
    let mut sending = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut typing_sent = use_signal(|| false);
    let mut emoji_open = use_signal(|| false);
    // Use shared help_text signal so async commands can update it too

    let reply_to = ui.reply_to.read().clone();
    let edit_event = ui.edit_event.read().clone();
    let upload_pending = ui.upload_pending.read().clone();

    // When entering edit mode, populate the input with the original text
    use_effect(move || {
        let edit = ui.edit_event.read().clone();
        if let Some((_, ref body)) = edit {
            text.set(body.clone());
        }
    });

    let mut ui_cancel = ui.clone();
    let mut ui_cancel2 = ui.clone();
    let ui_cancel3 = ui.clone();
    let ui_upload = ui.clone();
    let ui_typing = ui.clone();
    let core_typing = core.clone();

    // Upload handler
    let handle_upload = {
        let core = core.clone();
        move |filename: String, mime: String, data: Vec<u8>| {
            let room_id = match ui_upload.selected_room_id.read().clone() { Some(id) => id, None => return };
            let core = core.clone();
            spawn(async move {
                uploading.set(true);
                let client = match core.get_client().await { Ok(c) => c, Err(_) => { uploading.set(false); return; } };
                let _ = fluxchat_core::messages::upload_file(&client, &room_id, &filename, &mime, data).await;
                uploading.set(false);
            });
        }
    };

    let mut handle_send = move |_: ()| {
        let mut body = text().trim().to_string();

        // Handle slash commands via modular registry
        if body.starts_with('/') {
            let room_id = ui.selected_room_id.read().clone();
            match commands::dispatch(&body, room_id.as_deref(), core.clone(), ui.clone()) {
                CommandAction::Handled => {
                    text.set(String::new());
                    ui.help_text.set(None);
                    return;
                }
                CommandAction::Rewrite(new_body) => {
                    body = new_body;
                    ui.help_text.set(None);
                    // Fall through to normal send
                }
                CommandAction::ShowHelp(msg) => {
                    text.set(String::new());
                    ui.help_text.set(Some(msg));
                    return;
                }
                CommandAction::Noop => {
                    // Unknown or incomplete command — send as plain text
                }
            }
        }

        let body = body; // rebind as immutable
        let pending_upload = {
            let val = ui.upload_pending.read().clone();
            val
        };
        if body.is_empty() && pending_upload.is_none() || *sending.read() { return; }
        let raw_room_id = match ui.selected_room_id.read().clone() { Some(id) => id, None => return };

        // Resolve thread tab IDs to real room ID + thread root
        let (room_id, thread_root_eid) = if raw_room_id.starts_with("thread::") {
            let parts: Vec<&str> = raw_room_id.splitn(3, "::").collect();
            if parts.len() == 3 {
                (parts[1].to_string(), Some(parts[2].to_string()))
            } else { return; }
        } else {
            (raw_room_id.clone(), None)
        };

        // Build display_name → user_id map for mention resolution
        let member_map: std::collections::HashMap<String, String> = {
            let real_rid = if room_id.starts_with("thread::") {
                room_id.splitn(3, "::").nth(1).unwrap_or(&room_id).to_string()
            } else { room_id.clone() };
            ui.members_by_room.peek()
                .get(&real_rid)
                .map(|members| members.iter()
                    .map(|m| (m.display_name.clone(), m.user_id.clone()))
                    .collect())
                .unwrap_or_default()
        };

        let core = core.clone();
        let reply = ui.reply_to.read().clone();
        let edit = ui.edit_event.read().clone();

        text.set(String::new());
        ui.reply_to.set(None);
        ui.edit_event.set(None);
        ui.upload_pending.set(None);
        typing_sent.set(false);
        // Stop typing indicator (skip for thread tabs)
        if !room_id.starts_with("thread::") {
            let core = core.clone();
            let room_id = room_id.clone();
            spawn(async move {
                let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                let _ = fluxchat_core::messages::send_typing(&client, &room_id, false).await;
            });
        }

        // If there's a file pending, upload it (and send text after if any)
        if let Some((filename, mime, data)) = pending_upload {
            let core2 = core.clone();
            let body2 = body.clone();
            let room_id2 = room_id.clone();
            spawn(async move {
                sending.set(true);
                let client = match core2.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::upload_file(&client, &room_id2, &filename, &mime, data).await;
                // Also send text message if user typed something
                if !body2.is_empty() {
                    let _ = fluxchat_core::messages::send_message(&client, &room_id2, &body2).await;
                }
                sending.set(false);
            });
            return;
        }

        // Use Timeline API for send/reply/edit — handles local echo automatically
        if let Some((event_id, _)) = edit {
            let body = body.clone();
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let timelines = core.timelines.lock().await;
                    if let Some(timeline) = timelines.get(&*rid) {
                        if let Ok(eid) = fluxchat_core::ruma::EventId::parse(&event_id) {
                            use matrix_sdk::room::edit::EditedContent;
                            use matrix_sdk::ruma::events::room::message::RoomMessageEventContentWithoutRelation;
                            let content = RoomMessageEventContentWithoutRelation::text_plain(&body);
                            let item_id = matrix_sdk_ui::timeline::TimelineEventItemId::EventId(eid);
                            let _ = timeline.edit(&item_id, EditedContent::RoomMessage(content)).await;
                        }
                    } else {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                        let _ = fluxchat_core::messages::edit_message(&client, &room_id, &event_id, &body).await;
                    }
                }
                sending.set(false);
            });
        } else if let Some((event_id, _, _)) = reply {
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let timelines = core.timelines.lock().await;
                    if let Some(timeline) = timelines.get(&*rid) {
                        if let Ok(eid) = fluxchat_core::ruma::EventId::parse(&event_id) {
                            use matrix_sdk::ruma::events::room::message::RoomMessageEventContentWithoutRelation;
                            let content = RoomMessageEventContentWithoutRelation::text_plain(&body);
                            let _ = timeline.send_reply(content, eid).await;
                        }
                    } else {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                        let _ = fluxchat_core::messages::send_reply(&client, &room_id, &body, &event_id).await;
                    }
                }
                sending.set(false);
            });
        } else if let Some(ref thread_root) = thread_root_eid {
            // Thread reply — send with m.thread relation
            let thread_root = thread_root.clone();
            let raw_room_id = raw_room_id.clone();
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                    let room = client.get_room(&rid);
                    if let Some(room) = room {
                        if let Ok(root_eid) = fluxchat_core::ruma::EventId::parse(&thread_root) {
                            use matrix_sdk::ruma::events::room::message::Relation;
                            use matrix_sdk::ruma::events::relation::Thread;
                            let mut content = build_message_content(&body, &member_map);
                            content.relates_to = Some(Relation::Thread(
                                Thread::plain(root_eid.clone(), root_eid)
                            ));
                            let _ = room.send(content).await;

                            // Update thread tab messages from room's latest state
                            let mut ui = consume_context::<UiState>();
                            let thread_msgs = {
                                let msgs = ui.messages_by_room.read();
                                msgs.get(&room_id).map(|m| m.iter()
                                    .filter(|m| m.event_id == thread_root || m.thread_root.as_deref() == Some(&thread_root))
                                    .cloned()
                                    .collect::<Vec<_>>())
                            };
                            if let Some(thread_msgs) = thread_msgs {
                                ui.messages_by_room.write().insert(raw_room_id, thread_msgs);
                            }
                        }
                    }
                }
                sending.set(false);
            });
        } else {
            // Normal send via Timeline
            spawn(async move {
                sending.set(true);
                if let Ok(rid) = fluxchat_core::ruma::RoomId::parse(&room_id) {
                    let timelines = core.timelines.lock().await;
                    if let Some(timeline) = timelines.get(&*rid) {
                        let content = build_message_content(&body, &member_map);
                        let _ = timeline.send(content.into()).await;
                    } else {
                        let client = match core.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                        let _ = fluxchat_core::messages::send_message(&client, &room_id, &body).await;
                    }
                }
                sending.set(false);
            });
        }
    };

    // Prompt prefix changes based on mode
    let (prefix, prefix_color) = if edit_event.is_some() {
        ("ed\u{276f}", "var(--yellow)")
    } else if reply_to.is_some() {
        ("re\u{276f}", "var(--green)")
    } else {
        ("\u{276f}", "var(--accent-muted)")
    };

    // Format button helper — wraps selection or appends markup
    let make_fmt_handler = |pfx: &'static str, sfx: &'static str| {
        move |_| {
            let pfx = pfx.to_string();
            let sfx = sfx.to_string();
            spawn(async move {
                let mut eval = document::eval(r#"
                    let ta = document.getElementById('flux-input');
                    if (ta) { dioxus.send(ta.selectionStart + ',' + ta.selectionEnd); }
                    else { dioxus.send('-1,-1'); }
                "#);
                if let Ok(data) = eval.recv::<String>().await {
                    let parts: Vec<&str> = data.split(',').collect();
                    if parts.len() == 2 {
                        let start: usize = parts[0].parse().unwrap_or(0);
                        let end: usize = parts[1].parse().unwrap_or(0);
                        let current = text();
                        if start != end && start < current.len() && end <= current.len() {
                            let before = &current[..start];
                            let selected = &current[start..end];
                            let after = &current[end..];
                            text.set(format!("{before}{pfx}{selected}{sfx}{after}"));
                        } else {
                            text.set(format!("{current}{pfx}{sfx}"));
                        }
                    }
                }
            });
        }
    };

    rsx! {
        div { style: "flex-shrink: 0;",
            // Reply/edit context bar
            if let Some((_, ref name, ref body)) = reply_to {
                div { style: "display: flex; align-items: center; padding: 4px 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--green); margin: 0 12px;",
                    span { style: "color: var(--green); margin-right: 6px;", "replying to {name}:" }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        { body.chars().take(80).collect::<String>() }
                    }
                    span { style: "cursor: pointer; padding: 0 4px; color: var(--text-dim);",
                        onclick: move |_| { ui_cancel.reply_to.set(None); },
                        "×"
                    }
                }
            }
            if let Some((_, ref body)) = edit_event {
                div { style: "display: flex; align-items: center; padding: 4px 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--yellow); margin: 0 12px;",
                    span { style: "color: var(--yellow); margin-right: 6px;", "editing:" }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        { body.chars().take(80).collect::<String>() }
                    }
                    span { style: "cursor: pointer; padding: 0 4px; color: var(--text-dim);",
                        onclick: move |_| { ui_cancel2.edit_event.set(None); text.set(String::new()); },
                        "×"
                    }
                }
            }
            // Upload preview bar
            if let Some((ref filename, _, _)) = upload_pending {
                div { style: "display: flex; align-items: center; padding: 4px 16px; font-size: 12px; font-family: var(--font-mono); color: var(--text-dim); background: var(--bg-deep); border-left: 2px solid var(--accent); margin: 0 12px;",
                    span { style: "color: var(--accent); margin-right: 6px;",
                        if *uploading.read() { "uploading:" } else { "attach:" }
                    }
                    span { style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                        "{filename}"
                    }
                    if !*uploading.read() {
                        span { style: "cursor: pointer; padding: 0 6px; color: var(--green); font-weight: 600;",
                            onclick: {
                                let handle_upload = handle_upload.clone();
                                move |_| {
                                    let mut ui = consume_context::<UiState>();
                                    let pending = {
                                        let val = ui.upload_pending.read().clone();
                                        val
                                    };
                                    if let Some((filename, mime, data)) = pending {
                                        ui.upload_pending.set(None);
                                        handle_upload(filename, mime, data);
                                    }
                                }
                            },
                            "send"
                        }
                        span { style: "cursor: pointer; padding: 0 4px; color: var(--text-dim);",
                            onclick: move |_| {
                                consume_context::<UiState>().upload_pending.set(None);
                            },
                            "×"
                        }
                    }
                }
            }
            // Mention preview — shows who you're referencing
            { let preview_text = text();
              let preview_ui = consume_context::<UiState>();
              let sel_rid = preview_ui.selected_room_id.peek().clone();
              let real_rid = sel_rid.as_ref().map(|r| {
                  if r.starts_with("thread::") { r.splitn(3, "::").nth(1).unwrap_or(r).to_string() } else { r.clone() }
              });
              let mentioned_member = real_rid.and_then(|rid| {
                  preview_ui.members_by_room.peek().get(&rid).and_then(|members| {
                      // Find a member whose display name appears in the input (after a space or command)
                      members.iter().find(|m| {
                          let name = &m.display_name;
                          !name.is_empty() && name.len() > 1 && preview_text.contains(name)
                      }).map(|m| (m.display_name.clone(), m.user_id.clone(), m.avatar_url.clone()))
                  })
              });
              if let Some((ref name, ref uid, ref mxc)) = mentioned_member {
                  let avatar = mxc.as_ref().and_then(|m| core_media.media_cache.lock().unwrap().get(m).cloned());
                  let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
                  rsx! {
                      div { style: "display: flex; align-items: center; gap: 6px; padding: 3px 16px; font-size: 11px; font-family: var(--font-mono); color: var(--text-dim); margin: 0 12px;",
                          if let Some(ref src) = avatar {
                              img { src: "{src}", style: "width: 16px; height: 16px; border-radius: 50%; object-fit: cover;" }
                          } else {
                              div { style: "width: 16px; height: 16px; border-radius: 50%; background: var(--bg-hover); display: flex; align-items: center; justify-content: center; font-size: 8px; font-weight: 600; color: var(--text-muted);",
                                  "{initial}"
                              }
                          }
                          span { style: "color: var(--text-muted);", "{name}" }
                          span { style: "color: var(--text-dim);", "{uid}" }
                      }
                  }
              } else { rsx! {} }
            }
            // Slash command autocomplete popup
            { let current_text = text();
              let show_cmds = current_text.starts_with('/') && !current_text.contains(' ');
              let filtered: Vec<(String, String, String)> = if show_cmds {
                  commands::autocomplete_entries().into_iter()
                      .filter(|(name, _, _)| name.starts_with(&current_text[..]) || current_text == "/")
                      .collect()
              } else { Vec::new() };

              // User @mention autocomplete — trigger on last word starting with @
              let mention_ui = consume_context::<UiState>();
              let at_query = current_text.rsplit_once(' ')
                  .map(|(_, last)| last)
                  .unwrap_or(&current_text);
              let show_mentions = at_query.starts_with('@') && at_query.len() > 1 && !show_cmds;
              // (display_name, user_id, avatar_mxc)
              let mention_matches: Vec<(String, String, Option<String>)> = if show_mentions {
                  let needle = at_query[1..].to_lowercase();
                  let sel = mention_ui.selected_room_id.peek().clone();
                  if let Some(ref rid) = sel {
                      let real_rid = if rid.starts_with("thread::") {
                          rid.splitn(3, "::").nth(1).unwrap_or(rid).to_string()
                      } else { rid.clone() };
                      mention_ui.members_by_room.peek()
                          .get(&real_rid)
                          .map(|members| members.iter()
                              .filter(|m| m.display_name.to_lowercase().contains(&needle)
                                  || m.user_id.to_lowercase().contains(&needle))
                              .take(8)
                              .map(|m| (m.display_name.clone(), m.user_id.clone(), m.avatar_url.clone()))
                              .collect())
                          .unwrap_or_default()
                  } else { Vec::new() }
              } else { Vec::new() };

              if !filtered.is_empty() {
                rsx! {
                    div { style: "margin: 0 12px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 6px 6px 0 0; border-bottom: none; max-height: 200px; overflow-y: auto; padding: 4px 0;",
                        for (name, usage, desc) in filtered.iter() {
                            {
                              let cmd_text = format!("{name} {usage}");
                              let name = name.clone();
                              rsx! {
                                div {
                                    class: "cmd-item",
                                    style: "display: flex; align-items: center; gap: 8px; padding: 4px 12px; cursor: pointer;",
                                    onclick: move |_| {
                                        text.set(format!("{name} "));
                                    },
                                    span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--accent-muted); font-weight: 600; flex-shrink: 0;",
                                        "{cmd_text}"
                                    }
                                    span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim);",
                                        "{desc}"
                                    }
                                }
                            } }
                        }
                    }
                }
              } else if !mention_matches.is_empty() {
                rsx! {
                    div { style: "margin: 0 12px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 6px 6px 0 0; border-bottom: none; max-height: 200px; overflow-y: auto; padding: 4px 0;",
                        { let cache = core_media.media_cache.lock().unwrap();
                          let resolved: Vec<_> = mention_matches.iter().map(|(dn, uid, mxc)| {
                              let avatar = mxc.as_ref().and_then(|m| cache.get(m).cloned());
                              (dn.clone(), uid.clone(), avatar)
                          }).collect();
                          drop(cache);
                        rsx! {
                        for (display_name, user_id, avatar_src) in resolved.iter() {
                            { let display = display_name.clone();
                              let at_q = at_query.to_string();
                              let initial = display_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                              rsx! {
                                div {
                                    class: "cmd-item",
                                    style: "display: flex; align-items: center; gap: 8px; padding: 4px 12px; cursor: pointer;",
                                    onclick: move |_| {
                                        let current = text();
                                        let before = if let Some(idx) = current.rfind(&at_q) {
                                            &current[..idx]
                                        } else { &current };
                                        text.set(format!("{before}{display} "));
                                    },
                                    if let Some(ref src) = avatar_src {
                                        img { src: "{src}", style: "width: 18px; height: 18px; border-radius: 50%; object-fit: cover; flex-shrink: 0;" }
                                    } else {
                                        div { style: "width: 18px; height: 18px; border-radius: 50%; background: var(--bg-hover); display: flex; align-items: center; justify-content: center; font-size: 9px; font-weight: 600; color: var(--text-muted); flex-shrink: 0;",
                                            "{initial}"
                                        }
                                    }
                                    span { style: "font-family: var(--font-mono); font-size: 13px; color: var(--text-bright); font-weight: 600;",
                                        "{display_name}"
                                    }
                                    span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--text-dim);",
                                        "{user_id}"
                                    }
                                }
                            } }
                        }
                        } }
                    }
                }
              } else { rsx! {} }
            }
            // Help text display
            { let help_ui = use_context::<UiState>();
              let help_val = help_ui.help_text.read().clone();
              if let Some(ref msg) = help_val {
                rsx! {
                div { style: "margin: 0 12px 4px; background: var(--bg-deep); border: 1px solid var(--border); border-radius: 4px; padding: 8px 12px; font-family: var(--font-mono); font-size: 12px; color: var(--text-muted); white-space: pre-wrap; line-height: 1.6; position: relative;",
                    span {
                        style: "position: absolute; top: 4px; right: 8px; cursor: pointer; color: var(--text-dim); font-size: 14px;",
                        onclick: move |_| { consume_context::<UiState>().help_text.set(None); },
                        "×"
                    }
                    "{msg}"
                }
                }
              } else { rsx! {} }
            }

            // ── The command bar ──────────────────────────────────────
            div { style: "margin: 0 12px 8px; border-radius: 8px; background: var(--bg-deep); border: 1px solid var(--border); transition: border-color 0.15s, box-shadow 0.15s;",
                class: "command-bar",

                // Input row
                div { style: "display: flex; align-items: center; min-height: 44px; padding: 0 4px;",
                    // Attach button
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 6px 8px; display: inline-flex; align-items: center; flex-shrink: 0; border-radius: 4px; transition: color 0.1s, background 0.1s;",
                        title: "Attach file",
                        onclick: move |_| {
                            spawn(async move {
                                let mut eval = document::eval(r#"
                                    let input = document.createElement('input');
                                    input.type = 'file';
                                    input.accept = 'image/*,video/*,audio/*,application/*';
                                    input.onchange = async () => {
                                        if (input.files.length > 0) {
                                            let file = input.files[0];
                                            let reader = new FileReader();
                                            reader.onload = () => {
                                                let base64 = reader.result.split(',')[1];
                                                dioxus.send(JSON.stringify({
                                                    name: file.name,
                                                    type: file.type || 'application/octet-stream',
                                                    data: base64
                                                }));
                                            };
                                            reader.readAsDataURL(file);
                                        }
                                    };
                                    input.click();
                                "#);
                                if let Ok(json) = eval.recv::<String>().await {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                                        let name = parsed["name"].as_str().unwrap_or("file").to_string();
                                        let mime = parsed["type"].as_str().unwrap_or("application/octet-stream").to_string();
                                        if let Some(b64) = parsed["data"].as_str() {
                                            use base64::Engine;
                                            if let Ok(data) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                                let mut ui = consume_context::<UiState>();
                                                ui.upload_pending.set(Some((name, mime, data)));
                                            }
                                        }
                                    }
                                }
                            });
                        },
                        dangerous_inner_html: ICO_PLUS,
                    }

                    // Prompt
                    span { style: "color: {prefix_color}; font-family: var(--font-mono); font-size: 18px; font-weight: 700; flex-shrink: 0; padding: 0 6px; line-height: 1;", "{prefix}" }

                    // Text input
                    input {
                        id: "flux-input",
                        style: "flex: 1; background: transparent; border: none; outline: none; color: var(--text-bright); font-size: 14px; font-family: var(--font-mono); padding: 8px 0; min-width: 0;",
                        r#type: "text",
                        placeholder: "Type a message or /command...",
                        value: "{text}",
                        oninput: {
                            let core = core_typing.clone();
                            let ui = ui_typing.clone();
                            move |e: Event<FormData>| {
                                let val = e.value();
                                text.set(val.clone());
                                if !val.is_empty() && !*typing_sent.read() {
                                    typing_sent.set(true);
                                    let core = core.clone();
                                    if let Some(room_id) = ui.selected_room_id.peek().clone() {
                                        spawn(async move {
                                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                                            let _ = fluxchat_core::messages::send_typing(&client, &room_id, true).await;
                                        });
                                    }
                                } else if val.is_empty() && *typing_sent.read() {
                                    typing_sent.set(false);
                                    let core = core.clone();
                                    if let Some(room_id) = ui.selected_room_id.peek().clone() {
                                        spawn(async move {
                                            let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                                            let _ = fluxchat_core::messages::send_typing(&client, &room_id, false).await;
                                        });
                                    }
                                }
                            }
                        },
                        onkeydown: {
                            let mut ui = ui_cancel3.clone();
                            move |e: Event<KeyboardData>| {
                                let val = text();
                                let in_autocomplete = val.starts_with('/') && !val.contains(' ');

                                if e.key() == Key::Tab {
                                    // Slash command tab completion
                                    if in_autocomplete {
                                        let all = commands::autocomplete_entries();
                                        let filtered: Vec<_> = all.iter()
                                            .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                            .collect();
                                        if !filtered.is_empty() {
                                            text.set(format!("{} ", filtered[0].0));
                                        }
                                        return;
                                    }
                                    // @mention tab completion
                                    let last_word = val.rsplit_once(' ').map(|(_, w)| w).unwrap_or(&val);
                                    if last_word.starts_with('@') && last_word.len() > 1 {
                                        let needle = last_word[1..].to_lowercase();
                                        let mention_ui = consume_context::<UiState>();
                                        let sel = mention_ui.selected_room_id.peek().clone();
                                        if let Some(ref rid) = sel {
                                            let real_rid = if rid.starts_with("thread::") {
                                                rid.splitn(3, "::").nth(1).unwrap_or(rid).to_string()
                                            } else { rid.clone() };
                                            if let Some(members) = mention_ui.members_by_room.peek().get(&real_rid) {
                                                if let Some(m) = members.iter().find(|m|
                                                    m.display_name.to_lowercase().starts_with(&needle)
                                                    || m.user_id.to_lowercase().contains(&needle)
                                                ) {
                                                    let before = if let Some(idx) = val.rfind(last_word) {
                                                        &val[..idx]
                                                    } else { "" };
                                                    text.set(format!("{before}{} ", m.display_name));
                                                }
                                            }
                                        }
                                        return;
                                    }
                                }

                                if e.key() == Key::Enter && !e.modifiers().shift() {
                                    if in_autocomplete {
                                        let all = commands::autocomplete_entries();
                                        let filtered: Vec<_> = all.iter()
                                            .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                            .collect();
                                        if filtered.len() == 1 {
                                            text.set(format!("{} ", filtered[0].0));
                                            return;
                                        }
                                    }
                                    handle_send(());
                                } else if e.key() == Key::Escape {
                                    ui.reply_to.set(None);
                                    ui.edit_event.set(None);
                                    text.set(String::new());
                                    ui.help_text.set(None);
                                }
                            }
                        },
                    }

                    // Emoji button
                    div { style: "position: relative; flex-shrink: 0;",
                        span {
                            class: "cmd-btn",
                            style: "color: var(--text-dim); cursor: pointer; padding: 6px 8px; display: inline-flex; align-items: center; border-radius: 4px; transition: color 0.1s, background 0.1s;",
                            title: "Emoji",
                            onclick: move |_| { let v = *emoji_open.read(); emoji_open.set(!v); },
                            dangerous_inner_html: ICO_EMOJI,
                        }
                        if *emoji_open.read() {
                            div { style: "position: absolute; bottom: 100%; right: 0; margin-bottom: 8px; z-index: 100;",
                                EmojiPicker {
                                    on_select: move |emoji: String| {
                                        let current = text();
                                        text.set(format!("{current}{emoji}"));
                                        emoji_open.set(false);
                                    },
                                }
                            }
                        }
                    }
                }

                // Formatting toolbar — inside the bar, subtle divider
                div { style: "display: flex; align-items: center; gap: 1px; padding: 0 8px 4px; border-top: 1px solid rgba(255,255,255,0.03);",
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Bold",
                        onclick: make_fmt_handler("**", "**"),
                        dangerous_inner_html: ICO_BOLD,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Italic",
                        onclick: make_fmt_handler("*", "*"),
                        dangerous_inner_html: ICO_ITALIC,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Strikethrough",
                        onclick: make_fmt_handler("~~", "~~"),
                        dangerous_inner_html: ICO_STRIKE,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Inline code",
                        onclick: make_fmt_handler("`", "`"),
                        dangerous_inner_html: ICO_CODE,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Code block",
                        onclick: make_fmt_handler("```\n", "\n```"),
                        dangerous_inner_html: ICO_CODEBLOCK,
                    }
                    span {
                        class: "cmd-btn",
                        style: "color: var(--text-dim); cursor: pointer; padding: 3px 6px; display: inline-flex; align-items: center; border-radius: 3px; transition: color 0.1s, background 0.1s;",
                        title: "Link",
                        onclick: make_fmt_handler("[", "](url)"),
                        dangerous_inner_html: ICO_LINK,
                    }
                }
            }
        }
    }
}
