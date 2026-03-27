use dioxus::prelude::*;
use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;
use fluxchat_core::AppState;

use crate::state::{UiState, html_escape};
use crate::components::chat::emoji_picker::EmojiPicker;
use crate::components::chat::commands::{self, CommandAction};

/// Replace matched pairs of `delim` with HTML tags. Processes left-to-right, non-greedy.
fn replace_delimited(input: &str, delim: &str, open_tag: &str, close_tag: &str) -> (String, bool) {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    let mut changed = false;
    while let Some(start) = rest.find(delim) {
        let after_open = start + delim.len();
        if after_open >= rest.len() { break; }
        if let Some(end) = rest[after_open..].find(delim) {
            if end == 0 { // empty content like **** — skip
                out.push_str(&rest[..after_open]);
                rest = &rest[after_open..];
                continue;
            }
            let inner = &rest[after_open..after_open + end];
            out.push_str(&rest[..start]);
            out.push_str(open_tag);
            out.push_str(inner);
            out.push_str(close_tag);
            rest = &rest[after_open + end + delim.len()..];
            changed = true;
        } else {
            break;
        }
    }
    out.push_str(rest);
    (out, changed)
}

/// Simple inline markdown → HTML for formatting buttons: **bold**, *italic*, ~~strike~~, `code`, ```codeblock```
fn simple_markdown_to_html(input: &str) -> Option<String> {
    let mut out = input.to_string();
    let mut changed = false;

    // Code blocks: ```lang\n...\n``` or ```\n...\n``` or ```...```
    {
        let mut result = String::new();
        let mut src = out.as_str();
        let mut found = false;
        while let Some(start) = src.find("```") {
            result.push_str(&src[..start]);
            let after = &src[start + 3..];
            // Extract optional language tag (up to first newline or `)
            let (lang, code_start) = if let Some(nl) = after.find('\n') {
                let tag = after[..nl].trim();
                if !tag.is_empty() && tag.len() < 20 && tag.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '+' || c == '#') {
                    (Some(tag.to_string()), nl + 1)
                } else if tag.is_empty() {
                    (None, nl + 1)
                } else {
                    (None, 0)
                }
            } else {
                (None, 0)
            };
            let rest = &after[code_start..];
            if let Some(end) = rest.find("```") {
                let code = &rest[..end];
                if let Some(ref lang) = lang {
                    result.push_str(&format!("<pre><code class=\"language-{}\">", lang));
                } else {
                    result.push_str("<pre><code>");
                }
                result.push_str(code);
                result.push_str("</code></pre>");
                src = &rest[end + 3..];
                found = true;
            } else {
                result.push_str("```");
                src = after;
            }
        }
        result.push_str(src);
        if found { changed = true; out = result; }
    }

    // Inline code: `...`
    let (s, c) = replace_delimited(&out, "`", "<code>", "</code>");
    if c { changed = true; out = s; }

    // Bold before italic: **...**
    let (s, c) = replace_delimited(&out, "**", "<strong>", "</strong>");
    if c { changed = true; out = s; }

    // Italic: *...*
    let (s, c) = replace_delimited(&out, "*", "<em>", "</em>");
    if c { changed = true; out = s; }

    // Strikethrough: ~~...~~
    let (s, c) = replace_delimited(&out, "~~", "<del>", "</del>");
    if c { changed = true; out = s; }

    if changed { Some(out) } else { None }
}

