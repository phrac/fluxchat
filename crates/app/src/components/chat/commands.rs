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
            name: "/spoiler",
            usage: "<text>",
            description: "Send a spoiler message",
            scope: CommandScope::TextRewrite,
            execute: |args, _, _, _| {
                if args.is_empty() { return CommandAction::Noop; }
                CommandAction::Rewrite(args.to_string())
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
