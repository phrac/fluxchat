//! Driving AI agents over Matrix.
//!
//! Agent harnesses speak small protocols on top of ordinary events, which
//! this module reads back. What each one does is its profile (`harness.rs`);
//! the shapes are shared:
//! - a request (one of your messages) gets a reaction while it's worked on
//!   (Hermes: 👀), swapped for done / failed ones (✅ / ❌) at the end — or
//!   the agent just types while it works (`typing_means_working`);
//! - the answer often goes in a thread on that message;
//! - approvals and pickers are a message the bot reacts to itself with the
//!   choices; you answer by reacting, and the bot takes its reactions back
//!   once resolved, so "the sender's own reactions are still on it" means
//!   it's waiting for you (Hermes, the Claude Code Matrix channel);
//! - commands are `!name` or `/name` text, per harness.
//!
//! Which users are agents, and which harness runs each, is the user's call
//! (`Prefs::agents` / `agent_harness`, on the profile card).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use fluxchat_core::timeline::{parse_thread_key, real_room_id, thread_key};
use fluxchat_core::MessageInfo;

use iced::widget::{button, container, row, text, Space};
use iced::{Alignment, Element, Fill, Theme};

use crate::harness::Harness;
use crate::message::Message;
use crate::state::FluxChat;
use crate::ui::{self, Tone};

/// A prompt older than this isn't waiting any more (Hermes times approvals
/// out after 5 minutes; reactions left behind by a restart would otherwise
/// show as a live prompt forever).
const PROMPT_MAX_AGE_MS: u64 = 60 * 60 * 1000;
/// Panes a "watch runs" window fills up to (new runs beyond it wait).
const RUN_WINDOW_PANES: usize = 6;
/// Finished runs this recent still show in the Runs dialog.
const RECENT_RUN_MS: u64 = 6 * 60 * 60 * 1000;
/// A run shorter than this finishes without a notification (you were
/// probably still looking), unless you asked to be told.
const DONE_NOTIFY_MIN_MS: u64 = 30_000;
/// Only a request sent this recently has its new thread opened for you.
const FOLLOW_WINDOW_MS: u64 = 15 * 60 * 1000;
/// A progress line longer than this is prose, not a step.
const STEP_MAX_CHARS: usize = 300;

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
            BusyMode::Steer => "Read after the agent's next step, without interrupting (steer)",
            BusyMode::Queue => "Runs as the next turn, after this one (queue)",
        }
    }

    /// What goes in front of a message in this mode, for harness `h`
    /// (None: sent as typed, or the harness has no such command).
    fn prefix(self, h: &Harness) -> Option<String> {
        let name = match self {
            BusyMode::Send => return None,
            BusyMode::Steer => &h.steer,
            BusyMode::Queue => &h.queue,
        };
        (!name.is_empty()).then(|| format!("{} ", h.command(name)))
    }

    /// The modes harness `h` supports (Send always).
    pub fn available(h: &Harness) -> Vec<BusyMode> {
        BusyMode::ALL.into_iter().filter(|m| *m == BusyMode::Send || m.prefix(h).is_some()).collect()
    }
}


/// The state an agent's reactions give a request (one of your messages),
/// with the reactions harness `h` uses.
pub fn request_state(h: &Harness, msg: &MessageInfo, agent: &str) -> Option<RunState> {
    let by_agent = |emoji: &str| !emoji.is_empty() && msg.reactions.iter()
        .any(|r| r.emoji == emoji && r.users.iter().any(|u| u == agent));
    // Done / failed win: the working one is taken back a few seconds later.
    if by_agent(&h.failed_reaction) {
        Some(RunState::Failed)
    } else if by_agent(&h.done_reaction) {
        Some(RunState::Done)
    } else if by_agent(&h.working_reaction) {
        Some(RunState::Working)
    } else {
        None
    }
}

/// The choices of a prompt still waiting for an answer: the emoji its
/// sender reacted to it with, labelled. Empty when it isn't a prompt, it's
/// been resolved, or it's too old to still be live (`now_ms`).
pub fn prompt_choices(msg: &MessageInfo, now_ms: u64, labels: &[crate::harness::Choice]) -> Vec<(String, String)> {
    if now_ms.saturating_sub(msg.timestamp) > PROMPT_MAX_AGE_MS {
        return Vec::new();
    }
    msg.reactions.iter()
        .filter(|r| r.users.contains(&msg.sender))
        .map(|r| (r.emoji.clone(), choice_label(&r.emoji, &msg.body, labels)))
        .collect()
}

