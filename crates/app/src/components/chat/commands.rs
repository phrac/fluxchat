//! Modular slash command registry — irssi-style.
//!
//! Each command is a `SlashCommand` struct. Commands are registered in `all_commands()`
//! and looked up by name at dispatch time. To add a new command, just add an entry.

use std::sync::Arc;
use dioxus::prelude::*;
use fluxchat_core::AppState;
use crate::state::UiState;

/// What a command does when executed.
pub enum CommandAction {
    /// Command handled everything (cleared input, spawned work). Stop processing.
    Handled,
    /// Rewrite the message body and fall through to normal send.
    Rewrite(String),
    /// Show help text to the user (not sent to room).
    ShowHelp(String),
    /// Command not applicable (e.g. missing args). Do nothing.
    Noop,
}

/// Whether the command needs an active room.
#[derive(Clone, Copy, PartialEq)]
pub enum CommandScope {
    /// Works without a room selected (settings, help, etc.)
    Global,
    /// Requires a room to be selected
    Room,
    /// Rewrites body text, needs room for sending
    TextRewrite,
}

pub struct SlashCommand {
    pub name: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
    pub scope: CommandScope,
    pub execute: fn(args: &str, room_id: Option<&str>, core: Arc<AppState>, ui: UiState) -> CommandAction,
}

