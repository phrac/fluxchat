//! Slash command system for the Iced app.
//!
//! Each command produces a `CommandResult` that the update loop translates
//! into Messages. This keeps commands decoupled from UI framework details.

use crate::message::Message;
use crate::state::FluxChat;

/// Result of executing a slash command.
pub enum CommandResult {
    /// Command handled — emit these messages.
    Messages(Vec<Message>),
    /// Rewrite the input text and send as a normal message.
    Rewrite(String),
    /// Show feedback text to the user.
    Help(String),
    /// Do nothing.
    Noop,
}

/// A registered slash command.
pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub description: &'static str,
    pub needs_room: bool,
}

/// Get all registered commands (for autocomplete).
pub fn all_commands() -> Vec<Command> {
    vec![
        Command { name: "/help", usage: "[command]", description: "Show available commands", needs_room: false },
        Command { name: "/settings", usage: "", description: "Open settings", needs_room: false },
        Command { name: "/account", usage: "", description: "Open account settings", needs_room: false },
        Command { name: "/sessions", usage: "", description: "Open sessions settings", needs_room: false },
        Command { name: "/security", usage: "", description: "Open security settings", needs_room: false },
        Command { name: "/join", usage: "<room>", description: "Join a room", needs_room: false },
        Command { name: "/create", usage: "[name]", description: "Create a new room", needs_room: false },
        Command { name: "/leave", usage: "", description: "Leave the current room", needs_room: true },
        Command { name: "/part", usage: "", description: "Leave the current room", needs_room: true },
        Command { name: "/topic", usage: "[new topic]", description: "View or set room topic", needs_room: true },
        Command { name: "/me", usage: "<action>", description: "Send an emote", needs_room: true },
        Command { name: "/nick", usage: "<name>", description: "Change your display name", needs_room: false },
        Command { name: "/msg", usage: "<user> <message>", description: "Send a direct message", needs_room: false },
        Command { name: "/invite", usage: "<user>", description: "Invite user to room", needs_room: true },
        Command { name: "/kick", usage: "<user> [reason]", description: "Kick user from room", needs_room: true },
        Command { name: "/ban", usage: "<user> [reason]", description: "Ban user from room", needs_room: true },
        Command { name: "/op", usage: "<user> [level]", description: "Set user power level", needs_room: true },
        Command { name: "/deop", usage: "<user>", description: "Remove user power level", needs_room: true },
        Command { name: "/mute", usage: "", description: "Mute room notifications", needs_room: true },
        Command { name: "/unmute", usage: "", description: "Unmute room notifications", needs_room: true },
        Command { name: "/threads", usage: "", description: "Toggle threads panel", needs_room: true },
        Command { name: "/notices", usage: "", description: "Toggle notices panel", needs_room: false },
        Command { name: "/search", usage: "", description: "Toggle search", needs_room: true },
        Command { name: "/members", usage: "", description: "Toggle member list", needs_room: true },
        Command { name: "/pins", usage: "", description: "Toggle pinned messages", needs_room: true },
        Command { name: "/files", usage: "", description: "Toggle files panel", needs_room: true },
        Command { name: "/clear", usage: "", description: "Clear scrollback", needs_room: true },
        Command { name: "/shrug", usage: "[text]", description: r"Append ¯\_(ツ)_/¯", needs_room: true },
        Command { name: "/tableflip", usage: "[text]", description: "Append (╯°□°)╯︵ ┻━┻", needs_room: true },
        Command { name: "/unflip", usage: "[text]", description: "Append ┬─┬ノ( º _ ºノ)", needs_room: true },
        Command { name: "/lenny", usage: "[text]", description: "Append ( ͡° ͜ʖ ͡°)", needs_room: true },
        Command { name: "/spoiler", usage: "<text>", description: "Send a spoiler", needs_room: true },
        Command { name: "/timestamp", usage: "[short|full|off]", description: "Set timestamp format", needs_room: false },
        Command { name: "/compact", usage: "", description: "Toggle compact mode", needs_room: false },
        Command { name: "/joinpart", usage: "", description: "Toggle join/part messages", needs_room: false },
        Command { name: "/zoom", usage: "[50-200]", description: "Set UI zoom level", needs_room: false },
        Command { name: "/quit", usage: "", description: "Log out", needs_room: false },
    ]
}

