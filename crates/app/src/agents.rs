//! Driving AI agents (Hermes Agent) over Matrix.
//!
//! Hermes's Matrix adapter speaks a small protocol on top of ordinary
//! events, which this module reads back:
//! - a request (one of your messages) gets a 👀 reaction while it's being
//!   worked on, swapped for ✅ (done) or ❌ (failed) at the end;
//! - the answer goes in a thread on that message (auto-threading);
//! - approvals and pickers are a message the bot reacts to itself with the
//!   choices (✅ once, 🌀 session, ♾️ always, ❌ deny; numbered emoji for
//!   pickers). You answer by reacting; the bot takes its reactions back once
//!   it's resolved, so "the sender's own reactions are still on it" means
//!   it's waiting for you;
//! - commands are `!name` (Matrix clients keep `/` for themselves).
//!
//! Which users are agents is the user's call (`Prefs::agents`, toggled on
//! the profile card).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use fluxchat_core::timeline::{parse_thread_key, real_room_id, thread_key};
use fluxchat_core::MessageInfo;

use iced::widget::{button, container, row, text, Space};
use iced::{Alignment, Element, Fill, Theme};

use crate::message::Message;
use crate::state::FluxChat;
use crate::ui::{self, Tone};

/// Reactions an agent puts on a request: working on it / done / failed.
pub const WORKING: &str = "\u{1f440}";
pub const DONE: &str = "\u{2705}";
pub const FAILED: &str = "\u{274c}";

/// A prompt older than this isn't waiting any more (Hermes times approvals
/// out after 5 minutes; reactions left behind by a restart would otherwise
/// show as a live prompt forever).
const PROMPT_MAX_AGE_MS: u64 = 60 * 60 * 1000;
/// Only a request sent this recently has its new thread opened for you.
const FOLLOW_WINDOW_MS: u64 = 15 * 60 * 1000;

/// Where a run (a request and the agent's work on it) stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Working,
    /// Waiting on an approval or a choice from you.
    NeedsYou,
    Done,
    Failed,
}

impl RunState {
    pub fn label(self) -> &'static str {
        match self {
            RunState::Working => "working",
            RunState::NeedsYou => "needs you",
            RunState::Done => "done",
            RunState::Failed => "failed",
        }
    }

    pub fn color(self) -> iced::Color {
        let c = crate::theme::colors();
        match self {
            RunState::Working => c.accent,
            RunState::NeedsYou => c.yellow,
            RunState::Done => c.green,
            RunState::Failed => c.red,
        }
    }

    /// Worth flagging in lists (finished runs aren't).
    pub fn active(self) -> bool {
        matches!(self, RunState::Working | RunState::NeedsYou | RunState::Failed)
    }
}

/// What a message sent while the agent is working does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BusyMode {
    /// Plain message: the agent's own busy policy (Hermes: redirect the run).
    #[default]
    Send,
    /// `!steer`: read after the next tool call, without interrupting.
    Steer,
    /// `!queue`: the next turn, after this one.
    Queue,
}

impl BusyMode {
    pub const ALL: [BusyMode; 3] = [BusyMode::Send, BusyMode::Steer, BusyMode::Queue];

    pub fn label(self) -> &'static str {
        match self {
            BusyMode::Send => "Send",
            BusyMode::Steer => "Steer",
            BusyMode::Queue => "Queue",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            BusyMode::Send => "A plain message: the agent decides (Hermes redirects the run by default)",
            BusyMode::Steer => "Sent as !steer: read after the agent's next step, without interrupting",
            BusyMode::Queue => "Sent as !queue: runs as the next turn, after this one",
        }
    }

    fn prefix(self) -> Option<&'static str> {
        match self {
            BusyMode::Send => None,
            BusyMode::Steer => Some("!steer "),
            BusyMode::Queue => Some("!queue "),
        }
    }
}