/// A choice's label: the harness's, else the usual approval meaning of the
/// emoji, else (pickers) the body line that starts with it.
fn choice_label(emoji: &str, body: &str, labels: &[crate::harness::Choice]) -> String {
    let bare = |e: &str| e.trim_end_matches('\u{fe0f}').to_string();
    if let Some(c) = labels.iter().find(|c| bare(&c.emoji) == bare(emoji)) {
        return c.label.clone();
    }
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

/// `/name args` typed in an agent room, as harness `h` takes it (`!name
/// args` for Hermes): one of its commands, or a name fluxchat doesn't know
/// (skills: `/arxiv …`). None = one of fluxchat's own (`/me`, `/gif`, …) or
/// not a command.
pub fn agent_command(h: &Harness, input: &str) -> Option<String> {
    let rest = input.strip_prefix('/')?;
    let name = rest.split_whitespace().next().filter(|n| !n.starts_with('/'))?;
    let ours = crate::commands::all_commands().iter().any(|c| c.name.strip_prefix('/') == Some(name));
    (h.has_command(name) || !ours).then(|| format!("{}{rest}", h.command_prefix))
}

/// Slash completions in an agent room: its harness's commands, then
/// fluxchat's that they don't shadow.
pub fn completions(h: &Harness, prefix: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = h.commands.iter()
        .map(|c| (format!("/{}", c.name), c.description.clone()))
        .filter(|(n, _)| n.starts_with(prefix))
        .collect();
    out.extend(crate::commands::autocomplete_entries(prefix).into_iter()
        .filter(|(n, _)| !is_agent_command(h, n))
        .map(|(n, d)| (n.to_string(), d.to_string())));
    out
}

/// `/name` is one of harness `h`'s commands.
pub fn is_agent_command(h: &Harness, slash_name: &str) -> bool {
    slash_name.strip_prefix('/').is_some_and(|n| h.has_command(n))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// One step line of an agent's progress message: a tool emoji, a space,
/// what it's doing ("📖 Reading notes.md", "🔍 Searching the web for …").
fn is_step(line: &str) -> bool {
    let Some((head, tail)) = line.split_once(' ') else { return false };
    !tail.trim().is_empty()
        && line.chars().count() <= STEP_MAX_CHARS
        && (emojis::get(head).is_some() || emojis::get(head.trim_end_matches('\u{fe0f}')).is_some())
}

/// Add a pane for `key` to a watch window: split its newest pane, side by
/// side and stacked in turn, so the panes tile.
fn add_run_pane(w: &mut crate::panes::Window, key: String) {
    let order = crate::panes::layout_order(w.grid.layout());
    let Some(last) = order.last().copied() else { return };
    let axis = if order.len() % 2 == 1 {
        iced::widget::pane_grid::Axis::Vertical
    } else {
        iced::widget::pane_grid::Axis::Horizontal
    };
    w.grid.restore();
    let _ = w.grid.split(axis, last, crate::panes::PaneSlot { key: Some(key) });
}

/// The steps of an agent progress message: every line a step. Hermes edits
/// one such message, adding a line per tool it starts.
pub fn progress_steps(body: &str) -> Option<Vec<&str>> {
    let lines: Vec<&str> = body.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    (!lines.is_empty() && lines.iter().all(|l| is_step(l))).then_some(lines)
}

/// The agent's runtime info from Hermes's footer (`/footer on`): the last
/// paragraph of a final answer, `model · 42% · ~/dir` (any of those, plus
/// latency like `22s`, or `alias → served`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Runtime {
    pub model: Option<String>,
    pub context_pct: Option<u8>,
    pub cwd: Option<String>,
    pub latency: Option<String>,
    /// Spend so far ("$0.18"), from the agent protocol.
    pub cost: Option<String>,
}

impl Runtime {
    /// "claude-opus-4 · ctx 42% · ~/proj" (what's known).
    pub fn summary(&self) -> String {
        [
            self.model.clone(),
            self.context_pct.map(|p| format!("ctx {p}%")),
            self.cost.clone(),
            self.cwd.clone(),
        ].into_iter().flatten().collect::<Vec<_>>().join(" \u{00b7} ")
    }
}

/// Split Hermes's runtime footer off an answer: where the footer paragraph
/// starts in `body`, and what it says. None when the last paragraph isn't
/// one (prose, a one-word sign-off…).
pub fn split_footer(body: &str) -> Option<(usize, Runtime)> {
    let trimmed = body.trim_end();
    let start = trimmed.rfind("\n\n")? + 2;
    let line = trimmed[start..].trim();
    if line.is_empty() || line.contains('\n') || line.chars().count() > 200 {
        return None;
    }
    let parts: Vec<&str> = line.split(" \u{00b7} ").map(str::trim).collect();
    let mut rt = Runtime::default();
    for part in &parts {
        let digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        if let Some(pct) = part.strip_suffix('%').filter(|p| digits(p) && p.len() <= 3) {
            rt.context_pct = pct.parse().ok();
        } else if part.starts_with('~') || part.starts_with('/') {
            rt.cwd = Some(part.to_string());
        } else if *part == "<1s" || part.strip_suffix('s').is_some_and(|p| digits(p) || p.split_once('m').is_some_and(|(m, s)| digits(m) && digits(s))) {
            rt.latency = Some(part.to_string());
        } else if rt.model.is_none() && (part.contains(" \u{2192} ")
            || (!part.contains(char::is_whitespace) && part.chars().any(|c| c.is_ascii_digit() || c == '-')))
        {
            rt.model = Some(part.to_string());
        } else {
            return None;
        }
    }
    // A lone model-ish word could be prose; a % or a path is a footer.
    (parts.len() >= 2 || rt.context_pct.is_some() || rt.cwd.is_some()).then_some((start, rt))
}

/// `html` without the footer's text (and the tags / breaks around it).
fn strip_footer_html(html: &str, footer: &str) -> String {
    let Some(at) = html.rfind(footer) else { return html.to_string() };
    let mut start = at;
    loop {
        let before = html[..start].trim_end();
        let cut = ["<p>", "<br>", "<br/>", "<br />"].iter().find(|t| before.ends_with(**t));
        match cut {
            Some(t) => start = before.len() - t.len(),
            None => { start = before.len(); break; }
        }
    }
    let mut end = at + footer.len();
    let rest = &html[end..];
    if rest.trim_start().starts_with("</p>") {
        end += rest.len() - rest.trim_start().len() + 4;
    }
    let (head, tail) = (&html[..start], &html[end..]);
    // A <p> left open by cutting its opening tag's partner stays balanced:
    // only matching <p>…</p> pairs are removed together.
    let head_opens = head.matches("<p>").count() + head.matches("<p ").count();
    let head_closes = head.matches("</p>").count();
    if head_opens > head_closes && !tail.contains("</p>") {
        format!("{head}</p>{tail}")
    } else {
        format!("{head}{tail}")
    }
}

/// The fluxchat agent protocol (docs/agent-protocol.md): fields an agent
/// puts in its messages under `org.fluxchat.agent`, so state doesn't have
/// to be inferred. When present they win over a harness's conventions.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct Protocol {
    pub v: u32,
    /// Event ID of the request this message is about.
    pub run: Option<String>,
    /// "working" | "waiting" | "done" | "failed".
    pub state: Option<String>,
    pub step: Option<String>,
    pub steps: Option<u32>,
    /// 0.0–1.0 when the agent knows how far along it is.
    pub progress: Option<f32>,
    pub model: Option<String>,
    /// Context window used, 0.0–1.0.
    pub context: Option<f32>,
    pub cwd: Option<String>,
    pub cost_usd: Option<f64>,
    /// A prompt: answer by reacting with a choice's key.
    pub choices: Vec<ProtocolChoice>,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct ProtocolChoice {
    pub key: String,
    pub label: String,
}

impl Protocol {
    pub fn of(msg: &MessageInfo) -> Option<Protocol> {
        serde_json::from_value(msg.agent.clone()?).ok()
    }

    pub fn run_state(&self) -> Option<RunState> {
        match self.state.as_deref()? {
            "working" => Some(RunState::Working),
            "waiting" => Some(RunState::NeedsYou),
            "done" => Some(RunState::Done),
            "failed" => Some(RunState::Failed),
            _ => None,
        }
    }

    fn runtime(&self) -> Option<Runtime> {
        let rt = Runtime {
            model: self.model.clone(),
            context_pct: self.context.map(|c| (c.clamp(0.0, 1.0) * 100.0).round() as u8),
            cwd: self.cwd.clone(),
            latency: None,
            cost: self.cost_usd.map(|c| format!("${c:.2}")),
        };
        (rt != Runtime::default()).then_some(rt)
    }
}

/// A run: where it stands, when the request was sent, and the agent's
/// latest step (from its progress message, when the view is loaded).
#[derive(Debug, Clone, PartialEq)]
pub struct RunInfo {
    pub state: RunState,
    /// The request's timestamp (ms).
    pub since: u64,
    pub step: Option<String>,
    pub steps: usize,
    /// How far along, when the agent says (agent protocol).
    pub progress: Option<f32>,
}

impl RunInfo {
    fn new(state: RunState, since: u64) -> Self {
        RunInfo { state, since, step: None, steps: 0, progress: None }
    }

    /// "45s", "3m", "1h 20m" since the request.
    pub fn elapsed(&self) -> String {
        let secs = now_ms().saturating_sub(self.since) / 1000;
        match secs {
            0..=59 => format!("{secs}s"),
            60..=3599 => format!("{}m", secs / 60),
            _ => format!("{}h {}m", secs / 3600, secs % 3600 / 60),
        }
    }
}

/// Run states worked out from the loaded timelines (`refresh_agent_state`).
#[derive(Debug, Default)]
pub struct AgentRuns {
    /// By thread key (a request answered in a thread) or room key (one that
    /// wasn't threaded).
    states: HashMap<String, RunInfo>,
    /// Prompts already seen, by event ID (each is announced once).
    announced: std::collections::HashSet<String>,
    /// Messages a desktop notification was shown for (so a prompt that
    /// already notified as a message isn't announced twice).
    pub(crate) notified: std::collections::HashSet<String>,
    /// Rooms with a run waiting on you (from `states`; the room list asks
    /// for every row on every render).
    needs_you: std::collections::HashSet<String>,
    /// The latest runtime footer seen per room (timestamp, info).
    runtime: HashMap<String, (u64, Runtime)>,
    /// Runs that just finished (were working / waiting, now done / failed),
    /// for `announce_finished`.
    finished: Vec<String>,
    /// Runs you asked to hear about when they finish (bell in the run bar).
    pub(crate) watched: std::collections::HashSet<String>,
    /// Runs that just started, for "watch runs" windows to pick up.
    started: Vec<String>,
    /// Rooms an agent is typing in, since when (ms): "working" for
    /// harnesses that signal work that way.
    typing_since: HashMap<String, u64>,
}

impl AgentRuns {
    fn set(&mut self, key: String, run: Option<RunInfo>) {
        match run {
            Some(r) => { self.states.insert(key, r); }
            None => { self.states.remove(&key); }
        }
    }

    fn recount_needs_you(&mut self) {
        self.needs_you = self.states.iter()
            .filter(|(_, r)| r.state == RunState::NeedsYou)
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

    /// The harness running `agent` (its profile; Hermes unless chosen).
    pub(crate) fn harness_of(&self, agent: &str) -> std::sync::Arc<Harness> {
        crate::harness::get(self.prefs.agent_harness.get(agent).map(String::as_str).unwrap_or(crate::harness::DEFAULT_HARNESS))
    }

    /// The agent in a view's room, and its harness.
    pub(crate) fn agent_harness(&self, key: &str) -> Option<(String, std::sync::Arc<Harness>)> {
        let agent = self.agent_in(key)?.to_string();
        let h = self.harness_of(&agent);
        Some((agent, h))
    }

    /// A command as the agent takes it here: with its mention in front for
    /// harnesses that want that outside DMs (OpenClaw).
    fn addressed(&self, key: &str, agent: &str, h: &Harness, command: String) -> String {
        let room = real_room_id(key);
        let dm = self.rooms.iter().find(|r| r.room_id == room).is_some_and(|r| r.is_direct);
        if h.mention_commands && !dm { format!("{agent} {command}") } else { command }
    }

    /// Track agents typing (a working signal for some harnesses).
    pub(crate) fn note_agent_typing(&mut self, room: &str, user_ids: &[String]) {
        if user_ids.iter().any(|u| self.prefs.agents.contains(u)) {
            self.agent_runs.typing_since.entry(room.to_string()).or_insert_with(now_ms);
        } else {
            self.agent_runs.typing_since.remove(room);
        }
    }

    /// The run going on behind a view, if any: its own working / waiting
    /// state, else — for harnesses that work that way — the agent typing.
    pub(crate) fn live_run(&self, key: &str) -> Option<RunInfo> {
        let own = self.run_info(key).filter(|r| matches!(r.state, RunState::Working | RunState::NeedsYou)).cloned();
        own.or_else(|| {
            let (_, h) = self.agent_harness(key)?;
            let since = *self.agent_runs.typing_since.get(real_room_id(key)).filter(|_| h.typing_means_working)?;
            Some(RunInfo::new(RunState::Working, since))
        })
    }

    /// Where the run behind a view stands: a thread key for a threaded
    /// request, a room key for the room's latest unthreaded one.
    pub(crate) fn run_state(&self, key: &str) -> Option<RunState> {
        self.agent_runs.states.get(key).map(|r| r.state)
    }

    pub(crate) fn run_info(&self, key: &str) -> Option<&RunInfo> {
        self.agent_runs.states.get(key)
    }

    /// The run a room's header speaks for: the view's own (a thread), else
    /// the room's liveliest — one waiting on you, else the newest working —
    /// else the agent typing (harnesses that signal work that way).
    pub(crate) fn header_run(&self, key: &str) -> Option<RunInfo> {
        if key.starts_with("thread::") {
            return self.live_run(key);
        }
        let prefix = format!("thread::{key}::");
        self.agent_runs.states.iter()
            .filter(|(k, _)| *k == key || k.starts_with(&prefix))
            .map(|(_, r)| r)
            .filter(|r| matches!(r.state, RunState::Working | RunState::NeedsYou))
            .max_by_key(|r| (r.state == RunState::NeedsYou, r.since))
            .cloned()
            .or_else(|| self.live_run(key))
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
        // Protocol prompts say so; they're live while their state is "waiting".
        if let Some(p) = Protocol::of(msg).filter(|p| !p.choices.is_empty()) {
            return if p.run_state() == Some(RunState::NeedsYou) {
                p.choices.iter().map(|c| (c.key.clone(), c.label.clone())).collect()
            } else {
                Vec::new()
            };
        }
        prompt_choices(msg, now_ms(), &self.harness_of(&msg.sender).approvals)
    }

    /// What to send for typed `text` in view `key` when an agent is there:
    /// agent commands as `!name`, and while it's working, the busy mode's
    /// prefix. None = no agent / a fluxchat command / plain text as typed.
    pub(crate) fn agent_outgoing(&self, key: &str, text: &str) -> Option<String> {
        let (agent, h) = self.agent_harness(key)?;
        if text.starts_with('/') {
            if text.starts_with("//") {
                return None;
            }
            return agent_command(&h, text).map(|c| self.addressed(key, &agent, &h, c));
        }
        if text.starts_with('!') || self.live_run(key).map(|r| r.state) != Some(RunState::Working) {
            return None;
        }
        self.prefs.agent_busy_mode.prefix(&h).map(|p| self.addressed(key, &agent, &h, format!("{p}{text}")))
    }

    /// A harness command (by name) as it goes out to view `key`.
    pub(crate) fn agent_command_text(&self, key: &str, name: &str) -> Option<String> {
        let (agent, h) = self.agent_harness(key)?;
        (!name.is_empty()).then(|| self.addressed(key, &agent, &h, h.command(name)))
    }

    /// Take Hermes's runtime footer off the agent's answers in `msgs` (it's
    /// shown in the header chip and status bar instead) and remember the
    /// newest per room. Done on every update: the watcher resends whole lists.
    pub(crate) fn take_agent_footers(&mut self, key: &str, msgs: &mut [MessageInfo]) {
        // Before `msgs` are stored: a new view's agent may only be in them.
        let incoming = msgs.iter().find_map(|m| self.prefs.agents.iter().find(|a| **a == m.sender)).cloned();
        let Some((agent, h)) = self.agent_harness(key)
            .or_else(|| incoming.map(|a| { let h = self.harness_of(&a); (a, h) }))
        else { return };
        let room = real_room_id(key).to_string();
        // Runtime the agent reports through the protocol.
        for m in msgs.iter().filter(|m| m.sender == agent) {
            if let Some(rt) = Protocol::of(m).and_then(|p| p.runtime()) {
                let newer = self.agent_runs.runtime.get(&room).is_none_or(|(ts, _)| m.timestamp >= *ts);
                if newer {
                    self.agent_runs.runtime.insert(room.clone(), (m.timestamp, rt));
                }
            }
        }
        if !h.runtime_footer {
            return;
        }
        for m in msgs.iter_mut().filter(|m| m.sender == agent) {
            let Some((start, rt)) = split_footer(&m.body) else { continue };
            let footer = m.body[start..].trim().to_string();
            m.body.truncate(start);
            m.body.truncate(m.body.trim_end().len());
            if let Some(html) = m.formatted_body.as_mut() {
                *html = strip_footer_html(html, &footer);
            }
            let newer = self.agent_runs.runtime.get(&room).is_none_or(|(ts, _)| m.timestamp >= *ts);
            if newer {
                self.agent_runs.runtime.insert(room.clone(), (m.timestamp, rt));
            }
        }
    }

    /// Runs as the agent describes them (agent protocol) in a view's
    /// messages: per run, its latest status. Threaded requests go under
    /// their thread key; in a thread view, the thread's own key.
    fn protocol_runs(&self, key: &str, msgs: &[MessageInfo], agent: &str) -> Vec<(String, RunInfo)> {
        let mut latest: HashMap<String, (&MessageInfo, Protocol)> = HashMap::new();
        for m in msgs.iter().filter(|m| m.sender == agent) {
            let Some(p) = Protocol::of(m).filter(|p| p.run_state().is_some()) else { continue };
            let run_id = p.run.clone().unwrap_or_default();
            let newer = latest.get(&run_id).is_none_or(|(old, _)| m.timestamp >= old.timestamp);
            if newer {
                latest.insert(run_id, (m, p));
            }
        }
        let in_thread = parse_thread_key(key).is_some();
        let room = real_room_id(key);
        latest.into_iter().filter_map(|(run_id, (m, p))| {
            let request = msgs.iter().find(|r| r.event_id == run_id);
            let target = if in_thread {
                key.to_string()
            } else if request.is_some_and(|r| r.thread_count.unwrap_or(0) > 0 || r.thread_latest.is_some()) {
                thread_key(room, &run_id)
            } else {
                key.to_string()
            };
            Some((target, RunInfo {
                state: p.run_state()?,
                since: request.map(|r| r.timestamp).unwrap_or(m.timestamp),
                step: p.step.clone(),
                steps: p.steps.map(|n| n as usize).unwrap_or(0),
                progress: p.progress.map(|f| f.clamp(0.0, 1.0)),
            }))
        }).collect()
    }

    /// The agent's latest runtime info in a view's room.
    pub(crate) fn agent_runtime(&self, key: &str) -> Option<&Runtime> {
        self.agent_runs.runtime.get(real_room_id(key)).map(|(_, r)| r)
    }

    /// Recompute the run states a view's messages give (after they change).
    /// Returns the event IDs of prompts that just appeared, to announce.
    pub(crate) fn refresh_agent_state(&mut self, key: &str) -> Vec<String> {
        let Some((agent, h)) = self.agent_harness(key) else { return Vec::new() };
        let Some(msgs) = self.messages_by_room.get(key) else { return Vec::new() };
        let me = self.user_id.clone();
        let now = now_ms();
        let protocol_prompt = |m: &MessageInfo| Protocol::of(m)
            .is_some_and(|p| !p.choices.is_empty() && p.run_state() == Some(RunState::NeedsYou));
        let prompts: Vec<&MessageInfo> = msgs.iter()
            .filter(|m| m.sender == agent && (protocol_prompt(m) || !prompt_choices(m, now, &h.approvals).is_empty()))
            .collect();
        let fresh: Vec<String> = prompts.iter()
            .filter(|m| !self.agent_runs.announced.contains(&m.event_id))
            .map(|m| m.event_id.clone())
            .collect();
        let needs_you = !prompts.is_empty();
        // The agent's latest step after `since`, from its progress messages.
        let step_after = |since: u64, run: &mut RunInfo| {
            if !h.progress_steps {
                return;
            }
            if let Some(steps) = msgs.iter().rev()
                .filter(|m| m.sender == agent && m.timestamp >= since)
                .find_map(|m| progress_steps(&m.body))
            {
                run.step = steps.last().map(|s| s.to_string());
                run.steps = steps.len();
            }
        };
        let mut updates: Vec<(String, Option<RunInfo>)> = Vec::new();
        if parse_thread_key(key).is_some() {
            // A thread: its latest request (the root or a reply of yours).
            let latest = msgs.iter().rev().filter(|m| m.sender == me)
                .find_map(|m| request_state(&h, m, &agent).map(|s| RunInfo::new(s, m.timestamp)));
            let run = match (latest, needs_you) {
                (Some(mut r), true) => { r.state = RunState::NeedsYou; Some(r) }
                (None, true) => Some(RunInfo::new(RunState::NeedsYou, now)),
                (r, false) => r,
            };
            updates.push((key.to_string(), run.map(|mut r| { step_after(r.since, &mut r); r })));
        } else {
            // Threaded requests, unless their thread is open (its own view
            // knows better, approvals included).
            // Threaded = what the "N replies" row goes by: the SDK's summary,
            // a latest reply, or loaded replies.
            let loaded_replies = self.thread_counts.get(key);
            let mut latest_unthreaded: Option<RunInfo> = None;
            for m in msgs.iter().filter(|m| m.sender == me && m.thread_root.is_none()) {
                let Some(state) = request_state(&h, m, &agent) else { continue };
                let threaded = m.thread_count.unwrap_or(0) > 0
                    || m.thread_latest.is_some()
                    || loaded_replies.and_then(|c| c.get(&m.event_id)).is_some_and(|n| *n > 0);
                let tk = thread_key(key, &m.event_id);
                if !self.messages_by_room.contains_key(&tk) {
                    updates.push((tk, Some(RunInfo::new(state, m.timestamp))));
                }
                if !threaded {
                    latest_unthreaded = Some(RunInfo::new(state, m.timestamp));
                }
            }
            let run = match (latest_unthreaded, needs_you) {
                (Some(mut r), true) => { r.state = RunState::NeedsYou; Some(r) }
                (None, true) => Some(RunInfo::new(RunState::NeedsYou, now)),
                (r, false) => r,
            };
            updates.push((key.to_string(), run.map(|mut r| { step_after(r.since, &mut r); r })));
        }
        // Agent protocol: what the agent says about its runs wins.
        for (k, run) in self.protocol_runs(key, msgs, &agent) {
            updates.retain(|(u, _)| *u != k);
            updates.push((k, Some(run)));
        }
        for (k, run) in updates {
            let was_live = self.agent_runs.states.get(&k)
                .is_some_and(|r| matches!(r.state, RunState::Working | RunState::NeedsYou));
            let over = run.as_ref().is_some_and(|r| matches!(r.state, RunState::Done | RunState::Failed));
            if was_live && over {
                self.agent_runs.finished.push(k.clone());
            }
            let starting = !was_live && run.as_ref().is_some_and(|r| {
                matches!(r.state, RunState::Working | RunState::NeedsYou) && now.saturating_sub(r.since) < FOLLOW_WINDOW_MS
            });
            if starting {
                self.agent_runs.started.push(k.clone());
            }
            self.agent_runs.set(k, run);
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

    /// A desktop notification for each run that just finished: ones you
    /// asked about (bell), and — with the setting on — any that ran at least
    /// `DONE_NOTIFY_MIN_MS`. Not while it's on screen in a focused window.
    pub(crate) fn announce_finished(&mut self) {
        let finished = std::mem::take(&mut self.agent_runs.finished);
        for key in finished {
            let watched = self.agent_runs.watched.remove(&key);
            let Some(run) = self.agent_runs.states.get(&key).cloned() else { continue };
            let long = now_ms().saturating_sub(run.since) >= DONE_NOTIFY_MIN_MS;
            let looking = self.app_focused && (self.visible(&key) || (!key.starts_with("thread::") && self.room_visible(&key)));
            if !self.notifications_enabled || looking || !(watched || (self.prefs.notify_agent_done && long)) {
                continue;
            }
            let room = real_room_id(&key).to_string();
            let agent = self.agent_in(&key).map(|a| self.agent_name(&key, a)).unwrap_or_else(|| "Agent".into());
            let room_name = self.rooms.iter().find(|r| r.room_id == room).map(|r| r.name.clone()).unwrap_or_default();
            let what = if run.state == RunState::Failed { "failed" } else { "finished" };
            let body = if key.starts_with("thread::") { self.thread_title(&key) } else { "Your request".into() };
            crate::update::show_notification(
                format!("{agent} {what} ({}) - {room_name}", run.elapsed()),
                body,
                self.notify_sound,
                Some((self.timeline_tx.clone(), room)),
            );
        }
    }

    /// Find the rooms the agents are in and keep their timelines loaded, so
    /// runs everywhere are tracked (Runs dialog, notifications, room dots).
    pub(crate) fn watch_agent_rooms(&self) -> iced::Task<Message> {
        if self.prefs.agents.is_empty() {
            return iced::Task::none();
        }
        let core = self.core.clone();
        let agents = self.prefs.agents.clone();
        iced::Task::perform(
            async move {
                let client = core.get_client().await.ok()?;
                Some(fluxchat_core::rooms::rooms_with_members(&client, &agents).await)
            },
            |rooms| rooms.map(Message::AgentRoomsFound).unwrap_or(Message::Noop),
        )
    }

    /// Load the timelines of agent rooms not loaded yet (in the background:
    /// no media, no read receipts — that's only for views on screen).
    pub(crate) fn load_agent_rooms(&mut self, rooms: Vec<String>) -> iced::Task<Message> {
        let missing: Vec<String> = rooms.iter().filter(|r| !self.messages_by_room.contains_key(*r)).cloned().collect();
        self.agent_rooms = rooms;
        let core = self.core.clone();
        iced::Task::perform(
            async move {
                use futures_util::FutureExt;
                let Ok(client) = core.get_client().await else { return };
                for room in missing {
                    if !core.timelines.request_snapshot(&room).await {
                        let opened = fluxchat_core::timeline::get_or_create_for_key(&client, &core.timelines, &room).boxed().await;
                        if let Err(e) = opened {
                            tracing::warn!("Couldn't follow agent room {room}: {e}");
                        }
                    }
                }
            },
            |_| Message::Noop,
        )
    }

    /// Every known run, liveliest first: waiting on you, working, then
    /// recently finished (newest first).
    pub(crate) fn all_runs(&self) -> Vec<(String, RunInfo)> {
        let now = now_ms();
        let mut runs: Vec<(String, RunInfo)> = self.agent_runs.states.iter()
            .filter(|(_, r)| matches!(r.state, RunState::Working | RunState::NeedsYou) || now.saturating_sub(r.since) < RECENT_RUN_MS)
            .map(|(k, r)| (k.clone(), r.clone()))
            .collect();
        runs.sort_by_key(|(_, r)| {
            let rank = match r.state {
                RunState::NeedsYou => 0,
                RunState::Working => 1,
                _ => 2,
            };
            (rank, std::cmp::Reverse(r.since))
        });
        runs
    }

    /// A new window with a pane per active run; runs that start later get
    /// panes in it too (`follow_started_runs`), up to `RUN_WINDOW_PANES`.
    pub(crate) fn open_runs_window(&mut self) -> iced::Task<Message> {
        let live: Vec<String> = self.all_runs().into_iter()
            .filter(|(_, r)| matches!(r.state, RunState::Working | RunState::NeedsYou))
            .map(|(k, _)| k)
            .take(RUN_WINDOW_PANES)
            .collect();
        let Some(first) = live.first().cloned() else {
            self.help_text = Some("No agent runs going right now. A watch window opens with the next one.".into());
            let mut w = crate::panes::Window::new(None);
            w.follow_runs = true;
            self.windows.push(w);
            self.active_window = self.windows.len() - 1;
            self.layout_dirty = true;
            let p = self.window().focus;
            return self.focus_pane(p);
        };
        let mut w = crate::panes::Window::new(Some(first));
        w.follow_runs = true;
        self.windows.push(w);
        self.active_window = self.windows.len() - 1;
        self.layout_dirty = true;
        let mut tasks = Vec::new();
        for key in live.iter().skip(1) {
            add_run_pane(self.windows.last_mut().expect("just pushed"), key.clone());
            tasks.push(self.ensure_pane_view(key.clone()));
        }
        let p = self.window().focus;
        tasks.push(self.focus_pane(p));
        iced::Task::batch(tasks)
    }

    /// Give runs that just started a pane in each "watch runs" window.
    pub(crate) fn follow_started_runs(&mut self) -> iced::Task<Message> {
        let started = std::mem::take(&mut self.agent_runs.started);
        let mut tasks = Vec::new();
        for key in started {
            for i in 0..self.windows.len() {
                let w = &mut self.windows[i];
                if !w.follow_runs || w.pane_with(&key).is_some() || w.grid.len() >= RUN_WINDOW_PANES {
                    continue;
                }
                // An empty window's lone pane takes the first run.
                let empty = w.grid.iter().find(|(_, s)| s.key.is_none()).map(|(p, _)| *p);
                match empty {
                    Some(p) => {
                        if let Some(slot) = w.grid.get_mut(p) {
                            slot.key = Some(key.clone());
                        }
                    }
                    None => add_run_pane(w, key.clone()),
                }
                self.layout_dirty = true;
                tasks.push(self.ensure_pane_view(key.clone()));
                if i == self.active_window && self.window().focused_key() == Some(key.as_str()) {
                    tasks.push(self.update(Message::SelectRoom(key.clone())));
                }
            }
        }
        iced::Task::batch(tasks)
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
        let (agent, h) = self.agent_harness(key)?;
        let agent = agent.as_str();
        let now = now_ms();
        new.iter().rev()
            .filter(|m| m.sender == self.user_id && m.thread_root.is_none())
            .filter(|m| now.saturating_sub(m.timestamp) < FOLLOW_WINDOW_MS)
            .filter(|m| m.thread_count.unwrap_or(0) > 0)
            .filter(|m| request_state(&h, m, agent).is_some()
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
        let (agent, h) = self.agent_harness(key)?;
        let run = self.live_run(key)?;
        let run = &run;
        let state = run.state;
        let name = self.agent_name(key, &agent);
        let label = if state == RunState::Working {
            format!("{name} is working \u{00b7} {}", run.elapsed())
        } else {
            format!("{name} is waiting for your answer")
        };
        let mut bar = row![
            state_dot(state, 8.0),
            text(label).font(self.sidebar_font()).size(12).color(crate::theme::colors().text_normal)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        // What it's on now ("📖 Reading notes.md"), from its progress message.
        bar = bar.push(container(self.view_step(run)).width(Fill).clip(true));
        // Send / Steer / Queue, as far as the harness has them.
        let modes = BusyMode::available(&h);
        if state == RunState::Working && !compact && modes.len() > 1 {
            let mode = if modes.contains(&self.prefs.agent_busy_mode) { self.prefs.agent_busy_mode } else { BusyMode::Send };
            bar = bar.push(text("Next message").font(self.sidebar_font()).size(11).color(crate::theme::colors().text_dim));
            let switch = ui::segmented(modes.iter()
                .map(|m| (m.label().to_string(), Some(Message::SetBusyMode(*m)), *m == mode))
                .collect());
            bar = bar.push(ui::with_tooltip(container(switch).width(BUSY_SWITCH_WIDTH * modes.len() as f32 / 3.0), mode.hint()));
        }
        // Tell me when it's done (lit while asked).
        let watched = self.agent_runs.watched.contains(key);
        bar = bar.push(ui::with_tooltip(
            button(ui::icon(crate::icons::SVG_BELL, 13.0, if watched { crate::theme::colors().accent } else { crate::theme::colors().text_dim }))
                .on_press(Message::WatchRun(key.to_string()))
                .padding([4, 6])
                .style(ui::button_style(if watched { Tone::Primary } else { Tone::Ghost })),
            if watched { "You'll be notified when it's done (click to cancel)" } else { "Tell me when it's done" },
        ));
        // Stop, when the harness has a command for it.
        if let Some(stop) = self.agent_command_text(key, &h.stop) {
            bar = bar.push(ui::with_tooltip(
                ui::icon_btn(crate::icons::SVG_STOP, "Stop", Tone::Danger, Some(Message::AgentSend(key.to_string(), stop)))
                    .padding([3, 10]),
                "Interrupt the run",
            ));
        }
        let tint = state.color();
        let bar = iced::widget::column![bar]
            .push((state == RunState::Working).then(|| crate::progress::bar(2.0, tint, run.progress)))
            .spacing(4);
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

    /// A run in a list (task board): state, how long it ran / has been
    /// running, and its latest step while it's going.
    pub(crate) fn view_run_line<'a>(&self, run: &RunInfo, thread: &fluxchat_core::ThreadPreview) -> Element<'a, Message> {
        let took = match run.state {
            RunState::Working | RunState::NeedsYou => run.elapsed(),
            // Finished: from the request to the thread's last activity.
            _ => {
                let secs = (thread.last_activity * 1000).saturating_sub(run.since) / 1000;
                RunInfo { since: now_ms().saturating_sub(secs * 1000), ..run.clone() }.elapsed()
            }
        };
        let mut line = row![
            state_dot(run.state, 7.0),
            text(format!("{} \u{00b7} {took}", run.state.label())).font(self.sidebar_font()).size(11).color(run.state.color())
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(5)
        .align_y(Alignment::Center);
        if matches!(run.state, RunState::Working) {
            if let Some(step) = &run.step {
                line = line.push(text(step.clone()).font(self.sidebar_font()).size(11).color(crate::theme::colors().text_muted)
                    .wrapping(iced::widget::text::Wrapping::None));
            }
        }
        container(line).clip(true).into()
    }

    /// The run's current step and how many there have been, dimmed.
    fn view_step(&self, run: &RunInfo) -> Element<'_, Message> {
        match &run.step {
            Some(step) => row![
                text(step.clone()).font(self.sidebar_font()).size(12).color(crate::theme::colors().text_muted)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .push((run.steps > 1).then(|| text(format!("step {}", run.steps)).font(self.sidebar_font()).size(11)
                .color(crate::theme::colors().text_dim).wrapping(iced::widget::text::Wrapping::None)))
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
            None => Space::new().into(),
        }
    }

    /// The agent chip in a room's header: who, and what it's up to (idle /
    /// working with a progress bar / waiting on you). Opens the agent menu.
    pub(crate) fn view_agent_marker(&self, key: &str) -> Option<Element<'_, Message>> {
        let agent = self.agent_in(key)?;
        let name = self.agent_name(key, agent);
        let run = self.header_run(key);
        let run = run.as_ref();
        let colors = crate::theme::colors();
        let (status, color) = match run {
            Some(r) if r.state == RunState::NeedsYou => ("waiting on you".to_string(), r.state.color()),
            Some(r) => (format!("working \u{00b7} {}", r.elapsed()), r.state.color()),
            None => ("idle".to_string(), colors.text_dim),
        };
        let top = row![
            ui::icon(crate::icons::SVG_BOT, 14.0, if run.is_some() { color } else { colors.text_dim }),
            text(name).font(self.sidebar_font()).size(12).color(colors.text_normal)
                .wrapping(iced::widget::text::Wrapping::None),
            text(status).font(self.sidebar_font()).size(11).color(color)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        // Model and context fill, when its footer told us.
        .push(self.agent_runtime(key).map(|rt| {
            let short = [rt.model.clone(), rt.context_pct.map(|p| format!("{p}%"))]
                .into_iter().flatten().collect::<Vec<_>>().join(" ");
            text(short).font(self.sidebar_font()).size(11).color(colors.text_dim)
                .wrapping(iced::widget::text::Wrapping::None)
        }))
        .spacing(6)
        .align_y(Alignment::Center);
        let working = run.is_some_and(|r| r.state == RunState::Working);
        let chip = iced::widget::column![top]
            .push(working.then(|| crate::progress::bar(2.0, color, run.and_then(|r| r.progress))))
            .spacing(3)
            .width(iced::Length::Shrink);
        let tip = run.and_then(|r| r.step.clone()).unwrap_or_else(|| "Agent commands".into());
        Some(crate::ui::styled_tooltip(
            button(chip)
                .on_press(Message::OpenContextMenu(crate::state::ContextMenu::Agent { key: key.to_string() }))
                .padding([3, 8])
                .style(move |_, status| button::Style {
                    background: Some(if matches!(status, button::Status::Hovered) {
                        crate::theme::with_alpha(color, 0.16)
                    } else {
                        crate::theme::with_alpha(color, 0.08)
                    }.into()),
                    border: iced::Border { color: crate::theme::with_alpha(color, 0.3), width: 1.0, radius: 10.0.into() },
                    ..Default::default()
                }),
            text(tip).font(self.sidebar_font()).size(11),
            iced::widget::tooltip::Position::Bottom,
        ))
    }

    /// An agent's step log: collapsed to "N steps · latest", or every step.
    pub(crate) fn view_progress_log<'a>(&self, msg: &MessageInfo, steps: &[&str], expanded: bool) -> Element<'a, Message> {
        let colors = crate::theme::colors();
        let fs = self.msg_font_size;
        let line = |s: &str, color: iced::Color| -> Element<'a, Message> {
            text(s.replace('`', "")).font(self.msg_font()).size(fs - 1.0).color(color)
                .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                .into()
        };
        let toggle = button(
            text(if expanded { "Hide steps".to_string() } else { format!("{} steps", steps.len()) })
                .font(self.sidebar_font()).size(11).color(colors.accent),
        )
        .on_press(Message::ToggleAgentFold(msg.event_id.clone()))
        .padding([1, 6])
        .style(move |_, status| button::Style {
            background: Some(crate::theme::with_alpha(colors.accent, if matches!(status, button::Status::Hovered) { 0.16 } else { 0.08 }).into()),
            border: iced::Border { radius: 8.0.into(), ..Default::default() },
            ..Default::default()
        });
        if expanded {
            let mut col = iced::widget::column![].spacing(2);
            let last = steps.len().saturating_sub(1);
            for (i, s) in steps.iter().enumerate() {
                col = col.push(line(s, if i == last { colors.text_normal } else { colors.text_muted }));
            }
            iced::widget::column![col, toggle].spacing(4).width(Fill).into()
        } else {
            row![toggle, container(line(steps.last().copied().unwrap_or(""), colors.text_normal)).width(Fill).clip(true)]
                .spacing(8)
                .align_y(Alignment::Center)
                .width(Fill)
                .into()
        }
    }

    /// The Runs dialog: every agent run across rooms. Click one to open it;
    /// Watch in a window tiles the active ones and follows new ones.
    pub(crate) fn view_runs_dialog(&self) -> Element<'_, Message> {
        let colors = crate::theme::colors();
        let runs = self.all_runs();
        let working = runs.iter().filter(|(_, r)| r.state == RunState::Working).count();
        let waiting = runs.iter().filter(|(_, r)| r.state == RunState::NeedsYou).count();
        let subtitle = if self.prefs.agents.is_empty() {
            "Mark an agent on its profile (AI agent) to follow its runs here".to_string()
        } else {
            format!("{working} working \u{00b7} {waiting} waiting on you \u{00b7} across {} rooms", self.agent_rooms.len().max(1))
        };
        let mut list = iced::widget::column![].spacing(2);
        if runs.is_empty() {
            list = list.push(ui::empty_state(crate::icons::SVG_BOT, "No runs yet", Some("Runs show here as your agents work.".into())));
        }
        for (key, run) in &runs {
            let room = real_room_id(key);
            let room_name = self.rooms.iter().find(|r| r.room_id == room)
                .map(|r| if r.is_direct { r.name.clone() } else { format!("#{}", r.name) })
                .unwrap_or_else(|| room.to_string());
            let title = if key.starts_with("thread::") { self.thread_title(key) } else { "Request in the room".into() };
            let mut detail = row![
                state_dot(run.state, 7.0),
                text(format!("{} \u{00b7} {}", run.state.label(), run.elapsed())).font(self.sidebar_font()).size(11).color(run.state.color())
                    .wrapping(iced::widget::text::Wrapping::None),
                text(room_name).font(self.sidebar_font()).size(11).color(colors.text_dim)
                    .wrapping(iced::widget::text::Wrapping::None),
            ].spacing(6).align_y(Alignment::Center);
            if run.state == RunState::Working {
                if let Some(step) = &run.step {
                    detail = detail.push(text(step.clone()).font(self.sidebar_font()).size(11).color(colors.text_muted)
                        .wrapping(iced::widget::text::Wrapping::None));
                }
            }
            list = list.push(
                button(iced::widget::column![
                    text(crate::view::ellipsize(&title, 80)).font(self.sidebar_font()).size(13).color(colors.text_bright)
                        .wrapping(iced::widget::text::Wrapping::None),
                    container(detail).clip(true),
                ].spacing(3))
                .on_press(Message::Batch(vec![Message::CloseModal, Message::OpenAt(key.clone(), crate::panes::Placement::Auto)]))
                .padding([7, 10])
                .width(Fill)
                .style(ui::row_style(false)),
            );
        }
        let live = working + waiting > 0;
        iced::widget::column![
            ui::header("Agent runs", Some(subtitle)),
            container(iced::widget::scrollable(list).height(iced::Length::Shrink))
                .max_height(460.0),
            ui::footer(vec![
                ui::icon_btn(crate::icons::SVG_LAYERS, if live { "Watch in a window" } else { "Watch for new runs" }, Tone::Primary,
                    Some(Message::WatchRunsWindow)).into(),
            ]),
        ]
        .spacing(ui::GAP)
        .width(iced::Length::Fixed(580.0))
        .into()
    }

    /// Live runs across rooms for the status bar: (working, waiting on you).
    pub(crate) fn live_run_counts(&self) -> (usize, usize) {
        self.agent_runs.states.values().fold((0, 0), |(w, n), r| match r.state {
            RunState::Working => (w + 1, n),
            RunState::NeedsYou => (w, n + 1),
            _ => (w, n),
        })
    }

    /// Which harness runs `agent`: a dropdown of the known profiles (built-in
    /// and the TOML files in the data folder), with what the chosen one is.
    pub(crate) fn view_harness_picker<'a>(&self, agent: &str) -> Element<'a, Message> {
        let current = self.harness_of(agent);
        let names: Vec<String> = crate::harness::all().iter().map(|h| h.name.clone()).collect();
        let user = agent.to_string();
        let picker = iced::widget::pick_list(names, Some(current.name.clone()), move |name: String| {
            let id = crate::harness::all().into_iter().find(|h| h.name == name).map(|h| h.id.clone()).unwrap_or_default();
            Message::SetAgentHarness(user.clone(), id)
        })
        .font(crate::view::MONO)
        .text_size(13)
        .padding([6, 10])
        .width(Fill)
        .style(ui::pick_list_style);
        iced::widget::column![
            row![
                text("Harness").font(crate::view::MONO).size(13).color(crate::theme::colors().text_normal),
                container(picker).width(Fill),
            ].spacing(10).align_y(Alignment::Center),
            ui::hint(if current.about.is_empty() { "How fluxchat reads this agent's runs, commands and approvals".to_string() } else { current.about.clone() }),
        ]
        .spacing(4)
        .into()
    }

    /// The agent menu (header chip): quick commands for this view's session.
    pub(crate) fn agent_menu_items(&self, key: &str) -> Vec<(String, &'static [u8], Message)> {
        let Some((agent, h)) = self.agent_harness(key) else { return Vec::new() };
        let send = |name: &str| self.agent_command_text(key, name).map(|c| Message::AgentSend(key.to_string(), c));
        let mut items = Vec::new();
        if self.header_run(key).is_some_and(|r| r.state == RunState::Working) {
            if let Some(stop) = send(&h.stop) {
                items.push(("Stop the run".to_string(), crate::icons::SVG_STOP, stop));
            }
        }
        if self.live_run(key).is_some() && !self.agent_runs.watched.contains(key) {
            items.push(("Tell me when it's done".to_string(), crate::icons::SVG_BELL, Message::WatchRun(key.to_string())));
        }
        // The harness's quick commands.
        for item in &h.menu {
            let icon = match item.command.as_str() {
                "status" => crate::icons::SVG_INFO,
                "usage" | "cost" => crate::icons::SVG_SLIDERS,
                "context" => crate::icons::SVG_LAYERS,
                "model" => crate::icons::SVG_BOT,
                "new" | "reset" | "clear" => crate::icons::SVG_PLUS,
                _ => crate::icons::SVG_CHEVRON_RIGHT,
            };
            if let Some(msg) = send(&item.command) {
                items.push((item.label.clone(), icon, msg));
            }
        }
        items.push(("All agent runs".to_string(), crate::icons::SVG_LAYERS, Message::OpenModal(crate::state::Modal::Runs)));
        items.push((format!("Agent profile ({})", h.name), crate::icons::SVG_PERSON,
            Message::OpenModal(crate::state::Modal::Profile { user_id: agent })));
        items
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

    /// Hermes's reactions on a request: working on it / done / failed.
    const WORKING: &str = "\u{1f440}";
    const DONE: &str = "\u{2705}";
    const FAILED: &str = "\u{274c}";

    fn hermes() -> std::sync::Arc<Harness> {
        crate::harness::get("hermes")
    }

    fn react(emoji: &str, users: &[&str]) -> ReactionInfo {
        ReactionInfo { emoji: emoji.into(), count: users.len() as u32, users: users.iter().map(|u| u.to_string()).collect() }
    }

    #[test]
    fn a_requests_reactions_give_its_state() {
        let mut m = MessageInfo { sender: "@me:x".into(), ..Default::default() };
        assert_eq!(request_state(&hermes(), &m, "@bot:x"), None);
        m.reactions = vec![react(WORKING, &["@bot:x"])];
        assert_eq!(request_state(&hermes(), &m, "@bot:x"), Some(RunState::Working));
        // Someone else's 👀 is just a reaction.
        assert_eq!(request_state(&hermes(), &m, "@other:x"), None);
        m.reactions.push(react(DONE, &["@bot:x"]));
        assert_eq!(request_state(&hermes(), &m, "@bot:x"), Some(RunState::Done), "✅ wins over the 👀 not yet taken back");
        m.reactions = vec![react(FAILED, &["@bot:x"])];
        assert_eq!(request_state(&hermes(), &m, "@bot:x"), Some(RunState::Failed));
    }

    #[test]
    fn a_prompt_is_the_senders_own_reactions_and_ends_when_they_go() {
        let now = 10_000_000;
        let body = "⚠️ Dangerous command\n`rm -rf build`\n\nReact:\n✅ once\n♾️ always\n❌ deny";
        let mut m = MessageInfo { sender: "@bot:x".into(), body: body.into(), timestamp: now - 1000, ..Default::default() };
        m.reactions = vec![react("✅", &["@bot:x"]), react("♾️", &["@bot:x"]), react("❌", &["@bot:x"])];
        let labels: Vec<String> = prompt_choices(&m, now, &hermes().approvals).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["Approve once", "Always", "Deny"]);
        // Answered and resolved: the bot took its reactions back.
        m.reactions = vec![react("✅", &["@me:x"])];
        assert!(prompt_choices(&m, now, &hermes().approvals).is_empty());
        // Left over from long ago: not live.
        m.reactions = vec![react("✅", &["@bot:x"])];
        assert!(prompt_choices(&m, now + PROMPT_MAX_AGE_MS + 1, &hermes().approvals).is_empty());
    }

    #[test]
    fn picker_choices_are_labelled_from_their_lines() {
        let body = "Pick a model\n1️⃣ `claude-opus` — Anthropic\n2️⃣ `gpt-5` — OpenAI";
        let m = MessageInfo {
            sender: "@bot:x".into(), body: body.into(), timestamp: 5,
            reactions: vec![react("1️⃣", &["@bot:x"]), react("2️⃣", &["@bot:x"])],
            ..Default::default()
        };
        let labels: Vec<String> = prompt_choices(&m, 10, &[]).into_iter().map(|(_, l)| l).collect();
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
        // Replies loaded but no SDK summary count yet: still the thread's run.
        app.thread_counts.insert("!r:x".into(), [("$ask".to_string(), 1)].into_iter().collect());
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now - 9000), ask.clone()]);
        app.refresh_agent_state("!r:x");
        let tk = thread_key("!r:x", "$ask");
        assert_eq!(app.run_state(&tk), Some(RunState::Working));
        assert_eq!(app.run_state("!r:x"), None, "a threaded run isn't the room's");
        ask.thread_count = Some(1);
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
    fn step_logs_are_told_apart_from_answers() {
        let log = "📖 Reading 03-where-edge-lives.md\n🔍 Searching the web for `nfl week 4 lines`\n⚙️ execute_code (×3)";
        assert_eq!(progress_steps(log).map(|s| s.len()), Some(3));
        // An answer with bullets or prose isn't a step log.
        assert_eq!(progress_steps("• JAX@CIN: Under 52: HIT\n• NE@BUF: Bills -7: MISS"), None);
        assert_eq!(progress_steps("✅ Done.\nHere's the summary of what I found today"), None, "a prose line breaks it");
        assert_eq!(progress_steps("Here's what I found"), None);
        assert_eq!(progress_steps(""), None);
    }

    #[tokio::test]
    async fn a_run_knows_its_latest_step_and_the_header_picks_the_liveliest() {
        let mut app = agent_app();
        let now = now_ms();
        let mut ask = msg("$ask", "@me:x", now - 60_000);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        let mut log = msg("$log", "@bot:x", now - 30_000);
        log.thread_root = Some("$ask".into());
        log.body = "📖 Reading a.md\n🔍 Searching the web for odds".into();
        let tk = thread_key("!r:x", "$ask");
        app.messages_by_room.insert(tk.clone(), vec![ask.clone(), log]);
        app.refresh_agent_state(&tk);
        let run = app.run_info(&tk).unwrap();
        assert_eq!(run.state, RunState::Working);
        assert_eq!(run.step.as_deref(), Some("🔍 Searching the web for odds"));
        assert_eq!(run.steps, 2);
        assert_eq!(run.since, ask.timestamp);
        // The room's header shows that run; a finished one isn't shown.
        assert_eq!(app.header_run("!r:x").map(|r| r.steps), Some(2));
        ask.reactions = vec![react(DONE, &["@bot:x"])];
        app.messages_by_room.insert(tk.clone(), vec![ask]);
        app.refresh_agent_state(&tk);
        assert!(app.header_run("!r:x").is_none());
    }

    #[tokio::test]
    async fn each_harness_gets_its_own_commands_states_and_controls() {
        let mut app = agent_app();
        let now = now_ms();
        app.rooms = vec![fluxchat_core::RoomInfo { room_id: "!r:x".into(), name: "r".into(), ..Default::default() }];
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now)]);
        // OpenClaw: `/` commands as text, with its mention in rooms; no steer / queue.
        app.prefs.agent_harness.insert("@bot:x".into(), "openclaw".into());
        assert_eq!(app.agent_outgoing("!r:x", "/stop").as_deref(), Some("@bot:x /stop"));
        assert_eq!(app.agent_outgoing("!r:x", "/me waves"), None);
        let h = crate::harness::get("openclaw");
        assert_eq!(BusyMode::available(&h), vec![BusyMode::Send]);
        // It types while it works: that's a run.
        assert!(app.live_run("!r:x").is_none());
        app.note_agent_typing("!r:x", &["@bot:x".to_string()]);
        assert_eq!(app.live_run("!r:x").map(|r| r.state), Some(RunState::Working));
        app.note_agent_typing("!r:x", &[]);
        assert!(app.live_run("!r:x").is_none());
        // In a DM, no mention.
        app.rooms[0].is_direct = true;
        assert_eq!(app.agent_outgoing("!r:x", "/new").as_deref(), Some("/new"));
        // Claude Code (Matrix channel): no stop command, so no Stop.
        app.prefs.agent_harness.insert("@bot:x".into(), "claude-code".into());
        assert_eq!(app.agent_command_text("!r:x", &crate::harness::get("claude-code").stop), None);
        // Hermes reactions mean nothing to a harness that doesn't use them.
        let mut ask = msg("$ask", "@me:x", now);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        assert_eq!(request_state(&crate::harness::get("claude-code"), &ask, "@bot:x"), None);
        assert_eq!(request_state(&hermes(), &ask, "@bot:x"), Some(RunState::Working));
    }

    #[tokio::test]
    async fn the_agent_protocol_wins_over_inference() {
        let mut app = agent_app();
        app.prefs.agent_harness.insert("@bot:x".into(), "generic".into());
        let now = now_ms();
        let ask = msg("$ask", "@me:x", now - 60_000);
        let mut status = msg("$status", "@bot:x", now - 1000);
        status.thread_root = Some("$ask".into());
        status.agent = Some(serde_json::json!({
            "v": 1, "run": "$ask", "state": "working", "step": "Running tests", "steps": 3,
            "progress": 0.5, "model": "opus-5", "context": 0.42, "cost_usd": 0.18,
        }));
        let tk = thread_key("!r:x", "$ask");
        let mut thread = vec![ask.clone(), status.clone()];
        app.take_agent_footers(&tk, &mut thread);
        app.messages_by_room.insert(tk.clone(), thread);
        app.refresh_agent_state(&tk);
        let run = app.run_info(&tk).unwrap();
        assert_eq!((run.state, run.step.as_deref(), run.steps, run.progress), (RunState::Working, Some("Running tests"), 3, Some(0.5)));
        assert_eq!(run.since, ask.timestamp);
        assert_eq!(app.agent_runtime(&tk).map(|r| r.summary()).as_deref(), Some("opus-5 \u{00b7} ctx 42% \u{00b7} $0.18"));
        // A protocol prompt: live while "waiting", gone once the agent moves on.
        let mut prompt = msg("$p", "@bot:x", now);
        prompt.agent = Some(serde_json::json!({"run": "$ask", "state": "waiting",
            "choices": [{"key": "👍", "label": "Ship it"}, {"key": "👎", "label": "Not yet"}]}));
        assert_eq!(app.agent_prompt(&prompt), vec![("👍".to_string(), "Ship it".to_string()), ("👎".to_string(), "Not yet".to_string())]);
        app.messages_by_room.insert(tk.clone(), vec![ask.clone(), status.clone(), prompt.clone()]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::NeedsYou));
        prompt.agent = Some(serde_json::json!({"run": "$ask", "state": "done", "choices": [{"key": "👍", "label": "Ship it"}]}));
        assert!(app.agent_prompt(&prompt).is_empty());
    }

    #[test]
    fn commands_go_to_the_agent_unless_they_are_fluxchats_own() {
        assert_eq!(agent_command(&hermes(), "/goal win the week").as_deref(), Some("!goal win the week"));
        assert_eq!(agent_command(&hermes(), "/status").as_deref(), Some("!status"), "the agent's /status wins in its room");
        assert_eq!(agent_command(&hermes(), "/arxiv transformers").as_deref(), Some("!arxiv transformers"), "skills pass through");
        assert_eq!(agent_command(&hermes(), "/me waves"), None, "fluxchat's /me still works");
        assert_eq!(agent_command(&hermes(), "//literal"), None);
        assert_eq!(agent_command(&hermes(), "hello"), None);
        assert!(completions(&hermes(), "/st").iter().any(|(n, _)| *n == "/stop"));
        assert_eq!(completions(&hermes(), "/status").iter().filter(|(n, _)| *n == "/status").count(), 1);
    }
}