/// The full command registry. Add new commands here.
pub fn all_commands() -> Vec<SlashCommand> {
    vec![
        // ── Global commands ──────────────────────────────────────
        SlashCommand {
            name: "/settings",
            usage: "",
            description: "Open settings",
            scope: CommandScope::Global,
            execute: |_, _, _, mut ui| {
                ui.settings_tab.set("account".into());
                ui.settings_open.set(true);
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/account",
            usage: "",
            description: "Open account settings",
            scope: CommandScope::Global,
            execute: |_, _, _, mut ui| {
                ui.settings_tab.set("account".into());
                ui.settings_open.set(true);
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/sessions",
            usage: "",
            description: "Open sessions settings",
            scope: CommandScope::Global,
            execute: |_, _, _, mut ui| {
                ui.settings_tab.set("sessions".into());
                ui.settings_open.set(true);
                CommandAction::Handled
            },
        },
        // ── Room commands ────────────────────────────────────────
        SlashCommand {
            name: "/users",
            usage: "<query>",
            description: "Search for Matrix users",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                if args.is_empty() { return CommandAction::Noop; }
                let query = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    match fluxchat_core::rooms::search_users(&client, &query, 20).await {
                        Ok(results) => {
                            if results.is_empty() {
                                ui.help_text.set(Some(format!("No users found for '{query}'")));
                            } else {
                                let mut lines = vec![format!("Found {} users:", results.len()), String::new()];
                                for u in &results {
                                    let name = u.display_name.as_deref().unwrap_or("");
                                    if name.is_empty() {
                                        lines.push(format!("  {}", u.user_id));
                                    } else {
                                        lines.push(format!("  {} — {}", u.user_id, name));
                                    }
                                }
                                lines.push(String::new());
                                lines.push("Use /query <user> to open a DM, /whois <user> for profile".into());
                                ui.help_text.set(Some(lines.join("\n")));
                            }
                        }
                        Err(e) => { ui.help_text.set(Some(format!("Search failed: {e}"))); }
                    }
                });
                CommandAction::ShowHelp(format!("Searching for '{args}'..."))
            },
        },
        SlashCommand {
            name: "/directory",
            usage: "[search]",
            description: "Browse public rooms",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                let query = if args.is_empty() { None } else { Some(args.to_string()) };
                let args_display = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let search = query.as_deref();
                    match fluxchat_core::rooms::list_public_rooms(&client, search, Some(20), None).await {
                        Ok((rooms, _next)) => {
                            if rooms.is_empty() {
                                ui.help_text.set(Some("No public rooms found".into()));
                            } else {
                                let mut lines = vec![format!("Public rooms ({}):", rooms.len()), String::new()];
                                for r in &rooms {
                                    let name = r.name.as_deref().unwrap_or("(unnamed)");
                                    let alias = r.alias.as_deref().unwrap_or(&r.room_id);
                                    lines.push(format!("  {} — {} ({} members)", alias, name, r.num_members));
                                }
                                lines.push(String::new());
                                lines.push("Use /join <alias> to join a room".into());
                                ui.help_text.set(Some(lines.join("\n")));
                            }
                        }
                        Err(e) => { ui.help_text.set(Some(format!("Directory failed: {e}"))); }
                    }
                });
                let msg = if args_display.is_empty() { "Loading public rooms...".into() } else { format!("Searching rooms for '{args_display}'...") };
                CommandAction::ShowHelp(msg)
            },
        },
        SlashCommand {
            name: "/threads",
            usage: "",
            description: "Show threads in current room",
            scope: CommandScope::Room,
            execute: |_, _, _, mut ui| {
                ui.threads_open.set(true);
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/notices",
            usage: "",
            description: "Toggle server/system notices panel",
            scope: CommandScope::Room,
            execute: |_, _, _, mut ui| {
                let open = *ui.notices_open.peek();
                ui.notices_open.set(!open);
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/security",
            usage: "",
            description: "Open security settings",
            scope: CommandScope::Global,
            execute: |_, _, _, mut ui| {
                ui.settings_tab.set("security".into());
                ui.settings_open.set(true);
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/ignore",
            usage: "<user>",
            description: "Ignore a user (hide their messages)",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                if args.is_empty() { return CommandAction::Noop; }
                let user_id = if args.starts_with('@') { args.to_string() } else { format!("@{args}") };
                let uid = user_id.clone();
                ui.ignored_users.write().insert(user_id);
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    if let Ok(parsed) = matrix_sdk::ruma::UserId::parse(&uid) {
                        let _ = client.account().ignore_user(&parsed).await;
                    }
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/unignore",
            usage: "<user>",
            description: "Stop ignoring a user",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                if args.is_empty() { return CommandAction::Noop; }
                let user_id = if args.starts_with('@') { args.to_string() } else { format!("@{args}") };
                let uid = user_id.clone();
                ui.ignored_users.write().remove(&user_id);
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    if let Ok(parsed) = matrix_sdk::ruma::UserId::parse(&uid) {
                        let _ = client.account().unignore_user(&parsed).await;
                    }
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/whois",
            usage: "<user>",
            description: "Show user profile",
            scope: CommandScope::Global,
            execute: |args, _, _, mut ui| {
                if args.is_empty() { return CommandAction::Noop; }
                let user_id = if args.starts_with('@') { args.to_string() } else { format!("@{args}") };
                ui.profile_user_id.set(Some(user_id));
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/query",
            usage: "<user>",
            description: "Open a DM conversation",
            scope: CommandScope::Global,
            execute: |args, _, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let user = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    // Create or find the DM room, then select it
                    let _ = fluxchat_core::rooms::open_dm(&client, &user).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/clear",
            usage: "",
            description: "Clear message scrollback",
            scope: CommandScope::Room,
            execute: |_, room_id, _, mut ui| {
                let rid = room_id.unwrap_or_default().to_string();
                ui.messages_by_room.write().insert(rid, Vec::new());
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/away",
            usage: "[message]",
            description: "Set away status or return",
            scope: CommandScope::Global,
            execute: |args, _, core, _| {
                let is_away = !args.is_empty();
                let status_msg = if is_away { Some(args.to_string()) } else { None };
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    use matrix_sdk::ruma::api::client::presence::set_presence;
                    use matrix_sdk::ruma::presence::PresenceState;
                    let state = if is_away { PresenceState::Unavailable } else { PresenceState::Online };
                    let user_id = match client.user_id() { Some(u) => u.to_owned(), None => return };
                    let mut request = set_presence::v3::Request::new(user_id, state);
                    request.status_msg = status_msg;
                    let _ = client.send(request).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/status",
            usage: "<message>",
            description: "Set a custom status message (or clear with no args)",
            scope: CommandScope::Global,
            execute: |args, _, core, _| {
                let status_msg = if args.is_empty() { None } else { Some(args.to_string()) };
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    use matrix_sdk::ruma::api::client::presence::set_presence;
                    use matrix_sdk::ruma::presence::PresenceState;
                    let user_id = match client.user_id() { Some(u) => u.to_owned(), None => return };
                    let mut request = set_presence::v3::Request::new(user_id, PresenceState::Online);
                    request.status_msg = status_msg;
                    let _ = client.send(request).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/quit",
            usage: "",
            description: "Logout and quit",
            scope: CommandScope::Global,
            execute: |_, _, core, mut ui| {
                spawn(async move {
                    if let Ok(client) = core.get_client().await {
                        let _ = client.matrix_auth().logout().await;
                    }
                    core.db.delete_session();
                    ui.logged_in.set(false);
                    ui.settings_open.set(false);
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/part",
            usage: "",
            description: "Leave the current room",
            scope: CommandScope::Room,
            execute: |_, room_id, core, _| {
                let rid = room_id.unwrap_or_default().to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::leave_room(&client, &rid).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/join",
            usage: "<room>",
            description: "Join a room by alias or ID",
            scope: CommandScope::Room,
            execute: |args, _, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let args = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::join_room(&client, &args).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/leave",
            usage: "",
            description: "Leave the current room",
            scope: CommandScope::Room,
            execute: |_, room_id, core, _| {
                let rid = room_id.unwrap_or_default().to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::leave_room(&client, &rid).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/invite",
            usage: "<user>",
            description: "Invite a user to this room",
            scope: CommandScope::Room,
            execute: |args, room_id, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let rid = room_id.unwrap_or_default().to_string();
                let args = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::invite_user(&client, &rid, &args).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/kick",
            usage: "<user> [reason]",
            description: "Kick a user from this room",
            scope: CommandScope::Room,
            execute: |args, room_id, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let rid = room_id.unwrap_or_default().to_string();
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                let user = parts[0].to_string();
                let reason = parts.get(1).map(|s| s.to_string());
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::kick_user(&client, &rid, &user, reason.as_deref()).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/ban",
            usage: "<user> [reason]",
            description: "Ban a user from this room",
            scope: CommandScope::Room,
            execute: |args, room_id, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let rid = room_id.unwrap_or_default().to_string();
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                let user = parts[0].to_string();
                let reason = parts.get(1).map(|s| s.to_string());
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::ban_user(&client, &rid, &user, reason.as_deref()).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/op",
            usage: "<user> [level]",
            description: "Set a user's power level (default 50)",
            scope: CommandScope::Room,
            execute: |args, room_id, core, ui| {
                if args.is_empty() { return CommandAction::Noop; }
                let rid = room_id.unwrap_or_default().to_string();
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                let user_input = parts[0].to_string();
                let level: i64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(50);
                // Resolve display name → user_id
                let user = if user_input.starts_with('@') {
                    user_input
                } else {
                    let real_rid = if rid.starts_with("thread::") {
                        rid.splitn(3, "::").nth(1).unwrap_or(&rid).to_string()
                    } else { rid.clone() };
                    ui.members_by_room.peek()
                        .get(&real_rid)
                        .and_then(|members| members.iter().find(|m|
                            m.display_name.to_lowercase() == user_input.to_lowercase()
                            || m.user_id.to_lowercase().contains(&user_input.to_lowercase())
                        ))
                        .map(|m| m.user_id.clone())
                        .unwrap_or(user_input)
                };
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::set_user_power_level(&client, &rid, &user, level).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/deop",
            usage: "<user>",
            description: "Remove a user's power level (set to 0)",
            scope: CommandScope::Room,
            execute: |args, room_id, core, ui| {
                if args.is_empty() { return CommandAction::Noop; }
                let rid = room_id.unwrap_or_default().to_string();
                let user_input = args.trim().to_string();
                let user = if user_input.starts_with('@') {
                    user_input
                } else {
                    let real_rid = if rid.starts_with("thread::") {
                        rid.splitn(3, "::").nth(1).unwrap_or(&rid).to_string()
                    } else { rid.clone() };
                    ui.members_by_room.peek()
                        .get(&real_rid)
                        .and_then(|members| members.iter().find(|m|
                            m.display_name.to_lowercase() == user_input.to_lowercase()
                            || m.user_id.to_lowercase().contains(&user_input.to_lowercase())
                        ))
                        .map(|m| m.user_id.clone())
                        .unwrap_or(user_input)
                };
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::set_user_power_level(&client, &rid, &user, 0).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/topic",
            usage: "<text>",
            description: "Set the room topic",
            scope: CommandScope::Room,
            execute: |args, room_id, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let rid = room_id.unwrap_or_default().to_string();
                let args = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::set_room_topic(&client, &rid, &args).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/nick",
            usage: "<name>",
            description: "Change your display name",
            scope: CommandScope::Global,
            execute: |args, _, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let args = args.to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::profile::set_display_name(&client, &args).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/msg",
            usage: "<user> <message>",
            description: "Send a direct message",
            scope: CommandScope::Global,
            execute: |args, _, core, _| {
                if args.is_empty() { return CommandAction::Noop; }
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                let user = parts[0].to_string();
                let body = parts.get(1).unwrap_or(&"").to_string();
                if body.is_empty() { return CommandAction::Noop; }
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::rooms::send_dm(&client, &user, &body).await;
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/attach",
            usage: "",
            description: "Attach a file",
            scope: CommandScope::Room,
            execute: |_, _, _, _| {
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
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/mute",
            usage: "",
            description: "Mute notifications for this room",
            scope: CommandScope::Room,
            execute: |_, room_id, core, mut ui| {
                let rid = room_id.unwrap_or_default().to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    match fluxchat_core::rooms::set_notification_mode(&client, &rid, "mute").await {
                        Ok(_) => { ui.help_text.set(Some("Room muted".into())); }
                        Err(e) => { ui.help_text.set(Some(format!("Failed to mute: {e}"))); }
                    }
                });
                CommandAction::Handled
            },
        },
        SlashCommand {
            name: "/unmute",
            usage: "",
            description: "Unmute notifications for this room",
            scope: CommandScope::Room,
            execute: |_, room_id, core, mut ui| {
                let rid = room_id.unwrap_or_default().to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    match fluxchat_core::rooms::set_notification_mode(&client, &rid, "all").await {
                        Ok(_) => { ui.help_text.set(Some("Room unmuted".into())); }
                        Err(e) => { ui.help_text.set(Some(format!("Failed to unmute: {e}"))); }
                    }
                });
                CommandAction::Handled
            },
        },
        // ── Text rewrite commands ────────────────────────────────
        SlashCommand {
            name: "/me",
            usage: "<action>",
            description: "Send an action message",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                CommandAction::Rewrite(format!("* {args}"))
            },
        },
        SlashCommand {
            name: "/shrug",
            usage: "[message]",
            description: "Append \u{00af}\\_(\u{30c4})_/\u{00af}",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                let text = if args.is_empty() { "¯\\_(ツ)_/¯".into() } else { format!("{args} ¯\\_(ツ)_/¯") };
                CommandAction::Rewrite(text)
            },
        },
        SlashCommand {
            name: "/tableflip",
            usage: "[message]",
            description: "Append (\u{256f}\u{00b0}\u{25a1}\u{00b0})\u{256f}\u{fe35} \u{253b}\u{2501}\u{253b}",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                let text = if args.is_empty() { "(╯°□°)╯︵ ┻━┻".into() } else { format!("{args} (╯°□°)╯︵ ┻━┻") };
                CommandAction::Rewrite(text)
            },
        },
        SlashCommand {
            name: "/unflip",
            usage: "[message]",
            description: "Append \u{252c}\u{2500}\u{252c}\u{30ce}( \u{ba} _ \u{ba}\u{30ce})",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                let text = if args.is_empty() { "┬─┬ノ( º _ ºノ)".into() } else { format!("{args} ┬─┬ノ( º _ ºノ)") };
                CommandAction::Rewrite(text)
            },
        },
        SlashCommand {
            name: "/lenny",
            usage: "[message]",
            description: "Append ( \u{0361}\u{00b0} \u{035c}\u{0296} \u{0361}\u{00b0})",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                let text = if args.is_empty() { "( ͡° ͜ʖ ͡°)".into() } else { format!("{args} ( ͡° ͜ʖ ͡°)") };
                CommandAction::Rewrite(text)
            },
        },
        SlashCommand {
            name: "/slap",
            usage: "<user>",
            description: "Slap someone with a large trout",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                if args.is_empty() { return CommandAction::Noop; }
                CommandAction::Rewrite(format!("* slaps {args} around a bit with a large trout"))
            },
        },
        SlashCommand {
            name: "/troutslap",
            usage: "<user>",
            description: "The classic trout slap",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                if args.is_empty() { return CommandAction::Noop; }
                CommandAction::Rewrite(format!("* slaps {args} around a bit with a large trout"))
            },
        },
        SlashCommand {
            name: "/spoiler",
            usage: "<text>",
            description: "Send a spoiler message",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                if args.is_empty() { return CommandAction::Noop; }
                CommandAction::Rewrite(args.to_string())
            },
        },
        SlashCommand {
            name: "/timestamp",
            usage: "[short|full|off]",
            description: "Toggle timestamp format (short, full, or off)",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                let current = ui.timestamp_format.read().clone();
                let new_fmt = if args.trim().is_empty() {
                    match current.as_str() { "short" => "full", "full" => "off", _ => "short" }.to_string()
                } else {
                    match args.trim() {
                        "short" | "full" | "off" => args.trim().to_string(),
                        _ => return CommandAction::ShowHelp("Usage: /timestamp [short|full|off]".into()),
                    }
                };
                let _ = core.db.set_setting("timestamp_format", &new_fmt);
                ui.timestamp_format.set(new_fmt.clone());
                CommandAction::ShowHelp(format!("Timestamp format: {new_fmt}"))
            },
        },
        SlashCommand {
            name: "/compact",
            usage: "",
            description: "Toggle compact message display",
            scope: CommandScope::Global,
            execute: |_, _, core, mut ui| {
                let compact = !*ui.compact_mode.read();
                ui.compact_mode.set(compact);
                let _ = core.db.set_setting("compact_mode", if compact { "true" } else { "false" });
                CommandAction::ShowHelp(format!("Compact mode: {}", if compact { "on" } else { "off" }))
            },
        },
        SlashCommand {
            name: "/joinpart",
            usage: "",
            description: "Toggle join/part messages",
            scope: CommandScope::Global,
            execute: |_, _, core, mut ui| {
                let show = !*ui.show_joinpart.read();
                ui.show_joinpart.set(show);
                let _ = core.db.set_setting("show_joinpart", if show { "true" } else { "false" });
                CommandAction::ShowHelp(format!("Join/part messages: {}", if show { "shown" } else { "hidden" }))
            },
        },
        SlashCommand {
            name: "/highlight",
            usage: "<word>",
            description: "Add a highlight word (triggers mention-style notification)",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                let word = args.trim().to_lowercase();
                if word.is_empty() {
                    // Show current highlight words
                    let words = ui.highlight_words.read().clone();
                    if words.is_empty() {
                        return CommandAction::ShowHelp("No highlight words set. Use /highlight <word> to add one.".into());
                    }
                    return CommandAction::ShowHelp(format!("Highlight words: {}", words.join(", ")));
                }
                let mut words = ui.highlight_words.read().clone();
                if !words.contains(&word) {
                    words.push(word.clone());
                    ui.highlight_words.set(words.clone());
                    if let Ok(json) = serde_json::to_string(&words) {
                        let _ = core.db.set_setting("highlight_words", &json);
                    }
                }
                CommandAction::ShowHelp(format!("Added highlight word: {word}"))
            },
        },
        SlashCommand {
            name: "/unhighlight",
            usage: "[word]",
            description: "Remove a highlight word (or list all)",
            scope: CommandScope::Global,
            execute: |args, _, core, mut ui| {
                let word = args.trim().to_lowercase();
                if word.is_empty() {
                    let words = ui.highlight_words.read().clone();
                    if words.is_empty() {
                        return CommandAction::ShowHelp("No highlight words set. Use /highlight <word> to add one.".into());
                    }
                    return CommandAction::ShowHelp(format!("Highlight words: {}", words.join(", ")));
                }
                let mut words = ui.highlight_words.read().clone();
                if !words.iter().any(|w| w == &word) {
                    return CommandAction::ShowHelp(format!("'{word}' not in list. Current: {}", words.join(", ")));
                }
                words.retain(|w| w != &word);
                ui.highlight_words.set(words.clone());
                if let Ok(json) = serde_json::to_string(&words) {
                    let _ = core.db.set_setting("highlight_words", &json);
                }
                if words.is_empty() {
                    CommandAction::ShowHelp(format!("Removed '{word}'. No highlight words remaining."))
                } else {
                    CommandAction::ShowHelp(format!("Removed '{word}'. Remaining: {}", words.join(", ")))
                }
            },
        },
        SlashCommand {
            name: "/forward",
            usage: "<room> <message>",
            description: "Forward a message to another room",
            scope: CommandScope::Room,
            execute: |args, _, core, _| {
                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                if parts.len() < 2 { return CommandAction::ShowHelp("/forward <room_id_or_alias> <message>".into()); }
                let target = parts[0].to_string();
                let body = parts[1].to_string();
                spawn(async move {
                    let client = match core.get_client().await { Ok(c) => c, Err(_) => return };
                    let _ = fluxchat_core::messages::forward_message(&client, &target, &body, None).await;
                });
                CommandAction::Handled
            },
        },
    ]
}