/// Hermes gateway commands, from its slash-command reference. In an agent
/// room these go to the agent (as `!name`), shadowing fluxchat's own.
pub const AGENT_COMMANDS: &[(&str, &str)] = &[
    ("/stop", "Interrupt the running turn and stop background processes"),
    ("/steer", "Tell the running turn something, after its next step"),
    ("/queue", "Queue a prompt for the next turn"),
    ("/q", "Queue a prompt for the next turn"),
    ("/bg", "Run a prompt in a separate background session"),
    ("/btw", "Side question, answered without interrupting"),
    ("/goal", "Set a standing goal (status, pause, resume, clear)"),
    ("/subgoal", "Add criteria to the active goal"),
    ("/new", "Start a fresh session"),
    ("/reset", "Start a fresh session"),
    ("/retry", "Resend the last message"),
    ("/undo", "Remove the last exchange"),
    ("/status", "Session info and recap"),
    ("/usage", "Tokens, cost and context"),
    ("/context", "Context-window gauge"),
    ("/model", "Show or switch the model"),
    ("/reasoning", "Reasoning effort / display"),
    ("/fast", "Fast mode"),
    ("/personality", "Apply a personality"),
    ("/title", "Show or set the session title"),
    ("/sessions", "List previous sessions"),
    ("/resume", "Resume a named session"),
    ("/branch", "Branch the session into a new thread"),
    ("/fork", "Branch the session into a new thread"),
    ("/compress", "Compress the context"),
    ("/tasks", "Active agents and running tasks"),
    ("/agents", "Active agents and running tasks"),
    ("/approve", "Approve the pending command (session, always)"),
    ("/deny", "Deny the pending command"),
    ("/yolo", "Toggle skipping approval prompts"),
    ("/diff", "Show git changes"),
    ("/rollback", "List or restore checkpoints"),
    ("/plan", "Write a plan without executing"),
    ("/review", "Have a reviewer check recent work"),
    ("/loop", "Re-run a prompt on an interval"),
    ("/heartbeat", "Recurring prompt while idle"),
    ("/learn", "Distil a reusable skill"),
    ("/skills", "Review pending skill writes"),
    ("/memory", "Review pending memory writes"),
    ("/footer", "Toggle the runtime footer"),
    ("/verbose", "Cycle tool progress display"),
    ("/insights", "Usage analytics"),
    ("/commands", "Browse all commands and skills"),
    ("/help", "The agent's help"),
    ("/version", "Agent version"),
    ("/restart", "Restart the gateway"),
];

/// The state an agent's reactions give a request (one of your messages).
pub fn request_state(msg: &MessageInfo, agent: &str) -> Option<RunState> {
    let by_agent = |emoji: &str| msg.reactions.iter()
        .any(|r| r.emoji == emoji && r.users.iter().any(|u| u == agent));
    // ✅/❌ win: 👀 is taken back a few seconds after they land.
    if by_agent(FAILED) {
        Some(RunState::Failed)
    } else if by_agent(DONE) {
        Some(RunState::Done)
    } else if by_agent(WORKING) {
        Some(RunState::Working)
    } else {
        None
    }
}

/// The choices of a prompt still waiting for an answer: the emoji its
/// sender reacted to it with, labelled. Empty when it isn't a prompt, it's
/// been resolved, or it's too old to still be live (`now_ms`).
pub fn prompt_choices(msg: &MessageInfo, now_ms: u64) -> Vec<(String, String)> {
    if now_ms.saturating_sub(msg.timestamp) > PROMPT_MAX_AGE_MS {
        return Vec::new();
    }
    msg.reactions.iter()
        .filter(|r| r.users.contains(&msg.sender))
        .map(|r| (r.emoji.clone(), choice_label(&r.emoji, &msg.body)))
        .collect()
}

/// Approval choices by their emoji; pickers list theirs as body lines that
/// start with the emoji.
fn choice_label(emoji: &str, body: &str) -> String {
    let known = match emoji.trim_end_matches('\u{fe0f}') {
        "\u{2705}" => Some("Approve once"),
        "\u{1f300}" => Some("For this session"),
        "\u{267e}" => Some("Always"),
        "\u{274c}" | "\u{274e}" => Some("Deny"),
        _ => None,
    };
    if let Some(label) = known {
        return label.into();
    }
    body.lines()
        .find_map(|l| l.trim().strip_prefix(emoji))
        .map(|rest| rest.trim().replace('`', ""))
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| emoji.to_string())
}