/// Build a RoomMessageEventContent, resolving display names to Matrix mention links
/// and converting simple markdown formatting to HTML.
fn build_message_content(body: &str, member_map: &std::collections::HashMap<String, String>) -> matrix_sdk::ruma::events::room::message::RoomMessageEventContent {
    use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;

    let mut has_rich = false;
    let mut html = body.to_string();
    let plain = body.to_string();

    // Markdown formatting first (before newline conversion, so code blocks preserve \n)
    if let Some(formatted) = simple_markdown_to_html(&html) {
        html = formatted;
        has_rich = true;
    }

    // Newlines → <br> only outside <pre> blocks
    if html.contains('\n') {
        let mut out = String::new();
        let mut rest = html.as_str();
        while let Some(pre_start) = rest.find("<pre>") {
            out.push_str(&rest[..pre_start].replace('\n', "<br>"));
            let after = &rest[pre_start..];
            if let Some(pre_end) = after.find("</pre>") {
                out.push_str(&after[..pre_end + 6]);
                rest = &after[pre_end + 6..];
            } else {
                out.push_str(after);
                rest = "";
            }
        }
        out.push_str(&rest.replace('\n', "<br>"));
        if out != html { html = out; has_rich = true; }
    }

    // Mention resolution — sort by name length descending to replace longer names first
    let mut names: Vec<_> = member_map.iter().collect();
    names.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (display_name, user_id) in &names {
        if display_name.len() > 1 && body.contains(display_name.as_str()) {
            // Only replace word-bounded occurrences (not inside URLs or other words)
            let encoded_uid = user_id.replace('%', "%25").replace('"', "%22").replace('<', "%3C").replace('>', "%3E");
            let mention_html = format!(
                r#"<a href="https://matrix.to/#/{encoded_uid}">{}</a>"#,
                html_escape(display_name)
            );
            let mut new_html = String::with_capacity(html.len());
            let name = display_name.as_str();
            let mut search = html.as_str();
            let mut replaced = false;
            while let Some(pos) = search.find(name) {
                // Check character before match — skip if part of URL/word
                let before = if pos > 0 { search.as_bytes()[pos - 1] } else { b' ' };
                let after_pos = pos + name.len();
                let after = if after_pos < search.len() { search.as_bytes()[after_pos] } else { b' ' };
                let before_ok = !before.is_ascii_alphanumeric() && before != b'/' && before != b'.' && before != b'-' && before != b'_';
                let after_ok = !after.is_ascii_alphanumeric() && after != b'/' && after != b'.' && after != b'-' && after != b'_';
                if before_ok && after_ok {
                    new_html.push_str(&search[..pos]);
                    new_html.push_str(&mention_html);
                    search = &search[after_pos..];
                    replaced = true;
                } else {
                    new_html.push_str(&search[..after_pos]);
                    search = &search[after_pos..];
                }
            }
            new_html.push_str(search);
            if replaced {
                html = new_html;
                has_rich = true;
            }
        }
    }

    if has_rich {
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
    let mut text = use_signal(String::new);
    let mut sending = use_signal(|| false);
    let mut uploading = use_signal(|| false);
    let mut emoji_open = use_signal(|| false);
    let mut tab_prefix = use_signal(String::new);
    let mut tab_index = use_signal(|| 0usize);
    // Use shared help_text signal so async commands can update it too

    let reply_to = ui.reply_to.read().clone();
    let edit_event = ui.edit_event.read().clone();
    let upload_pending = ui.upload_pending.read().clone();

    // Auto-focus input on room switch + cancel typing in previous room
    let mut prev_room = use_signal(|| Option::<String>::None);
    let core_switch = core.clone();
    use_effect(move || {
        let new_room = ui.selected_room_id.read().clone();
        // Cancel typing in previous room
        if let Some(old_id) = prev_room.peek().clone() {
            if !old_id.starts_with("thread::") {
                let core = core_switch.clone();
                let old_id = old_id.clone();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::messages::send_typing(&client, &old_id, false).await;
                });
            }
        }
        prev_room.set(new_room);
        spawn(async move {
            let _ = document::eval("document.getElementById('flux-input')?.focus();");
        });
    });

    // When entering edit mode, populate the input with the original text
    use_effect(move || {
        let edit = ui.edit_event.read().clone();
        if let Some((_, ref body)) = edit {
            text.set(body.clone());
            let body_js = serde_json::to_string(body).unwrap_or_default();
            spawn(async move {
                let _ = document::eval(&format!(
                    r#"let el = document.getElementById('flux-input');
                    if (el) {{ el.innerText = {body_js}; el.focus();
                    let r = document.createRange(); r.selectNodeContents(el); r.collapse(false);
                    let s = window.getSelection(); s.removeAllRanges(); s.addRange(r); }}"#
                ));
            });
        }
    });

    let mut ui_cancel = ui.clone();
    let mut ui_cancel2 = ui.clone();
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

    let handle_send: Rc<RefCell<dyn FnMut(())>> = Rc::new(RefCell::new(move |_: ()| {
        // Trim both ASCII whitespace and non-breaking spaces (contenteditable inserts \u00A0)
        let mut body = text().replace('\u{00A0}', " ").trim().to_string();

        // Handle slash commands via modular registry
        if body.starts_with('/') {
            let room_id = ui.selected_room_id.read().clone();
            match commands::dispatch(&body, room_id.as_deref(), core.clone(), ui.clone()) {
                CommandAction::Handled => {
                    text.set(String::new());
                    let _ = document::eval("let el = document.getElementById('flux-input'); if(el) { el.innerHTML = ''; el.classList.remove('cmd-mode'); }");
                    ui.help_text.set(None);
                    return;
                }
                CommandAction::Rewrite(new_body) => {
                    body = new_body;
                    ui.help_text.set(None);
                }
                CommandAction::ShowHelp(msg) => {
                    text.set(String::new());
                    let _ = document::eval("let el = document.getElementById('flux-input'); if(el) { el.innerHTML = ''; el.classList.remove('cmd-mode'); }");
                    ui.help_text.set(Some(msg));
                    return;
                }
                CommandAction::Noop => {}
            }
        }

        let body = body;
        let pending_upload = {
            let val = ui.upload_pending.read().clone();
            val
        };
        if body.is_empty() && pending_upload.is_none() || *sending.read() { return; }
        let raw_room_id = match ui.selected_room_id.read().clone() { Some(id) => id, None => return };

        let (room_id, thread_root_eid) = if raw_room_id.starts_with("thread::") {
            let parts: Vec<&str> = raw_room_id.splitn(3, "::").collect();
            if parts.len() == 3 {
                (parts[1].to_string(), Some(parts[2].to_string()))
            } else { return; }
        } else {
            (raw_room_id.clone(), None)
        };

        let member_map: std::collections::HashMap<String, String> = {
            let real_rid = if room_id.starts_with("thread::") {
                room_id.split("::").nth(1).unwrap_or(&room_id).to_string()
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
        let _ = document::eval("let el = document.getElementById('flux-input'); if(el) { el.innerHTML = ''; el.classList.remove('cmd-mode'); }");
        ui.reply_to.set(None);
        ui.edit_event.set(None);
        ui.upload_pending.set(None);
        if !room_id.starts_with("thread::") {
            let core = core.clone();
            let room_id = room_id.clone();
            spawn(async move {
                let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                let _ = fluxchat_core::messages::send_typing(&client, &room_id, false).await;
            });
        }

        if let Some((filename, mime, data)) = pending_upload {
            let core2 = core.clone();
            let body2 = body.clone();
            let room_id2 = room_id.clone();
            spawn(async move {
                sending.set(true);
                let client = match core2.get_client().await { Ok(c) => c, Err(_) => { sending.set(false); return; } };
                let _ = fluxchat_core::messages::upload_file(&client, &room_id2, &filename, &mime, data).await;
                if !body2.is_empty() {
                    let _ = fluxchat_core::messages::send_message(&client, &room_id2, &body2).await;
                }
                sending.set(false);
            });
            return;
        }

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
    }));

    // ── Contenteditable JS interop ──────────────────────────────
    // Install event listeners and receive messages from JS
    let handle_send_ce = handle_send.clone();
    use_future(move || {
        let core_ce = core_typing.clone();
        let ui_ce = ui_typing.clone();
        let handle_send = handle_send_ce.clone();
        async move {
            // Wait a tick for DOM to be ready
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;

            let mut eval = document::eval(r#"
                (function() {
                    const el = document.getElementById('flux-input');
                    if (!el || el.__fluxCeInit) return;
                    el.__fluxCeInit = true;

                    // Extract plain text from contenteditable, reading display names from pills
                    window.__fluxExtractText = function(node) {
                        let result = '';
                        let children = Array.from((node || el).childNodes);
                        for (let i = 0; i < children.length; i++) {
                            let child = children[i];
                            if (child.nodeType === Node.TEXT_NODE) {
                                result += child.textContent;
                            } else if (child.dataset && child.dataset.displayName) {
                                result += child.dataset.displayName;
                            } else if (child.nodeName === 'BR') {
                                result += '\n';
                            } else if (child.nodeName === 'DIV' || child.nodeName === 'P') {
                                // Block elements get a newline prefix (unless first child)
                                if (i > 0) result += '\n';
                                result += window.__fluxExtractText(child);
                            } else {
                                result += child.innerText || '';
                            }
                        }
                        if (result === '\n') result = '';
                        // Normalize non-breaking spaces to regular spaces
                        result = result.replace(/\u00A0/g, ' ');
                        return result;
                    };

                    // Insert a mention pill at cursor position
                    window.__fluxInsertPill = function(displayName, userId, avatarSrc, initial, colorIdx) {
                        el.focus();
                        let pill = document.createElement('span');
                        pill.className = 'ce-mention-pill';
                        pill.contentEditable = 'false';
                        pill.dataset.userId = userId;
                        pill.dataset.displayName = displayName;
                        if (avatarSrc) {
                            pill.innerHTML = '<img src="' + avatarSrc + '" class="ce-pill-avatar">' +
                                '<span class="ce-pill-name nick-' + colorIdx + '">' + displayName + '</span>';
                        } else {
                            pill.innerHTML = '<span class="ce-pill-initial nick-' + colorIdx + '">' + initial + '</span>' +
                                '<span class="ce-pill-name nick-' + colorIdx + '">' + displayName + '</span>';
                        }
                        let sel = window.getSelection();
                        let range = sel.getRangeAt(0);
                        range.deleteContents();
                        range.insertNode(pill);
                        // Add space after pill and move cursor there
                        let space = document.createTextNode('\u00A0');
                        pill.after(space);
                        range = document.createRange();
                        range.setStartAfter(space);
                        range.collapse(true);
                        sel.removeAllRanges();
                        sel.addRange(range);
                        dioxus.send('input:' + window.__fluxExtractText());
                    };

                    el.addEventListener('input', () => {
                        // Clean stray <br> in empty contenteditable so :empty placeholder works
                        if (el.innerHTML === '<br>' || el.innerHTML === '<div><br></div>') {
                            el.innerHTML = '';
                        }
                        dioxus.send('input:' + window.__fluxExtractText());
                    });

                    el.addEventListener('keydown', (e) => {
                        if (e.key === 'Enter' && !e.shiftKey) {
                            e.preventDefault();
                            dioxus.send('send:' + window.__fluxExtractText());
                        } else if (e.key === 'Tab') {
                            e.preventDefault();
                            dioxus.send('tab:' + window.__fluxExtractText());
                        } else if (e.key === 'Escape') {
                            e.preventDefault();
                            dioxus.send('escape:');
                        } else if (e.key === 'ArrowUp') {
                            let t = window.__fluxExtractText();
                            if (t === '' || t === '\n') {
                                e.preventDefault();
                                dioxus.send('arrowup:');
                            }
                        }
                    });

                    el.addEventListener('paste', (e) => {
                        // Let image pastes propagate to global handler
                        if (e.clipboardData && e.clipboardData.items) {
                            for (let item of e.clipboardData.items) {
                                if (item.type.startsWith('image/')) return;
                            }
                        }
                        e.preventDefault();
                        let t = e.clipboardData.getData('text/plain');
                        document.execCommand('insertText', false, t);
                        dioxus.send('input:' + window.__fluxExtractText());
                    });

                    dioxus.send('ready:');
                })();
            "#);

            while let Ok(msg) = eval.recv::<String>().await {
                        if let Some(txt) = msg.strip_prefix("input:") {
                            let val = txt.to_string();
                            text.set(val.clone());
                            tab_prefix.set(String::new());

                            // Typing indicator — SDK debounces to every 3s, server timeout is 4s
                            let typing = !val.is_empty();
                            let core = core_ce.clone();
                            if let Some(room_id) = ui_ce.selected_room_id.peek().clone() {
                                if !room_id.starts_with("thread::") {
                                    spawn(async move {
                                        let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                                        let _ = fluxchat_core::messages::send_typing(&client, &room_id, typing).await;
                                    });
                                }
                            }

                            // Command mode CSS class toggle
                            if val.starts_with('/') && !val.contains(' ') {
                                let _ = document::eval("document.getElementById('flux-input')?.classList.add('cmd-mode');");
                            } else {
                                let _ = document::eval("document.getElementById('flux-input')?.classList.remove('cmd-mode');");
                            }
                        } else if let Some(txt) = msg.strip_prefix("send:") {
                            text.set(txt.to_string());
                            (handle_send.borrow_mut())(());
                            // Clear contenteditable
                            text.set(String::new());
                            let _ = document::eval(
                                "let el = document.getElementById('flux-input'); if(el) { el.innerHTML = ''; el.classList.remove('cmd-mode'); }"
                            );
                        } else if let Some(txt) = msg.strip_prefix("tab:") {
                            let val = txt.to_string();
                            text.set(val.clone());
                            let in_autocomplete = val.starts_with('/') && !val.contains(' ');

                            // Slash command tab completion
                            if in_autocomplete {
                                let all = commands::autocomplete_entries();
                                let filtered: Vec<_> = all.iter()
                                    .filter(|(name, _, _)| name.starts_with(&val[..]) || val == "/")
                                    .collect();
                                if !filtered.is_empty() {
                                    // Cycle through matches on repeated Tab
                                    let idx = if *tab_prefix.peek() == val {
                                        (*tab_index.peek() + 1) % filtered.len()
                                    } else {
                                        tab_prefix.set(val.clone());
                                        0
                                    };
                                    tab_index.set(idx);
                                    let c = format!("{} ", filtered[idx].0);
                                    text.set(c.clone());
                                    let c_js = serde_json::to_string(&c).unwrap_or_default();
                                    let _ = document::eval(&format!(
                                        r#"let el = document.getElementById('flux-input');
                                        if (el) {{ el.innerText = {c_js};
                                        let r = document.createRange(); r.selectNodeContents(el); r.collapse(false);
                                        let s = window.getSelection(); s.removeAllRanges(); s.addRange(r);
                                        el.classList.remove('cmd-mode'); }}"#
                                    ));
                                }
                            } else {
                                // @mention tab completion → insert pill
                                let last_word = val.rsplit_once(' ').map(|(_, w)| w).unwrap_or(&val);
                                if last_word.starts_with('@') && last_word.len() > 1 {
                                    let needle = last_word[1..].to_lowercase();
                                    let mention_ui = consume_context::<UiState>();
                                    let sel = mention_ui.selected_room_id.peek().clone();
                                    if let Some(ref rid) = sel {
                                        let real_rid = if rid.starts_with("thread::") {
                                            rid.split("::").nth(1).unwrap_or(rid).to_string()
                                        } else { rid.clone() };
                                        if let Some(members) = mention_ui.members_by_room.peek().get(&real_rid) {
                                            if let Some(m) = members.iter().find(|m|
                                                m.display_name.to_lowercase().starts_with(&needle)
                                                || m.user_id.to_lowercase().contains(&needle)
                                            ) {
                                                let display_name = m.display_name.clone();
                                                let user_id = m.user_id.clone();
                                                let avatar_src = m.avatar_url.as_ref().and_then(|mxc| {
                                                    core_ce.media_cache.lock().unwrap().get(mxc).cloned()
                                                });
                                                let initial = display_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                                                let color_idx = crate::state::nick_color_index(&display_name);

                                                // Remove the @query text, then insert pill
                                                let before = if let Some(idx) = val.rfind(last_word) {
                                                    val[..idx].to_string()
                                                } else { String::new() };

                                                let dn_js = serde_json::to_string(&display_name).unwrap_or_default();
                                                let uid_js = serde_json::to_string(&user_id).unwrap_or_default();
                                                let avatar_js = avatar_src.as_ref()
                                                    .map(|s| serde_json::to_string(s).unwrap_or_default())
                                                    .unwrap_or_else(|| "null".to_string());
                                                let initial_js = serde_json::to_string(&initial).unwrap_or_default();
                                                let before_js = serde_json::to_string(&before).unwrap_or_default();

                                                let _ = document::eval(&format!(
                                                    r#"let el = document.getElementById('flux-input');
                                                    if (el) {{
                                                        // Set text before the @query
                                                        el.innerText = {before_js};
                                                        // Move cursor to end
                                                        let r = document.createRange();
                                                        r.selectNodeContents(el);
                                                        r.collapse(false);
                                                        let s = window.getSelection();
                                                        s.removeAllRanges();
                                                        s.addRange(r);
                                                        // Insert pill at cursor
                                                        window.__fluxInsertPill({dn_js}, {uid_js}, {avatar_js}, {initial_js}, {color_idx});
                                                    }}"#
                                                ));
                                                // text signal will be updated by the pill insert's dioxus.send
                                            }
                                        }
                                    }
                                }
                            }
                        } else if msg.starts_with("escape:") {
                            let mut ui = consume_context::<UiState>();
                            ui.reply_to.set(None);
                            ui.edit_event.set(None);
                            text.set(String::new());
                            ui.help_text.set(None);
                            let _ = document::eval(
                                "let el = document.getElementById('flux-input'); if(el) { el.innerHTML = ''; el.classList.remove('cmd-mode'); }"
                            );
                        } else if msg.starts_with("arrowup:") {
                            // Edit last own message (IRC-style up-arrow)
                            let mut ui = consume_context::<UiState>();
                            let my_id = ui.user_id.peek().clone();
                            let room_id = ui.selected_room_id.peek().clone();
                            if let Some(rid) = room_id {
                                let real_rid = if rid.starts_with("thread::") {
                                    rid.split("::").nth(1).unwrap_or(&rid).to_string()
                                } else { rid.clone() };
                                if let Some(msgs) = ui.messages_by_room.peek().get(&real_rid) {
                                    // Find the last message sent by the current user
                                    if let Some(own_msg) = msgs.iter().rev().find(|m| m.sender == my_id && !m.body.is_empty()) {
                                        let eid = own_msg.event_id.clone();
                                        let body = own_msg.body.clone();
                                        ui.edit_event.set(Some((eid, body.clone())));
                                        ui.reply_to.set(None);
                                        text.set(body.clone());
                                        let body_js = serde_json::to_string(&body).unwrap_or_default();
                                        let _ = document::eval(&format!(
                                            r#"let el = document.getElementById('flux-input');
                                            if (el) {{ el.innerText = {body_js}; el.focus();
                                            let r = document.createRange(); r.selectNodeContents(el); r.collapse(false);
                                            let s = window.getSelection(); s.removeAllRanges(); s.addRange(r); }}"#
                                        ));
                                    }
                                }
                            }
                        } else if msg.starts_with("ready:") {
                            // JS listeners installed successfully
                        }
            }
        }
    });

    // Prompt prefix changes based on mode
    let (prefix, prefix_color) = if edit_event.is_some() {
        ("ed\u{276f}", "var(--yellow)")
    } else if reply_to.is_some() {
        ("re\u{276f}", "var(--green)")
    } else {
        ("\u{276f}", "var(--accent-muted)")
    };

    // Format button helper — wraps selection or appends markup (contenteditable version)
    let make_fmt_handler = |pfx: &'static str, sfx: &'static str| {
        move |_| {
            let pfx = pfx.to_string();
            let sfx = sfx.to_string();
            spawn(async move {
                let pfx_js = serde_json::to_string(&pfx).unwrap_or_default();
                let sfx_js = serde_json::to_string(&sfx).unwrap_or_default();
                let mut eval = document::eval(&format!(
                    r#"
                    let el = document.getElementById('flux-input');
                    if (el) {{
                        el.focus();
                        let sel = window.getSelection();
                        let selected = sel.toString();
                        if (selected) {{
                            document.execCommand('insertText', false, {pfx_js} + selected + {sfx_js});
                        }} else {{
                            document.execCommand('insertText', false, {pfx_js} + {sfx_js});
                        }}
                        let t = el.innerText;
                        if (t === '\n') t = '';
                        dioxus.send(t);
                    }} else {{ dioxus.send(''); }}
                    "#
                ));
                if let Ok(val) = eval.recv::<String>().await {
                    text.set(val);
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
                        onclick: move |_| {
                            ui_cancel2.edit_event.set(None);
                            text.set(String::new());
                            let _ = document::eval("let el = document.getElementById('flux-input'); if(el) { el.innerHTML = ''; el.classList.remove('cmd-mode'); }");
                        },
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
            // Mention preview removed — pills with avatars now show inline in the input
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
                          rid.split("::").nth(1).unwrap_or(rid).to_string()
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
                                        let val = format!("{name} ");
                                        text.set(val.clone());
                                        let v_js = serde_json::to_string(&val).unwrap_or_default();
                                        spawn(async move {
                                            let _ = document::eval(&format!(
                                                r#"let el = document.getElementById('flux-input');
                                                if (el) {{ el.innerText = {v_js}; el.focus();
                                                let r = document.createRange(); r.selectNodeContents(el); r.collapse(false);
                                                let s = window.getSelection(); s.removeAllRanges(); s.addRange(r);
                                                el.classList.remove('cmd-mode'); }}"#
                                            ));
                                        });
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
                              let uid = user_id.clone();
                              let av = avatar_src.clone();
                              let at_q = at_query.to_string();
                              let initial = display_name.chars().next().unwrap_or('?').to_uppercase().to_string();
                              let color_idx = crate::state::nick_color_index(display_name);
                              rsx! {
                                div {
                                    class: "cmd-item",
                                    style: "display: flex; align-items: center; gap: 8px; padding: 4px 12px; cursor: pointer;",
                                    onclick: move |_| {
                                        let current = text();
                                        let before = if let Some(idx) = current.rfind(&at_q) {
                                            current[..idx].to_string()
                                        } else { current.clone() };

                                        let dn_js = serde_json::to_string(&display).unwrap_or_default();
                                        let uid_js = serde_json::to_string(&uid).unwrap_or_default();
                                        let avatar_js = av.as_ref()
                                            .map(|s| serde_json::to_string(s).unwrap_or_default())
                                            .unwrap_or_else(|| "null".to_string());
                                        let initial_js = serde_json::to_string(&initial).unwrap_or_default();
                                        let before_js = serde_json::to_string(&before).unwrap_or_default();

                                        spawn(async move {
                                            let _ = document::eval(&format!(
                                                r#"let el = document.getElementById('flux-input');
                                                if (el) {{
                                                    el.innerText = {before_js};
                                                    el.focus();
                                                    let r = document.createRange();
                                                    r.selectNodeContents(el);
                                                    r.collapse(false);
                                                    let s = window.getSelection();
                                                    s.removeAllRanges();
                                                    s.addRange(r);
                                                    window.__fluxInsertPill({dn_js}, {uid_js}, {avatar_js}, {initial_js}, {color_idx});
                                                }}"#
                                            ));
                                        });
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
                            let _ = document::eval(r#"
                                (function() {
                                    let input = document.createElement('input');
                                    input.type = 'file';
                                    input.accept = 'image/*,video/*,audio/*,application/*';
                                    input.style.display = 'none';
                                    document.body.appendChild(input);
                                    input.addEventListener('change', () => {
                                        if (input.files.length > 0) {
                                            let file = input.files[0];
                                            let reader = new FileReader();
                                            reader.onload = () => {
                                                let base64 = reader.result.split(',')[1];
                                                if (window.__fluxchat_dioxus) {
                                                    window.__fluxchat_dioxus.send('file:' + JSON.stringify({
                                                        name: file.name,
                                                        type: file.type || 'application/octet-stream',
                                                        data: base64
                                                    }));
                                                }
                                                input.remove();
                                            };
                                            reader.readAsDataURL(file);
                                        } else {
                                            input.remove();
                                        }
                                    });
                                    input.click();
                                })();
                            "#);
                        },
                        dangerous_inner_html: ICO_PLUS,
                    }

                    // Prompt
                    span { style: "color: {prefix_color}; font-family: var(--font-mono); font-size: 18px; font-weight: 700; flex-shrink: 0; padding: 0 6px; line-height: 1;", "{prefix}" }

                    // Text input (contenteditable)
                    div {
                        id: "flux-input",
                        contenteditable: "true",
                        style: "flex: 1; background: transparent; border: none; outline: none; color: var(--text-bright); font-size: 14px; font-family: var(--font-mono); padding: 8px 0; min-width: 0; white-space: pre-wrap; word-break: break-word; overflow-y: auto; max-height: 120px; line-height: 1.4; caret-color: var(--accent);",
                    }

                    // Emoji button
                    div { style: "position: relative; flex-shrink: 0;",
                        span {
                            class: "cmd-btn",
                            style: "color: var(--text-dim); cursor: pointer; padding: 6px 8px; display: inline-flex; align-items: center; border-radius: 4px; transition: color 0.1s, background 0.1s;",
                            title: "Emoji",
                            onclick: move |_| { let v = *emoji_open.peek(); emoji_open.set(!v); },
                            dangerous_inner_html: ICO_EMOJI,
                        }
                        if *emoji_open.read() {
                            div { style: "position: absolute; bottom: 100%; right: 0; margin-bottom: 8px; z-index: 100;",
                                EmojiPicker {
                                    on_select: move |emoji: String| {
                                        let current = text();
                                        text.set(format!("{current}{emoji}"));
                                        emoji_open.set(false);
                                        let emoji_js = serde_json::to_string(&emoji).unwrap_or_default();
                                        spawn(async move {
                                            let _ = document::eval(&format!(
                                                "let el = document.getElementById('flux-input'); if(el) {{ el.focus(); document.execCommand('insertText', false, {emoji_js}); }}"
                                            ));
                                        });
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