/// Dispatch a slash command. Returns the action to take.
pub fn dispatch(input: &str, room_id: Option<&str>, core: Arc<AppState>, ui: UiState) -> CommandAction {
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd_name = parts[0];
    let args = parts.get(1).unwrap_or(&"").trim();

    let commands = all_commands();

    // /help or /help <cmd>
    if cmd_name == "/help" {
        return show_help(args, &commands);
    }

    if let Some(cmd) = commands.iter().find(|c| c.name == cmd_name) {
        // Global commands don't need a room
        if cmd.scope != CommandScope::Global && room_id.is_none() {
            return CommandAction::Noop;
        }
        (cmd.execute)(args, room_id, core, ui)
    } else {
        CommandAction::Noop
    }
}

fn show_help(filter: &str, commands: &[SlashCommand]) -> CommandAction {
    if filter.is_empty() {
        // List all commands grouped by scope
        let mut lines = Vec::new();
        lines.push("Available commands:".to_string());
        lines.push(String::new());

        let global: Vec<_> = commands.iter().filter(|c| c.scope == CommandScope::Global).collect();
        let room: Vec<_> = commands.iter().filter(|c| c.scope == CommandScope::Room).collect();
        let text: Vec<_> = commands.iter().filter(|c| c.scope == CommandScope::TextRewrite).collect();

        if !global.is_empty() {
            lines.push("Global:".to_string());
            for c in &global {
                lines.push(format!("  {} {} — {}", c.name, c.usage, c.description));
            }
        }
        if !room.is_empty() {
            lines.push("Room:".to_string());
            for c in &room {
                lines.push(format!("  {} {} — {}", c.name, c.usage, c.description));
            }
        }
        if !text.is_empty() {
            lines.push("Text:".to_string());
            for c in &text {
                lines.push(format!("  {} {} — {}", c.name, c.usage, c.description));
            }
        }
        lines.push(String::new());
        lines.push("Type /help <command> for details, or /<command> help".to_string());

        CommandAction::ShowHelp(lines.join("\n"))
    } else {
        // /help <cmd> — look up specific command (with or without leading /)
        let lookup = if filter.starts_with('/') { filter.to_string() } else { format!("/{filter}") };
        if let Some(cmd) = commands.iter().find(|c| c.name == lookup) {
            show_command_help(cmd)
        } else {
            CommandAction::ShowHelp(format!("Unknown command: {lookup}"))
        }
    }
}

fn show_command_help(cmd: &SlashCommand) -> CommandAction {
    let scope_label = match cmd.scope {
        CommandScope::Global => "global",
        CommandScope::Room => "requires room",
        CommandScope::TextRewrite => "text rewrite",
    };
    CommandAction::ShowHelp(format!(
        "{} {}\n{}\nScope: {}",
        cmd.name, cmd.usage, cmd.description, scope_label
    ))
}

/// Get autocomplete entries: (name, usage, description).
/// Returns owned strings so they can be used in Dioxus render closures.
pub fn autocomplete_entries() -> Vec<(String, String, String)> {
    let mut entries: Vec<_> = all_commands().iter()
        .map(|c| (c.name.to_string(), c.usage.to_string(), c.description.to_string()))
        .collect();
    entries.push(("/help".to_string(), "[command]".to_string(), "Show command help".to_string()));
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}
