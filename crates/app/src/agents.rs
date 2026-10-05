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
/// A "working" run with no sign of life this long isn't running: the agent
/// most likely restarted without taking its working reaction back.
const STALE_RUN_MS: u64 = 2 * 60 * 60 * 1000;
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


/// Whether a message has a thread: what its "N replies" row goes by (the
/// SDK's summary, a latest reply, or replies loaded — `loaded` per root).
fn has_thread(m: &MessageInfo, loaded: Option<&HashMap<String, u32>>) -> bool {
    m.thread_count.unwrap_or(0) > 0
        || m.thread_latest.is_some()
        || loaded.and_then(|c| c.get(&m.event_id)).is_some_and(|n| *n > 0)
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

/// Whether the agent has posted anything after `msgs[i]` in this view: it
/// waits while a prompt of its is open, so a later message means the prompt
/// is over (answered, timed out — Hermes edits it but leaves its reactions —
/// or dropped).
/// Status lines don't count: Hermes's heartbeat ("⏳ Working — 3 min")
/// runs on its own timer and can post while a prompt waits.
fn moved_on(msgs: &[MessageInfo], i: usize, agent: &str) -> bool {
    msgs.get(i + 1..).into_iter().flatten()
        .any(|m| m.sender == agent && !m.event_id.starts_with('~') && m.row_kind == fluxchat_core::RowKind::Message
            && progress_steps(&m.body).is_none())
}

/// Whether reaction prompt `msg` says it's over (harness `h`'s markers).
fn marked_over(msg: &MessageInfo, h: &Harness) -> bool {
    let body = msg.body.trim_start();
    h.prompt_over_markers.iter().any(|p| !p.is_empty() && body.starts_with(p.as_str()))
}

/// The choices of a question the agent asked as text (harness `h`'s
/// `question_prefix`; Hermes's clarify) that's still waiting: `msgs[i]` is
/// it, and nobody has posted after it in this view. Its numbered lines
/// ("  1. choice") are the choices, as (number, text); an open question has
/// none. None = not a question, or not waiting any more.
pub fn pending_question(msgs: &[MessageInfo], i: usize, agent: &str, h: &Harness, now_ms: u64) -> Option<Vec<(String, String)>> {
    let msg = msgs.get(i)?;
    let prefix = h.question_prefix.as_str();
    if prefix.is_empty() || msg.sender != agent || !msg.body.trim_start().starts_with(prefix)
        || now_ms.saturating_sub(sent_ms(msg)) > PROMPT_MAX_AGE_MS
    {
        return None;
    }
    // Answered (your reply), expired or superseded (the agent's next message).
    // (Your answer counts while it's still sending.)
    if msgs[i + 1..].iter().any(|m| !m.event_id.starts_with('~') && m.row_kind == fluxchat_core::RowKind::Message
        && (m.sender != agent || progress_steps(&m.body).is_none()))
    {
        return None;
    }
    Some(msg.body.lines()
        .filter_map(|l| {
            let (n, rest) = l.trim().split_once(". ")?;
            (!n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && !rest.trim().is_empty())
                .then(|| (n.to_string(), rest.trim().to_string()))
        })
        .collect())
}

/// The choices of a prompt still waiting for an answer: the emoji its
/// sender reacted to it with, labelled. Empty when it isn't a prompt, it's
/// been resolved, or it's too old to still be live (`now_ms`).
pub fn prompt_choices(msg: &MessageInfo, now_ms: u64, labels: &[crate::harness::Choice]) -> Vec<(String, String)> {
    if now_ms.saturating_sub(sent_ms(msg)) > PROMPT_MAX_AGE_MS {
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

/// When a message was sent, in ms (`MessageInfo.timestamp` is in seconds;
/// everything here works in ms).
pub(crate) fn sent_ms(m: &MessageInfo) -> u64 {
    m.timestamp.saturating_mul(1000)
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

/// Whether a search found nothing (live runs, finished history).
fn list_is_empty_hint(live: &[(String, RunInfo)], history: &[&crate::history::RunRecord]) -> bool {
    live.is_empty() && !history.iter().any(|h| h.ended.is_some())
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
    /// The agent handling it (rooms can have several).
    pub agent: String,
}

impl RunInfo {
    pub(crate) fn new(state: RunState, since: u64) -> Self {
        RunInfo { state, since, step: None, steps: 0, progress: None, agent: String::new() }
    }

    /// For picking between two agents' reports on one key: the livelier.
    fn rank(&self) -> (u8, std::cmp::Reverse<u64>) {
        let r = match self.state {
            RunState::NeedsYou => 0,
            RunState::Working => 1,
            RunState::Failed => 2,
            RunState::Done => 3,
        };
        (r, std::cmp::Reverse(self.since))
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
    /// Rooms an agent is typing in: which agent, since when (ms) —
    /// "working" for harnesses that signal work that way.
    typing_since: HashMap<String, (String, u64)>,
    /// Per room, the agent you're talking to when there are several.
    pub(crate) target: HashMap<String, String>,
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
    ///
    /// With several agents in the room, the one you're talking to: the one
    /// you picked (`target`), else the one on the view's run, else the one
    /// that spoke last here.
    pub(crate) fn agent_in(&self, key: &str) -> Option<&str> {
        let agents = self.agents_in(key);
        if agents.len() <= 1 {
            return agents.into_iter().next();
        }
        let room = real_room_id(key);
        let pick = |a: &str| agents.iter().copied().find(|x| *x == a);
        self.agent_runs.target.get(room).and_then(|t| pick(t))
            .or_else(|| self.run_info(key).and_then(|r| pick(&r.agent)))
            .or_else(|| self.messages_by_room.get(key)
                .and_then(|msgs| msgs.iter().rev().find_map(|m| pick(&m.sender))))
            .or_else(|| agents.first().copied())
    }

    /// Every marked agent in a view's room: members — or, before members
    /// load, senders and reactors (an agent's 👀 can come before any reply).
    pub(crate) fn agents_in(&self, key: &str) -> Vec<&str> {
        if self.prefs.agents.is_empty() {
            return Vec::new();
        }
        let room = real_room_id(key);
        let agent = |u: &str| self.prefs.agents.iter().find(|a| *a == u).map(String::as_str);
        let members = [key, room].into_iter()
            .filter_map(|k| self.members_by_room.get(k))
            .flatten()
            .filter_map(|m| agent(&m.user_id));
        let seen = [key, room].into_iter()
            .filter_map(|k| self.messages_by_room.get(k))
            .flatten()
            .flat_map(|m| std::iter::once(m.sender.as_str()).chain(m.reactions.iter().flat_map(|r| r.users.iter().map(String::as_str))))
            .filter_map(agent);
        let mut found: Vec<&str> = Vec::new();
        for a in members.chain(seen) {
            if !found.contains(&a) {
                found.push(a);
            }
        }
        found
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
        if h.mention_commands && !self.is_dm(key) { format!("{agent} {command}") } else { command }
    }

    fn is_dm(&self, key: &str) -> bool {
        let room = real_room_id(key);
        self.rooms.iter().find(|r| r.room_id == room).is_some_and(|r| r.is_direct)
    }

    /// Whether `agent` answers requests in a thread on them in `room`: as
    /// its last finished request there went, else its harness's habit
    /// (outside DMs).
    pub(crate) fn replies_in_threads(&self, room: &str, agent: &str, h: &Harness) -> bool {
        let loaded = self.thread_counts.get(room);
        self.messages_by_room.get(room).into_iter().flatten().rev()
            .filter(|m| m.sender == self.user_id && m.thread_root.is_none())
            .find(|m| request_state(h, m, agent) == Some(RunState::Done))
            .map(|m| has_thread(m, loaded))
            .unwrap_or_else(|| h.reply_in_thread && !self.is_dm(room))
    }

    /// Track agents typing (a working signal for some harnesses).
    pub(crate) fn note_agent_typing(&mut self, room: &str, user_ids: &[String]) {
        match user_ids.iter().find(|u| self.prefs.agents.contains(u)) {
            Some(agent) => {
                let entry = self.agent_runs.typing_since.entry(room.to_string()).or_insert_with(|| (agent.clone(), now_ms()));
                if entry.0 != *agent {
                    *entry = (agent.clone(), now_ms());
                }
            }
            None => { self.agent_runs.typing_since.remove(room); }
        }
    }

    /// The run going on behind a view, if any: its own working / waiting
    /// state, else — for harnesses that work that way — the agent typing.
    pub(crate) fn live_run(&self, key: &str) -> Option<RunInfo> {
        let own = self.run_info(key).filter(|r| matches!(r.state, RunState::Working | RunState::NeedsYou)).cloned();
        own.or_else(|| {
            let (agent, since) = self.agent_runs.typing_since.get(real_room_id(key))?;
            self.harness_of(agent).typing_means_working.then(|| RunInfo { agent: agent.clone(), ..RunInfo::new(RunState::Working, *since) })
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

    /// The agent's status reactions on your request (👀 / ✅ / ❌) that the
    /// state chip on its thread row already says (room view), or its 👀 while
    /// the thread's run bar shows (thread view), to leave out of the pills.
    /// Only ones the agent alone reacted with.
    pub(crate) fn status_reactions_hidden(&self, view_key: &str, msg: &MessageInfo) -> Vec<String> {
        if msg.sender != self.user_id {
            return Vec::new();
        }
        // In the thread: the run bar says it while the run is live.
        if let Some((_, root)) = parse_thread_key(view_key) {
            // The request the bar is about: the root, or a later message of yours.
            let Some(run) = self.live_run(view_key).filter(|r| root == msg.event_id || sent_ms(msg) == r.since) else {
                return Vec::new();
            };
            let working = self.harness_of(&run.agent).working_reaction.clone();
            return msg.reactions.iter()
                .filter(|r| !working.is_empty() && r.emoji == working && r.users.len() == 1 && r.users[0] == run.agent)
                .map(|r| r.emoji.clone())
                .collect();
        }
        if msg.thread_root.is_some() {
            return Vec::new();
        }
        let replies = self.thread_counts.get(view_key).and_then(|c| c.get(&msg.event_id)).copied().unwrap_or(0)
            .max(msg.thread_count.unwrap_or(0));
        let Some(run) = self.run_info(&thread_key(view_key, &msg.event_id)).filter(|_| replies > 0) else {
            return Vec::new();
        };
        let h = self.harness_of(&run.agent);
        msg.reactions.iter()
            .filter(|r| [&h.working_reaction, &h.done_reaction, &h.failed_reaction].iter().any(|e| !e.is_empty() && **e == r.emoji))
            .filter(|r| r.users.len() == 1 && r.users[0] == run.agent)
            .map(|r| r.emoji.clone())
            .collect()
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
    pub(crate) fn agent_prompt(&self, key: &str, msg: &MessageInfo) -> Vec<(String, String)> {
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
        // Over once the agent has posted after it in this view.
        let over = self.messages_by_room.get(key)
            .and_then(|msgs| msgs.iter().rposition(|m| m.event_id == msg.event_id).map(|i| moved_on(msgs, i, &msg.sender)))
            .unwrap_or(false);
        let h = self.harness_of(&msg.sender);
        if over || marked_over(msg, &h) {
            return Vec::new();
        }
        prompt_choices(msg, now_ms(), &h.approvals)
    }

    /// A question `msg` asks as text that's still waiting in view `key`:
    /// its numbered choices (empty = answer in your own words).
    pub(crate) fn agent_question(&self, key: &str, msg: &MessageInfo) -> Option<Vec<(String, String)>> {
        if !self.prefs.agents.contains(&msg.sender) {
            return None;
        }
        let h = self.harness_of(&msg.sender);
        if h.question_prefix.is_empty() {
            return None;
        }
        let msgs = self.messages_by_room.get(key)?;
        let i = msgs.iter().rposition(|m| m.event_id == msg.event_id)?;
        pending_question(msgs, i, &msg.sender, &h, now_ms())
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
        let mut agents: Vec<String> = self.agents_in(key).into_iter().map(str::to_string).collect();
        for m in msgs.iter() {
            if self.prefs.agents.contains(&m.sender) && !agents.contains(&m.sender) {
                agents.push(m.sender.clone());
            }
        }
        for agent in agents {
            let h = self.harness_of(&agent);
            self.take_footers_of(key, msgs, &agent, &h);
        }
    }

    fn take_footers_of(&mut self, key: &str, msgs: &mut [MessageInfo], agent: &str, h: &Harness) {
        let room = real_room_id(key).to_string();
        // Runtime the agent reports through the protocol.
        for m in msgs.iter().filter(|m| m.sender == agent) {
            if let Some(rt) = Protocol::of(m).and_then(|p| p.runtime()) {
                let newer = self.agent_runs.runtime.get(&room).is_none_or(|(ts, _)| sent_ms(m) >= *ts);
                if newer {
                    self.agent_runs.runtime.insert(room.clone(), (sent_ms(m), rt));
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
            let newer = self.agent_runs.runtime.get(&room).is_none_or(|(ts, _)| sent_ms(m) >= *ts);
            if newer {
                self.agent_runs.runtime.insert(room.clone(), (sent_ms(m), rt));
            }
        }
    }

    /// Runs as the agent describes them (agent protocol) in a view's
    /// messages: per run, its latest status. Threaded requests go under
    /// their thread key; in a thread view, the thread's own key.
    fn protocol_runs(&self, key: &str, msgs: &[MessageInfo], agent: &str) -> Vec<(String, RunInfo)> {
        // Per run: its status message (the newest without choices — an edit
        // keeps its message's timestamp, so prompts mustn't outrank it), and
        // whether a prompt of it is still open.
        let mut latest: HashMap<String, (&MessageInfo, Protocol)> = HashMap::new();
        let mut open_prompt: std::collections::HashSet<String> = std::collections::HashSet::new();
        for m in msgs.iter().filter(|m| m.sender == agent) {
            let Some(p) = Protocol::of(m).filter(|p| p.run_state().is_some()) else { continue };
            let run_id = p.run.clone().unwrap_or_default();
            if !p.choices.is_empty() {
                if p.run_state() == Some(RunState::NeedsYou) {
                    open_prompt.insert(run_id.clone());
                }
                if latest.contains_key(&run_id) {
                    continue;
                }
            }
            let newer = latest.get(&run_id).is_none_or(|(old, op)| !op.choices.is_empty() || sent_ms(m) >= sent_ms(old));
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
            let state = if open_prompt.contains(&run_id) { RunState::NeedsYou } else { p.run_state()? };
            Some((target, RunInfo {
                state,
                since: request.map(sent_ms).unwrap_or_else(|| sent_ms(m)),
                step: p.step.clone(),
                steps: p.steps.map(|n| n as usize).unwrap_or(0),
                progress: p.progress.map(|f| f.clamp(0.0, 1.0)),
                agent: agent.to_string(),
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
        let agents: Vec<String> = self.agents_in(key).into_iter().map(str::to_string).collect();
        if agents.is_empty() || !self.messages_by_room.contains_key(key) {
            return Vec::new();
        }
        let now = now_ms();
        // Each agent's view of the runs here; for a key two report on, the
        // livelier wins.
        let mut merged: Vec<(String, Option<RunInfo>)> = Vec::new();
        let mut fresh: Vec<String> = Vec::new();
        for agent in &agents {
            let h = self.harness_of(agent);
            let (updates, prompts) = self.agent_updates(key, agent, &h);
            fresh.extend(prompts);
            for (k, run) in updates {
                let run = run.map(|r| RunInfo { agent: agent.clone(), ..r });
                match merged.iter_mut().find(|(m, _)| *m == k) {
                    Some((_, slot)) => {
                        let better = match (&*slot, &run) {
                            (None, Some(_)) => true,
                            (Some(old), Some(new)) => new.rank() < old.rank(),
                            _ => false,
                        };
                        if better {
                            *slot = run;
                        }
                    }
                    None => merged.push((k, run)),
                }
            }
        }
        for (k, run) in merged {
            // Working with no sign of life for hours: not running any more.
            let run = match run {
                Some(r) if self.stalled(&k, &r) => {
                    self.end_stalled_run(&k, &r);
                    None
                }
                r => r,
            };
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
            if let Some(r) = &run {
                self.record_run(&k, r);
            }
            self.agent_runs.set(k, run);
        }
        self.agent_runs.recount_needs_you();
        self.agent_runs.announced.extend(fresh.iter().cloned());
        // Only prompts from the last couple of minutes are news (not ones
        // found in history when a view loads).
        let msgs = self.messages_by_room.get(key).map(Vec::as_slice).unwrap_or(&[]);
        fresh.into_iter()
            .filter(|id| msgs.iter().any(|m| m.event_id == *id && now.saturating_sub(sent_ms(m)) < 2 * 60 * 1000))
            .collect()
    }

    /// What one agent's signals say about the runs in view `key` (its
    /// harness's reactions and progress, the protocol), and the prompts of
    /// its that are new.
    fn agent_updates(&self, key: &str, agent: &str, h: &Harness) -> (Vec<(String, Option<RunInfo>)>, Vec<String>) {
        let agent = agent.to_string();
        let Some(msgs) = self.messages_by_room.get(key) else { return (Vec::new(), Vec::new()) };
        let me = self.user_id.clone();
        let now = now_ms();
        let protocol_prompt = |m: &MessageInfo| Protocol::of(m)
            .is_some_and(|p| !p.choices.is_empty() && p.run_state() == Some(RunState::NeedsYou));
        let prompts: Vec<&MessageInfo> = msgs.iter().enumerate()
            .filter(|(i, m)| m.sender == agent && (protocol_prompt(m)
                || (!prompt_choices(m, now, &h.approvals).is_empty() && !moved_on(msgs, *i, &agent) && !marked_over(m, h))
                || pending_question(msgs, *i, &agent, h, now).is_some()))
            .map(|(_, m)| m)
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
                .filter(|m| m.sender == agent && sent_ms(m) >= since)
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
                .find_map(|m| request_state(h, m, &agent).map(|s| RunInfo::new(s, sent_ms(m))));
            let run = match (latest, needs_you) {
                (Some(mut r), true) => { r.state = RunState::NeedsYou; Some(r) }
                (None, true) => Some(RunInfo::new(RunState::NeedsYou, now)),
                (r, false) => r,
            };
            updates.push((key.to_string(), run.map(|mut r| { step_after(r.since, &mut r); r })));
        } else {
            // Threaded requests, unless their thread is open (its own view
            // knows better, approvals included).
            let loaded_replies = self.thread_counts.get(key);
            let expect_thread = self.replies_in_threads(key, &agent, h);
            let mut latest_unthreaded: Option<RunInfo> = None;
            for m in msgs.iter().filter(|m| m.sender == me && m.thread_root.is_none()) {
                let Some(state) = request_state(h, m, &agent) else { continue };
                // An agent that answers in threads: the run is the thread's
                // from the start (its bar and Stop belong there).
                let threaded = has_thread(m, loaded_replies)
                    || (expect_thread && state == RunState::Working);
                let tk = thread_key(key, &m.event_id);
                if !self.messages_by_room.contains_key(&tk) {
                    updates.push((tk, Some(RunInfo::new(state, sent_ms(m)))));
                }
                if !threaded {
                    latest_unthreaded = Some(RunInfo::new(state, sent_ms(m)));
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
        (updates, fresh)
    }

    /// The latest sign that the agent is still on run `key` (ms): the request
    /// itself, the agent's messages after it there, newer replies in its
    /// thread, or the agent typing in the room right now.
    pub(crate) fn last_sign_of_life(&self, key: &str, run: &RunInfo) -> u64 {
        let now = now_ms();
        let room = real_room_id(key);
        if self.agent_runs.typing_since.get(room).is_some_and(|(a, _)| run.agent.is_empty() || *a == run.agent) {
            return now;
        }
        let by_agent = self.messages_by_room.get(key).into_iter().flatten()
            .filter(|m| (run.agent.is_empty() || m.sender == run.agent) && !m.event_id.starts_with('~'))
            .map(sent_ms)
            .filter(|t| *t >= run.since)
            .max();
        let thread = parse_thread_key(key).and_then(|(_, root)| self.room_threads.get(room)?
            .iter().find(|t| t.root_id == root).map(|t| t.last_activity * 1000));
        [Some(run.since), by_agent, thread].into_iter().flatten().max().unwrap_or(run.since)
    }

    /// Whether a working run has shown no sign of life for `STALE_RUN_MS`.
    pub(crate) fn stalled(&self, key: &str, run: &RunInfo) -> bool {
        run.state == RunState::Working
            && now_ms().saturating_sub(run.since) > STALE_RUN_MS
            && now_ms().saturating_sub(self.last_sign_of_life(key, run)) > STALE_RUN_MS
    }

    /// Drop working runs that have gone quiet for hours (checked on Tick: a
    /// run can go stale with no message arriving to trigger a refresh).
    pub(crate) fn sweep_stalled_runs(&mut self) {
        let stale: Vec<(String, RunInfo)> = self.agent_runs.states.iter()
            .filter(|(k, r)| self.stalled(k, r))
            .map(|(k, r)| (k.clone(), r.clone()))
            .collect();
        for (k, r) in stale {
            self.end_stalled_run(&k, &r);
            self.agent_runs.set(k, None);
        }
        self.agent_runs.recount_needs_you();
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
            // Answer from the notification: its choices (as reactions in this
            // view), and Stop when the agent's harness has one.
            let mut actions: Vec<(String, Message)> = match self.agent_question(key, m) {
                // A question: answering is replying with the choice's number.
                Some(choices) => choices.into_iter().take(3)
                    .map(|(n, label)| (format!("{n}. {label}"), Message::AgentSend(key.to_string(), n)))
                    .collect(),
                None => self.agent_prompt(key, m).into_iter().take(3)
                    .map(|(emoji, label)| (format!("{emoji} {label}"), Message::AgentReact(key.to_string(), id.clone(), emoji)))
                    .collect(),
            };
            if let Some(stop) = self.agent_command_text(key, &self.harness_of(&m.sender).stop) {
                actions.push(("Stop".into(), Message::AgentSend(key.to_string(), stop)));
            }
            crate::update::show_actionable_notification(
                format!("{} needs you - {room_name}", m.sender_name),
                what,
                self.notify_sound,
                actions,
                self.timeline_tx.clone(),
                room.clone(),
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
        let agents = self.agents_in(key);
        let now = now_ms();
        // Picked up by an agent that will answer in a thread on it: open
        // the thread now, before its first reply.
        let expected = |m: &MessageInfo| agents.iter().any(|a| {
            let h = self.harness_of(a);
            request_state(&h, m, a) == Some(RunState::Working) && self.replies_in_threads(key, a, &h)
        });
        let threaded = |m: &MessageInfo| m.thread_count.unwrap_or(0) > 0;
        new.iter().rev()
            .filter(|m| m.sender == self.user_id && m.thread_root.is_none())
            .filter(|m| now.saturating_sub(sent_ms(m)) < FOLLOW_WINDOW_MS)
            .filter(|m| expected(m) || (threaded(m) && agents.iter().any(|a| request_state(&self.harness_of(a), m, a).is_some()
                || m.thread_latest.as_ref().is_some_and(|l| l.sender == *a))))
            .find(|m| !old.iter().any(|o| o.event_id == m.event_id && (threaded(o) || expected(o))))
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
        self.display_name_in(key, agent)
    }

    /// Someone's display name in a view's room, else their ID's local part.
    pub(crate) fn display_name_in(&self, key: &str, agent: &str) -> String {
        [key, real_room_id(key)].iter()
            .filter_map(|k| self.members_by_room.get(*k))
            .flatten()
            .find(|m| m.user_id == agent)
            .map(|m| m.display_name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| agent.trim_start_matches('@').split(':').next().unwrap_or(agent).to_string())
    }

    /// Whose run a view's composer shows a bar for: the view's own, else —
    /// in a room — the liveliest run in one of its threads that isn't on
    /// screen (the agent answers there).
    pub(crate) fn run_bar_key(&self, key: &str) -> Option<String> {
        if self.live_run(key).is_some() {
            return Some(key.to_string());
        }
        if key.starts_with("thread::") {
            return None;
        }
        let prefix = format!("thread::{key}::");
        self.agent_runs.states.iter()
            .filter(|(k, r)| k.starts_with(&prefix) && matches!(r.state, RunState::Working | RunState::NeedsYou) && !self.visible(k))
            .max_by_key(|(_, r)| (r.state == RunState::NeedsYou, r.since))
            .map(|(k, _)| k.clone())
    }

    /// The bar above a view's composer while the agent is on its run: what
    /// it's doing, what your next message will do, and Stop. `compact` (side
    /// panes) leaves out the switch. A thread's run shown elsewhere (a room
    /// view, its thread off screen) names the thread, which a click opens;
    /// Stop still goes to the thread.
    pub(crate) fn view_run_bar(&self, key: &str, compact: bool) -> Option<Element<'_, Message>> {
        self.run_bar(key, compact).map(|(bar, tint)| container(bar)
            .padding([4, 12])
            .width(Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(crate::theme::with_alpha(tint, 0.08).into()),
                border: iced::Border { color: crate::theme::with_alpha(tint, 0.25), width: 1.0, radius: 0.0.into() },
                ..Default::default()
            })
            .into())
    }

    /// The run bar as a rounded card at the main composer's width.
    pub(crate) fn view_run_card(&self, key: &str) -> Option<Element<'_, Message>> {
        self.run_bar(key, false).map(|(bar, tint)| container(
            container(bar)
                .padding([6, 14])
                .width(Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(crate::theme::with_alpha(tint, 0.08).into()),
                    border: iced::Border { color: crate::theme::with_alpha(tint, 0.3), width: 1.0, radius: 10.0.into() },
                    ..Default::default()
                }))
            .padding(iced::Padding { top: 6.0, right: 14.0, bottom: 0.0, left: 14.0 })
            .into())
    }

    fn run_bar(&self, key: &str, compact: bool) -> Option<(Element<'_, Message>, iced::Color)> {
        let (agent, h) = self.agent_harness(key)?;
        let run = self.live_run(key)?;
        let run = &run;
        let state = run.state;
        let name = self.agent_name(key, &agent);
        let elsewhere = parse_thread_key(key).filter(|_| !self.visible(key));
        let compact = compact || elsewhere.is_some();
        let label = match (state, &elsewhere) {
            (RunState::Working, None) => format!("{name} is working \u{00b7} {}", run.elapsed()),
            (RunState::Working, Some(_)) => format!("{name} is working in \u{201c}{}\u{201d} \u{00b7} {}",
                crate::view::ellipsize(&self.thread_title(key), 40), run.elapsed()),
            (_, None) => format!("{name} is waiting for your answer"),
            (_, Some(_)) => format!("{name} is waiting for your answer in \u{201c}{}\u{201d}",
                crate::view::ellipsize(&self.thread_title(key), 40)),
        };
        let label: Element<'_, Message> = text(label).font(self.sidebar_font()).size(12).color(crate::theme::colors().text_normal)
            .wrapping(iced::widget::text::Wrapping::None).into();
        let label = match elsewhere {
            Some((room, root)) => ui::with_tooltip(
                iced::widget::mouse_area(label)
                    .on_press(Message::OpenThread(room.to_string(), root.to_string()))
                    .interaction(iced::mouse::Interaction::Pointer),
                "Open the thread",
            ),
            None => label,
        };
        let mut bar = row![state_dot(state, 8.0), label]
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
        Some((bar.into(), tint))
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
    /// `compact` (narrow pane title bars): icon and state only, the rest in
    /// the tooltip.
    pub(crate) fn view_agent_marker(&self, key: &str, compact: bool) -> Option<Element<'_, Message>> {
        let agents = self.agents_in(key);
        let talking_to = self.agent_in(key)?;
        if agents.len() == 1 {
            return Some(self.agent_chip(key, talking_to, false, compact));
        }
        // Several agents: a chip each; the one you're talking to is outlined.
        let mut chips = row![].spacing(4).align_y(Alignment::Center);
        for a in agents {
            chips = chips.push(self.agent_chip(key, a, a == talking_to, compact));
        }
        Some(chips.into())
    }

    /// The run of `agent` a header chip speaks for (in a room with several).
    fn header_run_of(&self, key: &str, agent: &str) -> Option<RunInfo> {
        if key.starts_with("thread::") {
            return self.live_run(key).filter(|r| r.agent.is_empty() || r.agent == agent);
        }
        let prefix = format!("thread::{key}::");
        self.agent_runs.states.iter()
            .filter(|(k, r)| (*k == key || k.starts_with(&prefix)) && r.agent == agent)
            .map(|(_, r)| r)
            .filter(|r| matches!(r.state, RunState::Working | RunState::NeedsYou))
            .max_by_key(|r| (r.state == RunState::NeedsYou, r.since))
            .cloned()
            .or_else(|| self.live_run(key).filter(|r| r.agent == agent))
    }

    /// One agent's chip: name, state (idle / working · 3m with a bar /
    /// waiting on you), model and context. Click: talk to it, and its menu.
    fn agent_chip(&self, key: &str, agent: &str, outlined: bool, compact: bool) -> Element<'_, Message> {
        let name = self.agent_name(key, agent);
        let run = self.header_run_of(key, agent);
        let run = run.as_ref();
        let colors = crate::theme::colors();
        let (status, color) = match run {
            Some(r) if r.state == RunState::NeedsYou => ("waiting on you".to_string(), r.state.color()),
            Some(r) => (format!("working \u{00b7} {}", r.elapsed()), r.state.color()),
            None => ("idle".to_string(), colors.text_dim),
        };
        let tip = run.and_then(|r| r.step.clone()).unwrap_or_else(|| "Agent commands".into());
        let tip = if compact { format!("{name} \u{00b7} {status}\n{tip}") } else { tip };
        let top = row![ui::icon(crate::icons::SVG_BOT, 14.0, if run.is_some() { color } else { colors.text_dim })]
        .push((!compact).then(|| text(name).font(self.sidebar_font()).size(12).color(colors.text_normal)
            .wrapping(iced::widget::text::Wrapping::None)))
        .push((!compact).then(|| text(status).font(self.sidebar_font()).size(11).color(color)
            .wrapping(iced::widget::text::Wrapping::None)))
        .push((compact && run.is_some()).then(|| state_dot(run.map(|r| r.state).unwrap_or(RunState::Working), 7.0)))
        // Model and context fill, when it reports them (one agent only:
        // runtime is kept per room).
        .push(self.agent_runtime(key).filter(|_| !compact && self.agents_in(key).len() == 1).map(|rt| {
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
        let room = real_room_id(key).to_string();
        let border = if outlined { colors.accent } else { crate::theme::with_alpha(color, 0.3) };
        crate::ui::styled_tooltip(
            button(chip)
                .on_press(Message::Batch(vec![
                    Message::SetAgentTarget(room, agent.to_string()),
                    Message::OpenContextMenu(crate::state::ContextMenu::Agent { key: key.to_string() }),
                ]))
                .padding([3, 8])
                .style(move |_, status| button::Style {
                    background: Some(if matches!(status, button::Status::Hovered) {
                        crate::theme::with_alpha(color, 0.16)
                    } else {
                        crate::theme::with_alpha(color, 0.08)
                    }.into()),
                    border: iced::Border { color: border, width: 1.0, radius: 10.0.into() },
                    ..Default::default()
                }),
            text(tip).font(self.sidebar_font()).size(11),
            iced::widget::tooltip::Position::Bottom,
        )
    }

    /// In a room with several agents: who your messages and commands go to.
    pub(crate) fn view_agent_target(&self, key: &str) -> Option<Element<'_, Message>> {
        let agents = self.agents_in(key);
        if agents.len() < 2 {
            return None;
        }
        let current = self.agent_in(key)?.to_string();
        let room = real_room_id(key).to_string();
        let switch = ui::segmented(agents.iter()
            .map(|a| (self.agent_name(key, a), Some(Message::SetAgentTarget(room.clone(), a.to_string())), *a == current))
            .collect());
        Some(container(row![
            ui::icon(crate::icons::SVG_BOT, 13.0, crate::theme::colors().text_dim),
            text("To").font(self.sidebar_font()).size(11).color(crate::theme::colors().text_dim),
            container(switch).width(iced::Length::Fixed(140.0 * agents.len().min(4) as f32)),
        ].spacing(8).align_y(Alignment::Center)).padding([2, 12]).into())
    }

    /// An agent message that addresses another agent in the room: a hand-off
    /// you can follow (click: talk to the one it went to).
    pub(crate) fn view_handoff<'a>(&self, view_key: &str, msg: &MessageInfo) -> Option<Element<'a, Message>> {
        if !self.prefs.agents.contains(&msg.sender) {
            return None;
        }
        let html = msg.formatted_body.as_deref().unwrap_or("");
        let to = self.agents_in(view_key).into_iter()
            .filter(|a| *a != msg.sender)
            .find(|a| msg.body.contains(*a) || html.contains(*a)
                || html.contains(&a.replace('@', "%40")))?
            .to_string();
        let from_name = self.agent_name(view_key, &msg.sender);
        let to_name = self.agent_name(view_key, &to);
        let room = real_room_id(view_key).to_string();
        Some(row![
            Space::new().width(iced::Length::Fixed(self.msg_content_indent())),
            button(row![
                ui::icon(crate::icons::SVG_FORWARD, 12.0, crate::theme::colors().accent),
                text(format!("{from_name} handed off to {to_name}")).font(self.sidebar_font()).size(11)
                    .color(crate::theme::colors().accent),
            ].spacing(6).align_y(Alignment::Center))
            .on_press(Message::SetAgentTarget(room, to))
            .padding([1, 6])
            .style(ui::row_style(false)),
        ].into())
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
        let query = self.runs_query.trim().to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        let room_label = |key: &str| {
            let room = real_room_id(key);
            self.rooms.iter().find(|r| r.room_id == room)
                .map(|r| if r.is_direct { r.name.clone() } else { format!("#{}", r.name) })
                .unwrap_or_else(|| room.to_string())
        };
        let section = |label: &str| container(text(label.to_string()).font(self.sidebar_font()).size(11).color(colors.text_dim))
            .padding(iced::Padding { top: 8.0, right: 10.0, bottom: 2.0, left: 10.0 });
        let export = |key: &str| ui::with_tooltip(
            button(ui::icon(crate::icons::SVG_DOWNLOAD, 13.0, colors.text_dim))
                .on_press(Message::ExportRun(key.to_string()))
                .padding(5)
                .style(ui::button_style(Tone::Ghost)),
            "Export the transcript (Markdown)",
        );

        // Now: what's going on.
        let live: Vec<(String, RunInfo)> = self.all_runs().into_iter()
            .filter(|(_, r)| matches!(r.state, RunState::Working | RunState::NeedsYou))
            .filter(|(k, r)| {
                let hay = format!("{} {} {}", if k.starts_with("thread::") { self.thread_title(k) } else { String::new() }, room_label(k), r.agent).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect();
        let working = live.iter().filter(|(_, r)| r.state == RunState::Working).count();
        let waiting = live.len() - working;
        let subtitle = if self.prefs.agents.is_empty() {
            "Mark an agent on its profile (AI agent) to follow its runs here".to_string()
        } else {
            format!("{working} working \u{00b7} {waiting} waiting on you \u{00b7} {} in history", self.run_history.len())
        };
        let mut list = iced::widget::column![].spacing(2);
        if !live.is_empty() {
            list = list.push(section("NOW"));
        }
        for (key, run) in &live {
            let title = if key.starts_with("thread::") { self.thread_title(key) } else { "Request in the room".into() };
            let mut detail = row![
                state_dot(run.state, 7.0),
                text(format!("{} \u{00b7} {}", run.state.label(), run.elapsed())).font(self.sidebar_font()).size(11).color(run.state.color())
                    .wrapping(iced::widget::text::Wrapping::None),
                text(room_label(key)).font(self.sidebar_font()).size(11).color(colors.text_dim)
                    .wrapping(iced::widget::text::Wrapping::None),
            ].spacing(6).align_y(Alignment::Center);
            if let Some(step) = run.step.as_ref().filter(|_| run.state == RunState::Working) {
                detail = detail.push(text(step.clone()).font(self.sidebar_font()).size(11).color(colors.text_muted)
                    .wrapping(iced::widget::text::Wrapping::None));
            }
            list = list.push(row![
                button(iced::widget::column![
                    text(crate::view::ellipsize(&title, 80)).font(self.sidebar_font()).size(13).color(colors.text_bright)
                        .wrapping(iced::widget::text::Wrapping::None),
                    container(detail).clip(true),
                ].spacing(3))
                .on_press(Message::Batch(vec![Message::CloseModal, Message::OpenAt(key.clone(), crate::panes::Placement::Auto)]))
                .padding([7, 10])
                .width(Fill)
                .style(ui::row_style(false)),
                export(key),
            ].align_y(Alignment::Center));
        }

        // History: runs seen live, newest first, with how long and what it cost.
        let history: Vec<&crate::history::RunRecord> = self.matching_history(&query).into_iter()
            .filter(|h| h.ended.is_some())
            .take(200)
            .collect();
        if !history.is_empty() {
            list = list.push(section("HISTORY"));
        }
        let now = now_ms();
        for h in history {
            let state = match h.state.as_str() {
                "failed" | crate::history::STALLED => RunState::Failed,
                "needs you" => RunState::NeedsYou,
                "working" => RunState::Working,
                _ => RunState::Done,
            };
            // Went quiet while working: say so, not "failed".
            let label = if h.state == crate::history::STALLED { crate::history::STALLED } else { state.label() };
            let when = format!("{} {}", crate::view_messages::local_day_label(h.started / 1000),
                crate::view_messages::format_clock(crate::view::local_time_of_day(h.started / 1000), false, self.prefs.twelve_hour));
            let extras = [Some(h.duration(now)), h.model.clone(), h.cost.clone()].into_iter().flatten().collect::<Vec<_>>().join(" \u{00b7} ");
            list = list.push(row![
                button(iced::widget::column![
                    text(crate::view::ellipsize(&h.title, 80)).font(self.sidebar_font()).size(13).color(colors.text_normal)
                        .wrapping(iced::widget::text::Wrapping::None),
                    container(row![
                        state_dot(state, 7.0),
                        text(format!("{label} \u{00b7} {extras}")).font(self.sidebar_font()).size(11).color(state.color())
                            .wrapping(iced::widget::text::Wrapping::None),
                        text(format!("{} \u{00b7} {when}", room_label(&h.key))).font(self.sidebar_font()).size(11).color(colors.text_dim)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(6).align_y(Alignment::Center)).clip(true),
                ].spacing(3))
                .on_press(Message::Batch(vec![Message::CloseModal, Message::OpenAt(h.key.clone(), crate::panes::Placement::Auto)]))
                .padding([7, 10])
                .width(Fill)
                .style(ui::row_style(false)),
                export(&h.key),
            ].align_y(Alignment::Center));
        }
        if live.is_empty() && self.run_history.is_empty() {
            list = list.push(ui::empty_state(crate::icons::SVG_BOT, "No runs yet", Some("Runs show here as your agents work.".into())));
        } else if !words.is_empty() && list_is_empty_hint(&live, &self.matching_history(&query)) {
            list = list.push(ui::empty_state(crate::icons::SVG_SEARCH, "Nothing matches", None));
        }
        iced::widget::column![
            ui::header("Agent runs", Some(subtitle)),
            ui::input("Search runs: title, room, agent", &self.runs_query, Message::RunsQuery),
            container(iced::widget::scrollable(list).height(iced::Length::Shrink))
                .max_height(460.0),
            ui::footer(vec![
                ui::icon_btn(crate::icons::SVG_LAYERS, if working + waiting > 0 { "Watch in a window" } else { "Watch for new runs" }, Tone::Primary,
                    Some(Message::WatchRunsWindow)).into(),
            ]),
        ]
        .spacing(ui::GAP)
        .width(iced::Length::Fixed(640.0))
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

    /// Someone in view `key` who acts like an agent but isn't marked one, and
    /// the harness it looks like: Hermes's 👀 / ✅ on your messages or its
    /// step logs → Hermes; prompts it reacts to itself with ✅ / ❌ → Claude
    /// Code's Matrix channel (Hermes if it offers 🌀 / ♾️); protocol fields →
    /// generic. Only from recent messages, and never for someone you said
    /// isn't an agent.
    pub(crate) fn suggested_agent(&self, key: &str) -> Option<(String, &'static str)> {
        let msgs = self.messages_by_room.get(key)?;
        let candidate = |u: &str| u != self.user_id && !self.prefs.agents.iter().any(|a| a == u)
            && !self.prefs.not_agents.iter().any(|a| a == u);
        let recent = &msgs[msgs.len().saturating_sub(80)..];
        // Status reactions (👀 / ✅ / ❌) on several of your messages by one
        // person: Hermes's acks (people react with these too, so ≥ 3).
        let mut acks: HashMap<&str, usize> = HashMap::new();
        for m in recent.iter().filter(|m| m.sender == self.user_id) {
            let mut by: Vec<&str> = m.reactions.iter()
                .filter(|r| matches!(r.emoji.trim_end_matches('\u{fe0f}'), "\u{1f440}" | "\u{2705}" | "\u{274c}"))
                .flat_map(|r| r.users.iter().map(String::as_str))
                .filter(|u| candidate(u))
                .collect();
            by.sort_unstable();
            by.dedup();
            for u in by {
                *acks.entry(u).or_default() += 1;
            }
        }
        if let Some((u, _)) = acks.into_iter().filter(|(_, n)| *n >= 3).max_by_key(|(_, n)| *n) {
            return Some((u.to_string(), "hermes"));
        }
        for m in recent.iter().rev() {
            if candidate(&m.sender) && m.agent.is_some() {
                return Some((m.sender.clone(), "generic"));
            }
            if candidate(&m.sender) {
                let own: Vec<&str> = m.reactions.iter()
                    .filter(|r| r.users.contains(&m.sender))
                    .map(|r| r.emoji.trim_end_matches('\u{fe0f}'))
                    .collect();
                let approval = own.contains(&"\u{2705}") && own.contains(&"\u{274c}");
                if approval {
                    let hermes = own.iter().any(|e| *e == "\u{1f300}" || *e == "\u{267e}");
                    return Some((m.sender.clone(), if hermes { "hermes" } else { "claude-code" }));
                }
                if progress_steps(&m.body).is_some_and(|s| s.len() > 1) {
                    return Some((m.sender.clone(), "hermes"));
                }
            }
        }
        None
    }

    /// "@x looks like an AI agent (Hermes Agent)": mark it with that harness, or not.
    pub(crate) fn view_agent_suggestion(&self, key: &str) -> Option<Element<'_, Message>> {
        let (user, harness) = self.suggested_agent(key)?;
        let name = self.agent_name(key, &user);
        let h = crate::harness::get(harness);
        Some(container(row![
            ui::icon(crate::icons::SVG_BOT, 14.0, crate::theme::colors().accent),
            text(format!("{name} looks like an AI agent ({}).", h.name)).font(self.sidebar_font()).size(12)
                .color(crate::theme::colors().text_normal).wrapping(iced::widget::text::Wrapping::None),
            Space::new().width(Fill),
            ui::btn("Mark as agent", Tone::Primary, Some(Message::Batch(vec![
                Message::SetAgentHarness(user.clone(), harness.to_string()),
                Message::SetAgent(user.clone(), true),
            ]))),
            ui::btn("It isn't", Tone::Ghost, Some(Message::NotAnAgent(user))),
        ].spacing(8).align_y(Alignment::Center))
        .padding([4, 12])
        .width(Fill)
        .style(|_: &Theme| container::Style {
            background: Some(crate::theme::with_alpha(crate::theme::colors().accent, 0.08).into()),
            ..Default::default()
        })
        .into())
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
    /// A question the agent asked as text, still waiting: a button per
    /// numbered choice (sends its number into the view, as the agent
    /// expects), or — an open question — a note to answer below.
    pub(crate) fn view_question_card<'a>(&self, view_key: &str, choices: Vec<(String, String)>) -> Element<'a, Message> {
        let colors = crate::theme::colors();
        let status = if choices.is_empty() { "Waiting for your answer: reply below" } else { "Waiting for you: pick one, or reply in your own words" };
        let mut col = iced::widget::column![
            row![
                state_dot(RunState::NeedsYou, 7.0),
                text(status).font(self.sidebar_font()).size(11).color(colors.text_dim),
            ].spacing(6).align_y(Alignment::Center),
        ].spacing(4);
        for (n, label) in choices {
            col = col.push(
                button(row![
                    text(format!("{n}.")).font(self.sidebar_font()).size(12).color(colors.accent),
                    text(crate::view::ellipsize(&label, 90)).font(self.sidebar_font()).size(12)
                        .wrapping(iced::widget::text::Wrapping::None),
                ].spacing(8).align_y(Alignment::Center))
                .on_press(Message::AgentSend(view_key.to_string(), n))
                .padding([4, 10])
                .style(ui::button_style(Tone::Secondary)),
            );
        }
        row![
            Space::new().width(iced::Length::Fixed(self.msg_content_indent())),
            container(col)
                .padding([8, 10])
                .clip(true)
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
        let mut m = MessageInfo { sender: "@bot:x".into(), body: body.into(), timestamp: (now - 1000) / 1000, ..Default::default() };
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
            sender: "@bot:x".into(), body: body.into(), timestamp: 0,
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
        // MessageInfo.timestamp is in seconds (the SDK's origin_server_ts / 1000).
        MessageInfo { event_id: id.into(), sender: sender.into(), body: id.into(), timestamp: ts / 1000, ..Default::default() }
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
        // Its last finished request here was answered in the room: it
        // doesn't thread, so only its first threaded reply makes a thread.
        let mut earlier = msg("$earlier", "@me:x", now - 9000);
        earlier.reactions = vec![react(DONE, &["@bot:x"])];
        let mut ask = msg("$ask", "@me:x", now - 3000);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        // First load: nothing to compare with, nothing followed.
        let mut threaded = ask.clone();
        threaded.thread_count = Some(1);
        let follow = |app: &FluxChat, new: &MessageInfo| app.thread_to_follow("!r:x", &[earlier.clone(), new.clone()]);
        assert_eq!(follow(&app, &threaded), None);
        app.messages_by_room.insert("!r:x".into(), vec![earlier.clone(), ask.clone()]);
        // The agent's first reply creates the thread.
        assert_eq!(follow(&app, &threaded), Some(thread_key("!r:x", "$ask")));
        // Already threaded before: not again.
        app.messages_by_room.insert("!r:x".into(), vec![earlier.clone(), threaded.clone()]);
        assert_eq!(follow(&app, &threaded), None);
        // An old request, or someone else's thread: no.
        let mut old = threaded.clone();
        old.timestamp = (now - FOLLOW_WINDOW_MS - 2000) / 1000;
        app.messages_by_room.insert("!r:x".into(), vec![earlier.clone(), ask.clone()]);
        assert_eq!(follow(&app, &old), None);
        // Turned off, or not the room you're in: no.
        app.prefs.follow_agent_threads = false;
        assert_eq!(follow(&app, &threaded), None);
        app.prefs.follow_agent_threads = true;
        let _ = app.update(Message::SelectRoom("!other:x".into()));
        assert_eq!(follow(&app, &threaded), None);
    }

    #[tokio::test]
    async fn an_agent_that_answers_in_threads_has_its_run_there_from_the_first_reaction() {
        let mut app = agent_app();
        let _ = app.update(Message::SelectRoom("!r:x".into()));
        let now = now_ms();
        let tk = thread_key("!r:x", "$ask");
        let sent = msg("$ask", "@me:x", now - 3000);
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now - 9000), sent.clone()]);
        // Hermes threads its answers (outside DMs): its 👀 opens the thread…
        let mut picked_up = sent.clone();
        picked_up.reactions = vec![react(WORKING, &["@bot:x"])];
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&picked_up)), Some(tk.clone()));
        app.messages_by_room.insert("!r:x".into(), vec![picked_up.clone()]);
        // …and the run is the thread's, not the room's: the room's composer
        // shows it as the thread's (Stop goes there).
        app.refresh_agent_state("!r:x");
        assert_eq!(app.run_state(&tk), Some(RunState::Working));
        assert_eq!(app.run_state("!r:x"), None);
        assert_eq!(app.run_bar_key("!r:x"), Some(tk.clone()));
        assert_eq!(app.agent_outgoing("!r:x", "also check B"), None, "the room isn't busy");
        // Its first reply doesn't open the thread a second time.
        let mut replied = picked_up.clone();
        replied.thread_count = Some(1);
        assert_eq!(app.thread_to_follow("!r:x", std::slice::from_ref(&replied)), None);
        // Once a finished request here went unthreaded, it's taken at its word.
        let mut plain = msg("$plain", "@me:x", now - 6000);
        plain.reactions = vec![react(DONE, &["@bot:x"])];
        app.messages_by_room.insert("!r:x".into(), vec![plain, picked_up]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.run_state("!r:x"), Some(RunState::Working));
        assert_eq!(app.run_bar_key("!r:x").as_deref(), Some("!r:x"));
    }

    #[tokio::test]
    async fn an_approval_is_over_when_it_times_out_or_the_agent_moves_on() {
        let mut app = agent_app();
        let now = now_ms();
        let tk = thread_key("!r:x", "$root");
        let mut root = msg("$root", "@me:x", now - 300_000);
        root.reactions = vec![react(WORKING, &["@bot:x"])];
        let mut prompt = msg("$p", "@bot:x", now - 200_000);
        prompt.thread_root = Some("$root".into());
        prompt.body = "\u{26a0}\u{fe0f} Dangerous command\n`curl …`".into();
        prompt.reactions = vec![react("\u{2705}", &["@bot:x", "@me:x"]), react("\u{1f300}", &["@bot:x"]), react("\u{274c}", &["@bot:x"])];
        let mut beat = msg("$beat", "@bot:x", now - 100_000);
        beat.thread_root = Some("$root".into());
        beat.body = "\u{23f3} Working \u{2014} 3 min \u{2014} iteration 3/150, receiving stream response".into();
        // Its heartbeat runs on a timer: still waiting.
        app.messages_by_room.insert(tk.clone(), vec![root.clone(), prompt.clone(), beat.clone()]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::NeedsYou));
        assert!(!app.agent_prompt(&tk, &prompt).is_empty());
        // Timed out: Hermes edits it, reactions stay.
        let mut timed_out = prompt.clone();
        timed_out.body = "\u{231b} Approval timed out after 1 minute \u{2014} the command was NOT run.".into();
        app.messages_by_room.insert(tk.clone(), vec![root.clone(), timed_out.clone(), beat.clone()]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::Working));
        assert!(app.agent_prompt(&tk, &timed_out).is_empty(), "no buttons");
        // Not edited, but it carried on talking: over too.
        let mut said = msg("$said", "@bot:x", now - 50_000);
        said.thread_root = Some("$root".into());
        said.body = "Let me pull the most relevant thread.".into();
        app.messages_by_room.insert(tk.clone(), vec![root, prompt.clone(), beat, said]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::Working));
        assert!(app.agent_prompt(&tk, &prompt).is_empty());
    }

    #[tokio::test]
    async fn a_question_asked_as_text_waits_on_you_until_you_reply() {
        let mut app = agent_app();
        let now = now_ms();
        let tk = thread_key("!r:x", "$root");
        let mut root = msg("$root", "@me:x", now - 120_000);
        root.reactions = vec![react(DONE, &["@bot:x"])];
        let mut ask = msg("$ask", "@me:x", now - 30_000);
        ask.thread_root = Some("$root".into());
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        let mut q = msg("$q", "@bot:x", now - 20_000);
        q.thread_root = Some("$root".into());
        q.body = "\u{2753} Which docs should I index into Hindsight now?\n\nReply \"skip\" to skip this question.\n\n  \
                  1. All of them in attachments/ (Recommended)\n  2. Just specific ones\n  3. Going forward only\n\n\
                  Reply with the number, the option text, or your own answer.".into();
        app.messages_by_room.insert(tk.clone(), vec![root.clone(), ask.clone(), q.clone()]);
        let fresh = app.refresh_agent_state(&tk);
        assert_eq!(fresh, vec!["$q".to_string()], "announced");
        assert_eq!(app.run_state(&tk), Some(RunState::NeedsYou));
        let choices = app.agent_question(&tk, &q).unwrap();
        assert_eq!(choices.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["1", "2", "3"]);
        assert_eq!(choices[1].1, "Just specific ones");
        // The bar says it: the 👀 on the request it's about isn't a pill.
        assert_eq!(app.status_reactions_hidden(&tk, &ask), vec![WORKING.to_string()]);
        // You answer (still sending counts): working again, no buttons.
        let mut answer = msg("$a", "@me:x", now);
        answer.thread_root = Some("$root".into());
        answer.body = "1".into();
        app.messages_by_room.insert(tk.clone(), vec![root, ask, q.clone(), answer]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::Working));
        assert_eq!(app.agent_question(&tk, &q), None);
    }

    #[tokio::test]
    async fn a_run_that_went_quiet_for_hours_is_stalled_not_working() {
        let mut app = agent_app();
        let now = now_ms();
        let hours = |h: u64| h * 60 * 60 * 1000;
        // Picked up (👀) five hours ago, never finished, nothing since: the
        // agent restarted without taking its reaction back.
        let mut plain = msg("$plain", "@me:x", now - hours(9));
        plain.reactions = vec![react(DONE, &["@bot:x"])];
        let mut ask = msg("$ask", "@me:x", now - hours(5));
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        app.messages_by_room.insert("!r:x".into(), vec![plain.clone(), ask.clone()]);
        app.run_history.push(crate::history::RunRecord {
            key: "!r:x".into(), room: "!r:x".into(), agent: "@bot:x".into(), title: "Request in the room".into(),
            state: "working".into(), started: sent_ms(&ask), ended: None, steps: 0, model: None, cost: None,
        });
        app.refresh_agent_state("!r:x");
        assert_eq!(app.run_state("!r:x"), None, "not running");
        let rec = app.run_history.last().unwrap();
        assert_eq!((rec.state.as_str(), rec.ended), (crate::history::STALLED, Some(sent_ms(&ask))));
        // Still talking an hour ago: a long run, not a stalled one.
        let mut said = msg("$said", "@bot:x", now - hours(1));
        said.body = "halfway there".into();
        app.messages_by_room.insert("!r:x".into(), vec![plain.clone(), ask.clone(), said]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.run_state("!r:x"), Some(RunState::Working));
        // Quiet, but typing right now: working.
        app.messages_by_room.insert("!r:x".into(), vec![plain, ask]);
        app.note_agent_typing("!r:x", &["@bot:x".to_string()]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.run_state("!r:x"), Some(RunState::Working));
        // Typing stops; the tick sweep notices without a new message.
        app.note_agent_typing("!r:x", &[]);
        app.sweep_stalled_runs();
        assert_eq!(app.run_state("!r:x"), None);
    }

    #[tokio::test]
    async fn the_thread_chip_replaces_the_agents_status_reactions() {
        let mut app = agent_app();
        let now = now_ms();
        let mut ask = msg("$ask", "@me:x", now - 5000);
        ask.reactions = vec![react(DONE, &["@bot:x"]), react("\u{1f44d}", &["@friend:x"])];
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now - 9000), ask.clone()]);
        app.refresh_agent_state("!r:x");
        // No thread row yet (nothing else says it): the ✅ stays.
        assert!(app.status_reactions_hidden("!r:x", &ask).is_empty());
        // A thread row with its "done" chip: the agent's ✅ goes, the 👍 stays.
        ask.thread_count = Some(2);
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now - 9000), ask.clone()]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.status_reactions_hidden("!r:x", &ask), vec![DONE.to_string()]);
        // Someone else ✅'d it too: that's a reaction, not a status.
        ask.reactions = vec![react(DONE, &["@bot:x", "@friend:x"])];
        assert!(app.status_reactions_hidden("!r:x", &ask).is_empty());
    }

    #[tokio::test]
    async fn outgoing_messages_get_agent_commands_and_the_busy_mode() {
        let mut app = agent_app();
        let now = now_ms();
        // No agent in the room: untouched.
        app.messages_by_room.insert("!plain:x".into(), vec![msg("$a", "@friend:x", now)]);
        assert_eq!(app.agent_outgoing("!plain:x", "/goal x"), None);
        // With the agent (one that answers in the room, not in threads).
        let mut done = msg("$done", "@me:x", now - 9000);
        done.reactions = vec![react(DONE, &["@bot:x"])];
        let mut ask = msg("$ask", "@me:x", now);
        ask.reactions = vec![react(WORKING, &["@bot:x"])];
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now), done, ask]);
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
        assert_eq!(run.since, ask.timestamp * 1000);
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
        assert_eq!(run.since, ask.timestamp * 1000);
        assert_eq!(app.agent_runtime(&tk).map(|r| r.summary()).as_deref(), Some("opus-5 \u{00b7} ctx 42% \u{00b7} $0.18"));
        // A protocol prompt: live while "waiting", gone once the agent moves on.
        let mut prompt = msg("$p", "@bot:x", now);
        prompt.agent = Some(serde_json::json!({"run": "$ask", "state": "waiting",
            "choices": [{"key": "👍", "label": "Ship it"}, {"key": "👎", "label": "Not yet"}]}));
        assert_eq!(app.agent_prompt("!r:x", &prompt), vec![("👍".to_string(), "Ship it".to_string()), ("👎".to_string(), "Not yet".to_string())]);
        app.messages_by_room.insert(tk.clone(), vec![ask.clone(), status.clone(), prompt.clone()]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::NeedsYou));
        prompt.agent = Some(serde_json::json!({"run": "$ask", "state": "working", "choices": [{"key": "👍", "label": "Ship it"}]}));
        assert!(app.agent_prompt("!r:x", &prompt).is_empty());
        // Answered, then the status message is edited to done: the run is
        // done, though the prompt (sent later) still says "working".
        status.agent = Some(serde_json::json!({"run": "$ask", "state": "done", "steps": 3}));
        app.messages_by_room.insert(tk.clone(), vec![ask.clone(), status, prompt]);
        app.refresh_agent_state(&tk);
        assert_eq!(app.run_state(&tk), Some(RunState::Done));
    }

    #[tokio::test]
    async fn rooms_with_several_agents_track_each_and_talk_to_one() {
        let mut app = agent_app();
        app.prefs.agents.push("@coder:x".into());
        app.prefs.agent_harness.insert("@coder:x".into(), "generic".into());
        let now = now_ms();
        // Hermes works on one request; the coder reports on another (protocol).
        let mut ask_bot = msg("$a1", "@me:x", now - 9000);
        ask_bot.reactions = vec![react(WORKING, &["@bot:x"])];
        let ask_coder = msg("$a2", "@me:x", now - 5000);
        let mut coder = msg("$c1", "@coder:x", now - 1000);
        coder.agent = Some(serde_json::json!({"run": "$a2", "state": "working", "step": "Writing tests"}));
        app.messages_by_room.insert("!r:x".into(), vec![msg("$hi", "@bot:x", now - 20_000), ask_bot, ask_coder, coder]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.agents_in("!r:x"), ["@bot:x", "@coder:x"]);
        let bot_run = app.run_info(&thread_key("!r:x", "$a1")).unwrap();
        assert_eq!((bot_run.state, bot_run.agent.as_str()), (RunState::Working, "@bot:x"));
        let coder_run = app.run_info("!r:x").unwrap();
        assert_eq!((coder_run.agent.as_str(), coder_run.step.as_deref()), ("@coder:x", Some("Writing tests")));
        // Talking to: the one on the room's run here, then whoever you pick.
        assert_eq!(app.agent_in("!r:x"), Some("@coder:x"));
        let _ = app.update(Message::SetAgentTarget("!r:x".into(), "@bot:x".into()));
        assert_eq!(app.agent_in("!r:x"), Some("@bot:x"));
        assert_eq!(app.agent_outgoing("!r:x", "/stop").as_deref(), Some("!stop"), "Hermes's way");
        // A hand-off: the bot addresses the coder.
        let handoff = MessageInfo { body: "@coder:x can you take the tests?".into(), ..msg("$h", "@bot:x", now) };
        assert!(app.view_handoff("!r:x", &handoff).is_some());
        assert!(app.view_handoff("!r:x", &msg("$n", "@bot:x", now)).is_none());
    }

    #[tokio::test]
    async fn agents_are_suggested_from_how_they_act() {
        let mut app = agent_app();
        app.prefs.agents.clear();
        let now = now_ms();
        let mut mine: Vec<MessageInfo> = (0..3).map(|i| {
            let mut m = msg(&format!("$m{i}"), "@me:x", now - 60_000 + i * 1000);
            m.reactions = vec![react(DONE, &["@bot:x"])];
            m
        }).collect();
        // One ack isn't enough (people react with ✅ too).
        app.messages_by_room.insert("!r:x".into(), mine[..1].to_vec());
        assert_eq!(app.suggested_agent("!r:x"), None);
        app.messages_by_room.insert("!r:x".into(), mine.clone());
        assert_eq!(app.suggested_agent("!r:x"), Some(("@bot:x".to_string(), "hermes")));
        // Said it isn't: never again.
        let _ = app.update(Message::NotAnAgent("@bot:x".into()));
        assert_eq!(app.suggested_agent("!r:x"), None);
        // A prompt someone reacts to themselves with ✅ / ❌: the Claude Code channel.
        let mut prompt = msg("$p", "@cc:x", now);
        prompt.reactions = vec![react("\u{2705}", &["@cc:x"]), react("\u{274c}", &["@cc:x"])];
        mine.push(prompt);
        app.messages_by_room.insert("!r:x".into(), mine);
        assert_eq!(app.suggested_agent("!r:x"), Some(("@cc:x".to_string(), "claude-code")));
        // Marked: no suggestion.
        app.prefs.agents.push("@cc:x".into());
        assert_eq!(app.suggested_agent("!r:x"), None);
    }

    #[tokio::test]
    async fn a_coding_agents_diff_is_reviewed_and_answered() {
        let mut app = agent_app();
        let _ = app.update(Message::SelectRoom("!r:x".into()));
        let now = now_ms();
        let body = "Here's the fix:\n```diff\n--- a/x.py\n+++ b/x.py\n@@ -1,2 +1,2 @@\n-print(1)\n+print(2)\n keep\n```";
        let patch = MessageInfo { body: body.into(), ..msg("$d", "@bot:x", now) };
        assert_eq!(crate::review::summary(&patch), Some((1, 1, 1)));
        app.messages_by_room.insert("!r:x".into(), vec![patch]);
        let _ = app.update(Message::OpenReview("!r:x".into(), "$d".into()));
        let r = app.review.as_ref().unwrap();
        assert_eq!((r.files.len(), r.files[0].path.as_str()), (1, "x.py"));
        assert!(matches!(app.modal, Some(crate::state::Modal::Review)));
        let _ = app.update(Message::ReviewComment("rename it".into()));
        let _ = app.update(Message::ReviewFinish(false));
        assert!(app.review.is_none() && app.modal.is_none());
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