/// Dispatch a slash command from the input text.
pub fn dispatch(input: &str, state: &FluxChat) -> CommandResult {
    let trimmed = input.trim();
    let (cmd, args) = match trimmed.split_once(' ') {
        Some((c, a)) => (c, a.trim()),
        None => (trimmed, ""),
    };

    match cmd {
        "/help" => {
            let cmds = all_commands();
            if args.is_empty() {
                let help = cmds
                    .iter()
                    .map(|c| format!("{} {} — {}", c.name, c.usage, c.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                CommandResult::Help(help)
            } else {
                let target = if args.starts_with('/') { args } else { &format!("/{args}") };
                match cmds.iter().find(|c| c.name == target) {
                    Some(c) => CommandResult::Help(format!("{} {} — {}", c.name, c.usage, c.description)),
                    None => CommandResult::Help(format!("Unknown command: {target}")),
                }
            }
        }

        // --- UI commands ---
        "/settings" => CommandResult::Messages(vec![Message::OpenModal(
            crate::state::Modal::Settings { tab: "account".into() },
        )]),
        "/account" => CommandResult::Messages(vec![Message::OpenModal(
            crate::state::Modal::Settings { tab: "account".into() },
        )]),
        "/sessions" => CommandResult::Messages(vec![Message::OpenModal(
            crate::state::Modal::Settings { tab: "sessions".into() },
        )]),
        "/security" => CommandResult::Messages(vec![Message::OpenModal(
            crate::state::Modal::Settings { tab: "security".into() },
        )]),
        "/create" => CommandResult::Messages(vec![Message::OpenModal(
            crate::state::Modal::CreateRoom,
        )]),
        "/threads" => CommandResult::Messages(vec![Message::ToggleThreads]),
        "/notices" => CommandResult::Messages(vec![Message::ToggleNotices]),
        "/search" => CommandResult::Messages(vec![Message::ToggleSearch]),
        "/members" => CommandResult::Messages(vec![Message::ToggleMemberList]),
        "/pins" => CommandResult::Messages(vec![Message::TogglePins]),
        "/files" => CommandResult::Messages(vec![Message::ToggleFiles]),
        "/quit" => CommandResult::Messages(vec![Message::Logout]),

        // --- Prefs ---
        "/timestamp" => {
            let fmt = match args {
                "short" | "full" | "off" => args.to_string(),
                _ => {
                    // Cycle
                    match state.timestamp_format.as_str() {
                        "short" => "full".into(),
                        "full" => "off".into(),
                        _ => "short".into(),
                    }
                }
            };
            CommandResult::Messages(vec![
                Message::SetTimestampFormat(fmt.clone()),
                Message::Noop, // clear input handled by caller
            ])
        }
        "/compact" => CommandResult::Messages(vec![Message::ToggleCompactMode]),
        "/joinpart" => CommandResult::Messages(vec![Message::ToggleJoinPart]),
        "/zoom" => {
            let level = if args.is_empty() {
                match state.zoom_level {
                    z if z < 100 => 100,
                    100 => 110,
                    110 => 125,
                    125 => 150,
                    _ => 100,
                }
            } else if let Ok(l) = args.parse::<u32>() {
                if (50..=200).contains(&l) {
                    l
                } else {
                    return CommandResult::Help("Zoom must be between 50 and 200".into());
                }
            } else {
                return CommandResult::Help("Usage: /zoom [50-200]".into());
            };
            CommandResult::Messages(vec![Message::SetZoomLevel(level)])
        }

        // --- Room commands (async) ---
        "/join" => {
            if args.is_empty() {
                return CommandResult::Help("Usage: /join <room>".into());
            }
            CommandResult::Messages(vec![Message::JoinRoom(args.to_string())])
        }
        "/leave" | "/part" => {
            if let Some(room_id) = &state.selected_room_id {
                CommandResult::Messages(vec![Message::LeaveRoom(room_id.clone())])
            } else {
                CommandResult::Help("No room selected".into())
            }
        }
        "/invite" => {
            if args.is_empty() {
                return CommandResult::Help("Usage: /invite <user>".into());
            }
            if let Some(room_id) = &state.selected_room_id {
                CommandResult::Messages(vec![Message::InviteUser(room_id.clone(), args.to_string())])
            } else {
                CommandResult::Help("No room selected".into())
            }
        }
        "/kick" => {
            if args.is_empty() {
                return CommandResult::Help("Usage: /kick <user> [reason]".into());
            }
            let (user, reason) = args.split_once(' ')
                .map(|(u, r)| (u, Some(r.to_string())))
                .unwrap_or((args, None));
            if let Some(room_id) = &state.selected_room_id {
                CommandResult::Messages(vec![Message::KickUser(room_id.clone(), user.to_string(), reason)])
            } else {
                CommandResult::Help("No room selected".into())
            }
        }
        "/ban" => {
            if args.is_empty() {
                return CommandResult::Help("Usage: /ban <user> [reason]".into());
            }
            let (user, reason) = args.split_once(' ')
                .map(|(u, r)| (u, Some(r.to_string())))
                .unwrap_or((args, None));
            if let Some(room_id) = &state.selected_room_id {
                CommandResult::Messages(vec![Message::BanUser(room_id.clone(), user.to_string(), reason)])
            } else {
                CommandResult::Help("No room selected".into())
            }
        }
        "/topic" => {
            if let Some(room_id) = &state.selected_room_id {
                if args.is_empty() {
                    // Show current topic
                    let topic = state.rooms.iter()
                        .find(|r| r.room_id == *room_id)
                        .and_then(|r| r.topic.clone())
                        .unwrap_or_else(|| "(no topic)".into());
                    CommandResult::Help(format!("Topic: {topic}"))
                } else {
                    CommandResult::Messages(vec![Message::SetTopic(room_id.clone(), args.to_string())])
                }
            } else {
                CommandResult::Help("No room selected".into())
            }
        }
        "/nick" => {
            if args.is_empty() {
                return CommandResult::Help("Usage: /nick <name>".into());
            }
            CommandResult::Messages(vec![Message::SetNick(args.to_string())])
        }
        "/msg" => {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() < 2 {
                return CommandResult::Help("Usage: /msg <user> <message>".into());
            }
            CommandResult::Messages(vec![Message::SendDm(parts[0].to_string(), parts[1].to_string())])
        }

        // --- Text rewrites ---
        "/me" => {
            if args.is_empty() {
                CommandResult::Help("Usage: /me <action>".into())
            } else {
                // /me is special — we need to send as emote, not rewrite
                CommandResult::Rewrite(format!("/me {args}"))
            }
        }
        "/shrug" => CommandResult::Rewrite(
            if args.is_empty() { r"¯\_(ツ)_/¯".into() } else { format!(r"{args} ¯\_(ツ)_/¯") }
        ),
        "/tableflip" => CommandResult::Rewrite(
            if args.is_empty() { "(╯°□°)╯︵ ┻━┻".into() } else { format!("{args} (╯°□°)╯︵ ┻━┻") }
        ),
        "/unflip" => CommandResult::Rewrite(
            if args.is_empty() { "┬─┬ノ( º _ ºノ)".into() } else { format!("{args} ┬─┬ノ( º _ ºノ)") }
        ),
        "/lenny" => CommandResult::Rewrite(
            if args.is_empty() { "( ͡° ͜ʖ ͡°)".into() } else { format!("{args} ( ͡° ͜ʖ ͡°)") }
        ),
        "/spoiler" => {
            if args.is_empty() {
                CommandResult::Help("Usage: /spoiler <text>".into())
            } else {
                CommandResult::Rewrite(format!("||{args}||"))
            }
        }

        // --- Clear ---
        "/clear" => {
            if let Some(room_id) = &state.selected_room_id {
                CommandResult::Messages(vec![Message::SyncEvent(
                    crate::message::SyncEvent::MessagesUpdated(room_id.clone(), Vec::new()),
                )])
            } else {
                CommandResult::Noop
            }
        }

        _ => CommandResult::Help(format!("Unknown command: {cmd}. Type /help for commands.")),
    }
}

/// Get autocomplete matches for a partial command.
pub fn autocomplete(prefix: &str) -> Vec<&'static str> {
    let commands = all_commands();
    // Leak the vec to get 'static references (it's a small, fixed list)
    let commands: &'static [Command] = Box::leak(commands.into_boxed_slice());
    commands
        .iter()
        .filter(|c| c.name.starts_with(prefix))
        .map(|c| c.name)
        .collect()
}

/// Get autocomplete entries for display (name + description).
pub fn autocomplete_entries(prefix: &str) -> Vec<(&'static str, &'static str)> {
    let commands = all_commands();
    let commands: &'static [Command] = Box::leak(commands.into_boxed_slice());
    commands
        .iter()
        .filter(|c| c.name.starts_with(prefix))
        .map(|c| (c.name, c.description))
        .collect()
}