/// `/name args` → `!name args` in an agent room: an agent command, or a name
/// fluxchat doesn't know (skills: `/arxiv …`). None = one of fluxchat's own
/// (`/me`, `/gif`, …) or not a command.
pub fn agent_command(input: &str) -> Option<String> {
    let rest = input.strip_prefix('/')?;
    let name = rest.split_whitespace().next().filter(|n| !n.starts_with('/'))?;
    let slash = format!("/{name}");
    let ours = crate::commands::all_commands().iter().any(|c| c.name == slash);
    let theirs = AGENT_COMMANDS.iter().any(|(n, _)| *n == slash);
    (theirs || !ours).then(|| format!("!{rest}"))
}

/// Slash completions in an agent room: the agent's commands, then
/// fluxchat's that they don't shadow.
pub fn completions(prefix: &str) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = AGENT_COMMANDS.iter()
        .filter(|(n, _)| n.starts_with(prefix))
        .copied()
        .collect();
    out.extend(crate::commands::autocomplete_entries(prefix).into_iter()
        .filter(|(n, _)| !AGENT_COMMANDS.iter().any(|(a, _)| a == n)));
    out
}

pub fn is_agent_command(name: &str) -> bool {
    AGENT_COMMANDS.iter().any(|(n, _)| *n == name)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Run states worked out from the loaded timelines (`refresh_agent_state`).
#[derive(Debug, Default)]
pub struct AgentRuns {
    /// By thread key (a request answered in a thread) or room key (one that
    /// wasn't threaded).
    states: HashMap<String, RunState>,
    /// Prompts already seen, by event ID (each is announced once).
    announced: std::collections::HashSet<String>,
    /// Messages a desktop notification was shown for (so a prompt that
    /// already notified as a message isn't announced twice).
    pub(crate) notified: std::collections::HashSet<String>,
    /// Rooms with a run waiting on you (from `states`; the room list asks
    /// for every row on every render).
    needs_you: std::collections::HashSet<String>,
}

impl AgentRuns {
    fn set(&mut self, key: String, state: Option<RunState>) {
        match state {
            Some(s) => { self.states.insert(key, s); }
            None => { self.states.remove(&key); }
        }
    }

    fn recount_needs_you(&mut self) {
        self.needs_you = self.states.iter()
            .filter(|(_, s)| **s == RunState::NeedsYou)
            .map(|(k, _)| real_room_id(k).to_string())
            .collect();
    }
}

impl FluxChat {
    /// The agent in a view's room: a member — or, before members load, a
    /// sender or reactor (its 👀 can come before any reply) — that the user
    /// marked as an agent.
    pub(crate) fn agent_in(&self, key: &str) -> Option<&str> {
        if self.prefs.agents.is_empty() {
            return None;
        }
        let room = real_room_id(key);
        let agent = |u: &str| self.prefs.agents.iter().find(|a| *a == u).map(String::as_str);
        [key, room].iter()
            .filter_map(|k| self.members_by_room.get(*k))
            .flatten()
            .find_map(|m| agent(&m.user_id))
            .or_else(|| [key, room].iter()
                .filter_map(|k| self.messages_by_room.get(*k))
                .flatten()
                .find_map(|m| agent(&m.sender).or_else(|| m.reactions.iter()
                    .flat_map(|r| &r.users)
                    .find_map(|u| agent(u)))))
    }

    /// Where the run behind a view stands: a thread key for a threaded
    /// request, a room key for the room's latest unthreaded one.
    pub(crate) fn run_state(&self, key: &str) -> Option<RunState> {
        self.agent_runs.states.get(key).copied()
    }

    /// Any run in the room (or its threads) waiting on you.
    pub(crate) fn room_needs_you(&self, room_id: &str) -> bool {
        self.agent_runs.needs_you.contains(room_id)
    }

    /// A thread view closed: forget what it said, and let the room's view
    /// (the request's reactions) speak for the thread again.
    pub(crate) fn forget_agent_view(&mut self, key: &str) {
        self.agent_runs.set(key.to_string(), None);
        self.agent_runs.recount_needs_you();
        let room = real_room_id(key).to_string();
        if room != key {
            self.refresh_agent_state(&room);
        }
    }

    /// The prompt choices to offer on `msg` (only from an agent).
    pub(crate) fn agent_prompt(&self, msg: &MessageInfo) -> Vec<(String, String)> {
        if !self.prefs.agents.contains(&msg.sender) {
            return Vec::new();
        }
        prompt_choices(msg, now_ms())
    }

    /// What to send for typed `text` in view `key` when an agent is there:
    /// agent commands as `!name`, and while it's working, the busy mode's
    /// prefix. None = no agent / a fluxchat command / plain text as typed.
    pub(crate) fn agent_outgoing(&self, key: &str, text: &str) -> Option<String> {
        self.agent_in(key)?;
        if text.starts_with('/') {
            return if text.starts_with("//") { None } else { agent_command(text) };
        }
        if text.starts_with('!') || self.run_state(key) != Some(RunState::Working) {
            return None;
        }
        self.prefs.agent_busy_mode.prefix().map(|p| format!("{p}{text}"))
    }

    /// Recompute the run states a view's messages give (after they change).
    /// Returns the event IDs of prompts that just appeared, to announce.
    pub(crate) fn refresh_agent_state(&mut self, key: &str) -> Vec<String> {
        let Some(agent) = self.agent_in(key).map(str::to_string) else { return Vec::new() };
        let Some(msgs) = self.messages_by_room.get(key) else { return Vec::new() };
        let me = self.user_id.clone();
        let now = now_ms();
        let prompts: Vec<&MessageInfo> = msgs.iter()
            .filter(|m| m.sender == agent && !prompt_choices(m, now).is_empty())
            .collect();
        let fresh: Vec<String> = prompts.iter()
            .filter(|m| !self.agent_runs.announced.contains(&m.event_id))
            .map(|m| m.event_id.clone())
            .collect();
        let needs_you = !prompts.is_empty();
        let mut updates: Vec<(String, Option<RunState>)> = Vec::new();
        if parse_thread_key(key).is_some() {
            // A thread: its latest request (the root or a reply of yours).
            let latest = msgs.iter().rev().filter(|m| m.sender == me).find_map(|m| request_state(m, &agent));
            updates.push((key.to_string(), if needs_you { Some(RunState::NeedsYou) } else { latest }));
        } else {
            // Threaded requests, unless their thread is open (its own view
            // knows better, approvals included).
            let mut latest_unthreaded = None;
            for m in msgs.iter().filter(|m| m.sender == me && m.thread_root.is_none()) {
                let Some(state) = request_state(m, &agent) else { continue };
                if m.thread_count.unwrap_or(0) > 0 {
                    let tk = thread_key(key, &m.event_id);
                    if !self.messages_by_room.contains_key(&tk) {
                        updates.push((tk, Some(state)));
                    }
                } else {
                    latest_unthreaded = Some(state);
                }
            }
            updates.push((key.to_string(), if needs_you { Some(RunState::NeedsYou) } else { latest_unthreaded }));
        }
        for (k, state) in updates {
            self.agent_runs.set(k, state);
        }
        self.agent_runs.recount_needs_you();
        self.agent_runs.announced.extend(fresh.iter().cloned());
        // Only prompts from the last couple of minutes are news (not ones
        // found in history when a view loads).
        let msgs = self.messages_by_room.get(key).map(Vec::as_slice).unwrap_or(&[]);
        fresh.into_iter()
            .filter(|id| msgs.iter().any(|m| m.event_id == *id && now.saturating_sub(m.timestamp) < 2 * 60 * 1000))
            .collect()
    }

    /// A desktop notification for each prompt that just appeared in view
    /// `key`, unless you're looking at it or it already notified as a
    /// message. Sent whatever the mentions-only setting says: the agent is
    /// stuck until you answer.
    pub(crate) fn announce_prompts(&mut self, key: &str, event_ids: &[String]) {
        let room = real_room_id(key).to_string();
        if !self.notifications_enabled || (self.app_focused && self.room_visible(&room)) {
            return;
        }
        let Some(msgs) = self.messages_by_room.get(key) else { return };
        for id in event_ids {
            if self.agent_runs.notified.contains(id) {
                continue;
            }
            let Some(m) = msgs.iter().find(|m| m.event_id == *id) else { continue };
            let what = m.body.lines().map(|l| l.trim().trim_matches(|c: char| c == '*' || c == '⚠' || c == '\u{fe0f}').trim())
                .find(|l| !l.is_empty())
                .unwrap_or("Waiting for your answer")
                .chars().take(200).collect::<String>();
            let room_name = self.rooms.iter().find(|r| r.room_id == room).map(|r| r.name.clone()).unwrap_or_default();
            crate::update::show_notification(
                format!("{} needs you - {room_name}", m.sender_name),
                what,
                self.notify_sound,
                Some((self.timeline_tx.clone(), room.clone())),
            );
            self.agent_runs.notified.insert(id.clone());
        }
    }

    /// The thread to open because the agent just started one on a request
    /// of yours in room `key` (comparing the old messages with `new`).
    pub(crate) fn thread_to_follow(&self, key: &str, new: &[MessageInfo]) -> Option<String> {
        if !self.prefs.follow_agent_threads || key.starts_with("thread::")
            || self.selected_room_id.as_deref() != Some(key)
        {
            return None;
        }
        // Only a room that was already loaded: on first load every recent
        // thread would look new.
        let old = self.messages_by_room.get(key)?;
        let agent = self.agent_in(key)?;
        let now = now_ms();
        new.iter().rev()
            .filter(|m| m.sender == self.user_id && m.thread_root.is_none())
            .filter(|m| now.saturating_sub(m.timestamp) < FOLLOW_WINDOW_MS)
            .filter(|m| m.thread_count.unwrap_or(0) > 0)
            .filter(|m| request_state(m, agent).is_some()
                || m.thread_latest.as_ref().is_some_and(|l| l.sender == agent))
            .find(|m| !old.iter().any(|o| o.event_id == m.event_id && o.thread_count.unwrap_or(0) > 0))
            .map(|m| thread_key(key, &m.event_id))
    }
}

// --- Views ---------------------------------------------------------------

/// Width of the Send / Steer / Queue switch in the run bar.
const BUSY_SWITCH_WIDTH: f32 = 190.0;

/// A small round dot in a run state's colour.
pub(crate) fn state_dot<'a>(state: RunState, size: f32) -> Element<'a, Message> {
    container(Space::new().width(size).height(size))
        .style(move |_: &Theme| container::Style {
            background: Some(state.color().into()),
            border: iced::Border { radius: (size / 2.0).into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

impl FluxChat {
    /// How the agent is called in the view's room.
    fn agent_name(&self, key: &str, agent: &str) -> String {
        [key, real_room_id(key)].iter()
            .filter_map(|k| self.members_by_room.get(*k))
            .flatten()
            .find(|m| m.user_id == agent)
            .map(|m| m.display_name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| agent.trim_start_matches('@').split(':').next().unwrap_or(agent).to_string())
    }

    /// The bar above a view's composer while the agent is on its run: what
    /// it's doing, what your next message will do, and Stop. `compact` (side
    /// panes) leaves out the switch.
    pub(crate) fn view_run_bar(&self, key: &str, compact: bool) -> Option<Element<'_, Message>> {
        let agent = self.agent_in(key)?;
        let state = self.run_state(key).filter(|s| matches!(s, RunState::Working | RunState::NeedsYou))?;
        let name = self.agent_name(key, agent);
        let label = if state == RunState::Working {
            format!("{name} is working")
        } else {
            format!("{name} is waiting for your answer")
        };
        let mut bar = row![
            state_dot(state, 8.0),
            text(label).font(self.sidebar_font()).size(12).color(crate::theme::colors().text_normal)
                .wrapping(iced::widget::text::Wrapping::None),
            Space::new().width(Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        if state == RunState::Working && !compact {
            let mode = self.prefs.agent_busy_mode;
            bar = bar.push(text("Next message").font(self.sidebar_font()).size(11).color(crate::theme::colors().text_dim));
            let switch = ui::segmented(BusyMode::ALL.iter()
                .map(|m| (m.label().to_string(), Some(Message::SetBusyMode(*m)), *m == mode))
                .collect());
            bar = bar.push(ui::with_tooltip(container(switch).width(BUSY_SWITCH_WIDTH), mode.hint()));
        }
        bar = bar.push(ui::with_tooltip(
            ui::icon_btn(crate::icons::SVG_STOP, "Stop", Tone::Danger, Some(Message::AgentSend(key.to_string(), "!stop".into())))
                .padding([3, 10]),
            "Interrupt the run (!stop)",
        ));
        let tint = state.color();
        Some(container(bar)
            .padding([4, 12])
            .width(Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(crate::theme::with_alpha(tint, 0.08).into()),
                border: iced::Border { color: crate::theme::with_alpha(tint, 0.25), width: 1.0, radius: 0.0.into() },
                ..Default::default()
            })
            .into())
    }

    /// An agent's approval / picker prompt as buttons (each sends its
    /// reaction). The one you picked stays lit until the agent resolves it.
    pub(crate) fn view_prompt_card<'a>(&self, msg: &'a MessageInfo, choices: Vec<(String, String)>) -> Element<'a, Message> {
        let picked = |emoji: &str| msg.reactions.iter()
            .any(|r| r.emoji == emoji && r.users.contains(&self.user_id));
        let answered = choices.iter().any(|(e, _)| picked(e));
        let mut buttons = row![].spacing(6).align_y(Alignment::Center);
        for (emoji, label) in choices {
            let tone = if label == "Deny" {
                Tone::Danger
            } else if picked(&emoji) {
                Tone::Primary
            } else {
                Tone::Secondary
            };
            let on_press = (!answered).then(|| Message::ToggleReaction(msg.event_id.clone(), emoji.clone()));
            let lit = picked(&emoji);
            buttons = buttons.push(
                button(row![
                    text(emoji).size(14),
                    text(label).font(self.sidebar_font()).size(12),
                ].spacing(6).align_y(Alignment::Center))
                .on_press_maybe(on_press)
                .padding([4, 10])
                .style(move |t, status| {
                    let mut s = ui::button_style(tone)(t, status);
                    if lit {
                        s.border = iced::Border { color: crate::theme::colors().accent, width: 1.0, radius: ui::RADIUS.into() };
                    }
                    s
                }),
            );
        }
        let status = if answered { "Sent, waiting for the agent\u{2026}" } else { "Waiting for you" };
        row![
            Space::new().width(iced::Length::Fixed(self.msg_content_indent())),
            container(
                iced::widget::column![
                    row![
                        state_dot(RunState::NeedsYou, 7.0),
                        text(status).font(self.sidebar_font()).size(11).color(crate::theme::colors().text_dim),
                    ].spacing(6).align_y(Alignment::Center),
                    buttons,
                ].spacing(6),
            )
            .padding([8, 10])
            .style(|_: &Theme| container::Style {
                background: Some(crate::theme::with_alpha(crate::theme::colors().yellow, 0.06).into()),
                border: iced::Border {
                    color: crate::theme::with_alpha(crate::theme::colors().yellow, 0.3),
                    width: 1.0,
                    radius: ui::RADIUS.into(),
                },
                ..Default::default()
            }),
        ]
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluxchat_core::ReactionInfo;

    fn react(emoji: &str, users: &[&str]) -> ReactionInfo {
        ReactionInfo { emoji: emoji.into(), count: users.len() as u32, users: users.iter().map(|u| u.to_string()).collect() }
    }

    #[test]
    fn a_requests_reactions_give_its_state() {
        let mut m = MessageInfo { sender: "@me:x".into(), ..Default::default() };
        assert_eq!(request_state(&m, "@bot:x"), None);
        m.reactions = vec![react(WORKING, &["@bot:x"])];
        assert_eq!(request_state(&m, "@bot:x"), Some(RunState::Working));
        // Someone else's 👀 is just a reaction.
        assert_eq!(request_state(&m, "@other:x"), None);
        m.reactions.push(react(DONE, &["@bot:x"]));
        assert_eq!(request_state(&m, "@bot:x"), Some(RunState::Done), "✅ wins over the 👀 not yet taken back");
        m.reactions = vec![react(FAILED, &["@bot:x"])];
        assert_eq!(request_state(&m, "@bot:x"), Some(RunState::Failed));
    }

    #[test]
    fn a_prompt_is_the_senders_own_reactions_and_ends_when_they_go() {
        let now = 10_000_000;
        let body = "⚠️ Dangerous command\n`rm -rf build`\n\nReact:\n✅ once\n♾️ always\n❌ deny";
        let mut m = MessageInfo { sender: "@bot:x".into(), body: body.into(), timestamp: now - 1000, ..Default::default() };
        m.reactions = vec![react("✅", &["@bot:x"]), react("♾️", &["@bot:x"]), react("❌", &["@bot:x"])];
        let labels: Vec<String> = prompt_choices(&m, now).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["Approve once", "Always", "Deny"]);
        // Answered and resolved: the bot took its reactions back.
        m.reactions = vec![react("✅", &["@me:x"])];
        assert!(prompt_choices(&m, now).is_empty());
        // Left over from long ago: not live.
        m.reactions = vec![react("✅", &["@bot:x"])];
        assert!(prompt_choices(&m, now + PROMPT_MAX_AGE_MS + 1).is_empty());
    }

    #[test]
    fn picker_choices_are_labelled_from_their_lines() {
        let body = "Pick a model\n1️⃣ `claude-opus` — Anthropic\n2️⃣ `gpt-5` — OpenAI";
        let m = MessageInfo {
            sender: "@bot:x".into(), body: body.into(), timestamp: 5,
            reactions: vec![react("1️⃣", &["@bot:x"]), react("2️⃣", &["@bot:x"])],
            ..Default::default()
        };
        let labels: Vec<String> = prompt_choices(&m, 10).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["claude-opus — Anthropic", "gpt-5 — OpenAI"]);
    }

    fn agent_app() -> FluxChat {
        let mut app = crate::panes::flow_tests::app();
        app.user_id = "@me:x".into();
        app.prefs.agents = vec!["@bot:x".into()];
        app
    }

    fn msg(id: &str, sender: &str, ts: u64) -> MessageInfo {
        MessageInfo { event_id: id.into(), sender: sender.into(), body: id.into(), timestamp: ts, ..Default::default() }
    }

    #[tokio::test]
    async fn runs_come_from_the_room_and_their_threads() {
        let mut app = agent_app();
        let now = now_ms();
        let mut ask = msg("$ask", "@me:x", now - 5000);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        ask.thread_count = Some(1);
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now - 9000), ask.clone()]);
        app.refresh_agent_state("!r:x");
        let tk = thread_key("!r:x", "$ask");
        assert_eq!(app.run_state(&tk), Some(RunState::Working));
        assert!(!app.room_needs_you("!r:x"));
        // The thread opens; the agent asks for approval in it.
        let mut prompt = msg("$approve", "@bot:x", now - 1000);
        prompt.thread_root = Some("$ask".into());
        prompt.reactions = vec![react("\u{2705}", &["@bot:x"]), react("\u{274c}", &["@bot:x"])];
        app.messages_by_room.insert(tk.clone(), vec![ask.clone(), prompt.clone()]);
        let fresh = app.refresh_agent_state(&tk);
        assert_eq!(fresh, vec!["$approve".to_string()]);
        assert_eq!(app.run_state(&tk), Some(RunState::NeedsYou));
        assert!(app.room_needs_you("!r:x"));
        assert!(app.refresh_agent_state(&tk).is_empty(), "announced once");
        // Approved and finished: the bot took its reactions back and ✅'d the request.
        prompt.reactions = vec![react("\u{2705}", &["@me:x"])];
        ask.reactions = vec![react(DONE, &["@bot:x"])];
        app.messages_by_room.insert(tk.clone(), vec![ask, prompt]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::Done));
        assert!(!app.room_needs_you("!r:x"));
    }

    #[tokio::test]
    async fn only_a_new_thread_on_a_recent_request_in_the_room_youre_in_is_followed() {
        let mut app = agent_app();
        let _ = app.update(Message::SelectRoom("!r:x".into()));
        let now = now_ms();
        let mut ask = msg("$ask", "@me:x", now - 3000);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        // First load: nothing to compare with, nothing followed.
        let mut threaded = ask.clone();
        threaded.thread_count = Some(1);
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&threaded)), None);
        app.messages_by_room.insert("!r:x".into(), vec![ask.clone()]);
        // The agent's first reply creates the thread.
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&threaded)), Some(thread_key("!r:x", "$ask")));
        // Already threaded before: not again.
        app.messages_by_room.insert("!r:x".into(), vec![threaded.clone()]);
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&threaded)), None);
        // An old request, or someone else's thread: no.
        let mut old = threaded.clone();
        old.timestamp = now - FOLLOW_WINDOW_MS - 1;
        app.messages_by_room.insert("!r:x".into(), vec![ask.clone()]);
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&old)), None);
        // Turned off, or not the room you're in: no.
        app.prefs.follow_agent_threads = false;
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&threaded)), None);
        app.prefs.follow_agent_threads = true;
        let _ = app.update(Message::SelectRoom("!other:x".into()));
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&threaded)), None);
    }

    #[tokio::test]
    async fn outgoing_messages_get_agent_commands_and_the_busy_mode() {
        let mut app = agent_app();
        let now = now_ms();
        // No agent in the room: untouched.
        app.messages_by_room.insert("!plain:x".into(), vec![msg("$a", "@friend:x", now)]);
        assert_eq!(app.agent_outgoing("!plain:x", "/goal x"), None);
        // With the agent.
        let mut ask = msg("$ask", "@me:x", now);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now), ask]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.agent_outgoing("!r:x", "/goal x").as_deref(), Some("!goal x"));
        assert_eq!(app.agent_outgoing("!r:x", "/me waves"), None);
        // Working (unthreaded run): plain text follows the busy mode.
        assert_eq!(app.agent_outgoing("!r:x", "also check B"), None, "Send: as typed");
        app.prefs.agent_busy_mode = BusyMode::Steer;
        assert_eq!(app.agent_outgoing("!r:x", "also check B").as_deref(), Some("!steer also check B"));
        app.prefs.agent_busy_mode = BusyMode::Queue;
        assert_eq!(app.agent_outgoing("!r:x", "then C").as_deref(), Some("!queue then C"));
        assert_eq!(app.agent_outgoing("!r:x", "!stop"), None, "explicit commands as typed");
    }

    #[test]
    fn commands_go_to_the_agent_unless_they_are_fluxchats_own() {
        assert_eq!(agent_command("/goal win the week").as_deref(), Some("!goal win the week"));
        assert_eq!(agent_command("/status").as_deref(), Some("!status"), "the agent's /status wins in its room");
        assert_eq!(agent_command("/arxiv transformers").as_deref(), Some("!arxiv transformers"), "skills pass through");
        assert_eq!(agent_command("/me waves"), None, "fluxchat's /me still works");
        assert_eq!(agent_command("//literal"), None);
        assert_eq!(agent_command("hello"), None);
        assert!(completions("/st").iter().any(|(n, _)| *n == "/stop"));
        assert_eq!(completions("/status").iter().filter(|(n, _)| *n == "/status").count(), 1);
    }
}
