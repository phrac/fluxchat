//! Ask several agents the same thing and compare their answers side by side.
//!
//! In a room with two or more agents, "Ask all" (next to the "To" switch)
//! arms the composer: the next task goes to every agent there as its own
//! message mentioning just that agent ("@Name task"), so each answers it in
//! its own thread and session — Hermes, by default, only answers mentions
//! outside DMs. Agents over their budget are left out.
//!
//! The Compare dialog shows one column per agent: its run state, time and
//! cost, and its answer (its messages in its thread, or its replies to the
//! request in the room; step logs left out). Threads not loaded yet are
//! opened in the background. "Open threads side by side" puts them in a new
//! window, one pane each. Comparisons last the session.

use std::collections::HashSet;

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Fill, Length, Task, Theme};

use fluxchat_core::timeline::{real_room_id, thread_key};
use fluxchat_core::MessageInfo;

use crate::agents::{progress_steps, request_state, sent_ms, state_dot, RunState};
use crate::message::Message;
use crate::state::FluxChat;
use crate::ui::{self, Tone};

/// How far back a sent comparison's messages are looked for (ms): local
/// echoes keep the send time, the server's can differ a little.
const MATCH_SLACK_MS: u64 = 60_000;
/// Answer column size in the dialog; more columns than fit scroll sideways.
const ANSWER_HEIGHT: f32 = 420.0;
const COLUMN_WIDTH: f32 = 340.0;
const COLUMN_GAP: f32 = 10.0;
/// Columns shown without scrolling.
const COLUMNS_SHOWN: usize = 3;

#[derive(Debug, Clone)]
pub struct Comparison {
    pub room: String,
    pub prompt: String,
    /// When it was sent (ms).
    pub created: u64,
    pub entries: Vec<Entry>,
    /// Agents left out (over budget).
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub agent: String,
    /// The message sent to it ("@Name task").
    pub body: String,
}

#[derive(Debug, Default)]
pub struct CompareState {
    /// Rooms whose next task goes to every agent.
    pub(crate) armed: HashSet<String>,
    pub(crate) list: Vec<Comparison>,
    /// The comparison the dialog shows.
    pub(crate) shown: usize,
    /// Thread timelines being opened for comparisons.
    loading: HashSet<String>,
}

/// One agent's column, worked out from the loaded timelines.
struct Answer<'a> {
    state: Option<RunState>,
    /// "4m", "$0.12" — what's known.
    details: Vec<String>,
    messages: Vec<&'a MessageInfo>,
    /// The thread it answers in (exists or expected).
    thread: Option<String>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl FluxChat {
    /// Whether "Ask all" can show for view `key`: a room (not a thread) with
    /// two or more agents.
    pub(crate) fn can_compare(&self, key: &str) -> bool {
        !key.starts_with("thread::") && self.agents_in(key).len() >= 2
    }

    /// Whether the composer in view `key` sends the next task to every agent.
    pub(crate) fn compare_armed(&self, key: &str) -> bool {
        self.compare.armed.contains(key) && self.can_compare(key)
    }

    /// Send `text` to every agent in `room`, one message each, and show the
    /// Compare dialog for it.
    pub(crate) fn send_comparison(&mut self, room: String, text: String) -> Task<Message> {
        self.compare.armed.remove(&room);
        let dm = self.rooms.iter().any(|r| r.room_id == room && r.is_direct);
        let mut entries = Vec::new();
        let mut skipped = Vec::new();
        for agent in self.agents_in(&room).into_iter().map(str::to_string).collect::<Vec<_>>() {
            let over = self.budget_status(&agent).is_some_and(|st| st.level == crate::budgets::Level::Over);
            if over {
                skipped.push(agent);
                continue;
            }
            // The display name resolves to a pill + m.mentions when sent.
            let body = if dm { text.clone() } else { format!("@{} {text}", self.display_name_in(&room, &agent)) };
            entries.push(Entry { agent, body });
        }
        if entries.is_empty() {
            self.help_text = Some("Every agent here is over its budget: nothing was sent.".into());
            return Task::none();
        }
        let bodies: Vec<String> = entries.iter().map(|e| e.body.clone()).collect();
        self.compare.list.push(Comparison { room: room.clone(), prompt: text, created: now_ms(), entries, skipped });
        self.compare.shown = self.compare.list.len() - 1;
        self.modal = Some(crate::state::Modal::Compare);
        let core = self.core.clone();
        let markdown = self.prefs.use_markdown;
        Task::perform(
            async move {
                let client = core.get_client().await.map_err(|e| e.to_string())?;
                // In order, so the requests read top to bottom like the columns.
                for body in bodies {
                    fluxchat_core::timeline::send_text(&client, &core.timelines, &room, &body, None, markdown)
                        .await.map_err(|e| e.to_string())?;
                }
                Ok::<_, String>(())
            },
            |r| match r {
                Ok(()) => Message::Noop,
                Err(e) => Message::ShowError(format!("Couldn't send to every agent: {e}")),
            },
        )
    }

    /// The request a comparison sent to one agent, once it's in the room.
    fn compare_request<'a>(&'a self, c: &Comparison, e: &Entry) -> Option<&'a MessageInfo> {
        self.messages_by_room.get(&c.room)?.iter().find(|m| {
            m.sender == self.user_id && m.thread_root.is_none() && m.body.trim() == e.body.trim()
                && sent_ms(m) + MATCH_SLACK_MS >= c.created
        })
    }

    /// Whether `m` is a request one of `room`'s comparisons sent (its thread
    /// isn't followed into a pane: the Compare dialog shows them all).
    pub(crate) fn is_comparison_request(&self, room: &str, m: &MessageInfo) -> bool {
        self.compare.list.iter()
            .filter(|c| c.room == room)
            .any(|c| c.entries.iter().any(|e| m.body.trim() == e.body.trim() && sent_ms(m) + MATCH_SLACK_MS >= c.created))
    }

    fn compare_answer<'a>(&'a self, c: &Comparison, e: &Entry) -> Answer<'a> {
        let Some(req) = self.compare_request(c, e) else {
            return Answer { state: None, details: vec!["Sending\u{2026}".into()], messages: Vec::new(), thread: None };
        };
        let tk = thread_key(&c.room, &req.event_id);
        let h = self.harness_of(&e.agent);
        let run = self.run_info(&tk).filter(|r| r.agent.is_empty() || r.agent == e.agent);
        let state = run.map(|r| r.state).or_else(|| request_state(&h, req, &e.agent));
        let threaded = req.thread_count.unwrap_or(0) > 0 || req.thread_latest.is_some() || run.is_some();
        let answer = |m: &&MessageInfo| m.sender == e.agent && !m.is_system() && progress_steps(&m.body).is_none();
        let mut messages: Vec<&MessageInfo> = match self.messages_by_room.get(&tk) {
            Some(thread) => thread.iter().filter(answer).collect(),
            None => Vec::new(),
        };
        // Answered in the room: its replies to the request.
        if messages.is_empty() {
            messages = self.messages_by_room.get(&c.room).into_iter().flatten()
                .filter(answer)
                .filter(|m| m.reply_to_id.as_deref() == Some(req.event_id.as_str()))
                .collect();
        }
        let mut details = Vec::new();
        if let Some(r) = run.filter(|r| matches!(r.state, RunState::Working | RunState::NeedsYou)) {
            details.push(r.elapsed());
            if let Some(step) = &r.step {
                details.push(step.clone());
            }
        }
        if let Some(rec) = self.run_history.iter().rev().find(|r| r.key == tk && r.started == sent_ms(req)) {
            if rec.ended.is_some() {
                details.push(rec.duration(now_ms()));
            }
            details.extend(rec.cost.clone());
        }
        Answer { state, details, messages, thread: threaded.then_some(tk) }
    }

    /// Open the threads of the shown comparison that aren't loaded (after the
    /// room's messages change, and when the dialog opens).
    pub(crate) fn load_compare_threads(&mut self) -> Task<Message> {
        if !matches!(self.modal, Some(crate::state::Modal::Compare)) {
            return Task::none();
        }
        let Some(c) = self.compare.list.get(self.compare.shown).cloned() else { return Task::none() };
        let missing: Vec<String> = c.entries.iter()
            .filter_map(|e| self.compare_answer(&c, e).thread)
            .filter(|k| !self.messages_by_room.contains_key(k))
            .collect();
        let mut tasks = Vec::new();
        for key in missing {
            if !self.compare.loading.insert(key.clone()) {
                continue;
            }
            let core = self.core.clone();
            tasks.push(Task::perform(
                async move {
                    let client = core.get_client().await.ok()?;
                    if !core.timelines.request_snapshot(&key).await {
                        fluxchat_core::timeline::get_or_create_for_key(&client, &core.timelines, &key).await.ok()?;
                    }
                    Some(())
                },
                |_| Message::Noop,
            ));
        }
        Task::batch(tasks)
    }

    pub(crate) fn handle_compare_message(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            Message::ToggleCompare(room) => {
                if !self.compare.armed.remove(&room) {
                    self.compare.armed.insert(room);
                }
                Some(iced::widget::operation::focus(crate::view::input_id()))
            }
            Message::OpenCompare(i) => {
                if i < self.compare.list.len() {
                    self.compare.shown = i;
                    self.modal = Some(crate::state::Modal::Compare);
                }
                Some(self.load_compare_threads())
            }
            Message::CompareOpenThreads => {
                let Some(c) = self.compare.list.get(self.compare.shown).cloned() else { return Some(Task::none()) };
                let threads: Vec<String> = c.entries.iter().filter_map(|e| self.compare_answer(&c, e).thread).collect();
                let Some(first) = threads.first().cloned() else { return Some(Task::none()) };
                self.modal = None;
                self.windows.push(crate::panes::Window::new(Some(first.clone())));
                self.active_window = self.windows.len() - 1;
                self.layout_dirty = true;
                let mut tasks = vec![self.ensure_pane_view(first)];
                for key in threads.into_iter().skip(1) {
                    let w = self.windows.last_mut().expect("just pushed");
                    let last = crate::panes::layout_order(w.grid.layout()).last().copied();
                    if let Some(last) = last {
                        let _ = w.grid.split(iced::widget::pane_grid::Axis::Vertical, last, crate::panes::PaneSlot { key: Some(key.clone()) });
                    }
                    tasks.push(self.ensure_pane_view(key));
                }
                let p = self.window().focus;
                tasks.push(self.focus_pane(p));
                Some(Task::batch(tasks))
            }
            _ => None,
        }
    }

    /// Next to the "To" switch: send the next task to every agent here.
    pub(crate) fn view_compare_toggle<'a>(&self, key: &str) -> Option<Element<'a, Message>> {
        if !self.can_compare(key) {
            return None;
        }
        let armed = self.compare_armed(key);
        let colors = crate::theme::colors();
        let color = if armed { colors.accent } else { colors.text_dim };
        let label = if armed { format!("Asking all {}", self.agents_in(key).len()) } else { "Ask all".to_string() };
        let toggle = button(row![
            ui::icon(crate::icons::SVG_COLUMNS, 12.0, color),
            text(label).font(self.sidebar_font()).size(11).color(color),
        ].spacing(5).align_y(Alignment::Center))
        .on_press(Message::ToggleCompare(key.to_string()))
        .padding([2, 8])
        .style(move |_, status| button::Style {
            background: Some(crate::theme::with_alpha(colors.accent,
                if armed { 0.16 } else if matches!(status, button::Status::Hovered) { 0.08 } else { 0.0 }).into()),
            border: iced::Border { radius: 8.0.into(), color: crate::theme::with_alpha(color, 0.4), width: 1.0 },
            ..Default::default()
        });
        Some(ui::with_tooltip(toggle,
            "Send your next message to every agent here (each its own copy), then compare their answers side by side"))
    }

    /// The agent menu's way back to this room's latest comparison.
    pub(crate) fn last_comparison_in(&self, key: &str) -> Option<usize> {
        let room = real_room_id(key);
        self.compare.list.iter().rposition(|c| c.room == room)
    }

    pub(crate) fn view_compare_modal(&self) -> Element<'_, Message> {
        let colors = crate::theme::colors();
        let Some(c) = self.compare.list.get(self.compare.shown) else { return Space::new().into() };
        let room_name = self.rooms.iter().find(|r| r.room_id == c.room).map(|r| r.name.clone()).unwrap_or_default();
        let mut subtitle = format!("{} agents \u{00b7} #{room_name}", c.entries.len());
        if !c.skipped.is_empty() {
            let names: Vec<String> = c.skipped.iter().map(|a| self.agent_name(&c.room, a)).collect();
            subtitle.push_str(&format!(" \u{00b7} left out (over budget): {}", names.join(", ")));
        }
        let fs = self.msg_font_size;
        let mut columns = row![].spacing(COLUMN_GAP).height(Length::Shrink);
        let mut any_thread = false;
        for e in &c.entries {
            let a = self.compare_answer(c, e);
            any_thread |= a.thread.is_some();
            let (state_label, state_color) = match a.state {
                Some(s) => (s.label().to_string(), s.color()),
                None if !a.messages.is_empty() => ("answered".to_string(), colors.text_muted),
                None => ("waiting to start".to_string(), colors.text_dim),
            };
            let mut status = row![].spacing(6).align_y(Alignment::Center);
            if let Some(s) = a.state {
                status = status.push(state_dot(s, 7.0));
            }
            status = status.push(text(state_label).font(self.sidebar_font()).size(11).color(state_color)
                .wrapping(iced::widget::text::Wrapping::None));
            if !a.details.is_empty() {
                status = status.push(text(a.details.join(" \u{00b7} ")).font(self.sidebar_font()).size(11).color(colors.text_dim)
                    .wrapping(iced::widget::text::Wrapping::None));
            }
            let mut body = column![].spacing(10).width(Fill);
            if a.messages.is_empty() {
                let note = match a.state {
                    Some(RunState::Working) => "Working on it\u{2026}",
                    Some(RunState::NeedsYou) => "Waiting on you: open its thread to answer",
                    Some(RunState::Failed) => "It failed without an answer",
                    _ if a.thread.is_some() => "Loading its thread\u{2026}",
                    _ => "No answer yet",
                };
                body = body.push(text(note).font(self.sidebar_font()).size(12).color(colors.text_muted));
            }
            for m in &a.messages {
                let spans = match &m.formatted_body {
                    Some(html) => crate::html::html_to_spans_with_rooms(html, &self.rooms, fs),
                    None => crate::html::plain_text_to_spans(&m.body),
                };
                body = body.push(crate::selectable_text::selectable_rich_text(spans)
                    .font(self.msg_font())
                    .size(fs)
                    .wrapping(iced::widget::text::Wrapping::WordOrGlyph)
                    .width(Fill)
                    .on_link_click(Message::OpenUrl));
            }
            // Its thread; or, answered in the room, its reply there.
            let open = match (&a.thread, a.messages.first()) {
                (Some(k), _) => Some(Message::Batch(vec![Message::CloseModal, Message::OpenAt(k.clone(), crate::panes::Placement::Auto)])),
                (None, Some(m)) => Some(Message::Batch(vec![Message::CloseModal, Message::OpenMention(c.room.clone(), m.event_id.clone())])),
                (None, None) => None,
            };
            let head = row![
                ui::icon(crate::icons::SVG_BOT, 14.0, colors.accent),
                text(self.agent_name(&c.room, &e.agent)).font(self.sidebar_font()).size(13).color(colors.text_bright)
                    .wrapping(iced::widget::text::Wrapping::None),
                Space::new().width(Fill),
                ui::btn("Open", Tone::Ghost, open),
            ].spacing(8).align_y(Alignment::Center);
            columns = columns.push(
                ui::card(column![
                    head,
                    container(status).clip(true),
                    scrollable(container(body).padding(iced::Padding { right: 10.0, ..Default::default() }))
                        .height(Length::Fixed(ANSWER_HEIGHT)),
                ].spacing(8))
                .width(Length::Fixed(COLUMN_WIDTH)),
            );
        }
        let mut footer: Vec<Element<'_, Message>> = Vec::new();
        if self.compare.shown > 0 {
            footer.push(ui::btn("Earlier", Tone::Ghost, Some(Message::OpenCompare(self.compare.shown - 1))).into());
        }
        if self.compare.shown + 1 < self.compare.list.len() {
            footer.push(ui::btn("Later", Tone::Ghost, Some(Message::OpenCompare(self.compare.shown + 1))).into());
        }
        footer.push(ui::icon_btn(crate::icons::SVG_COLUMNS, "Open threads side by side", Tone::Primary,
            any_thread.then_some(Message::CompareOpenThreads)).into());
        let shown = c.entries.len().clamp(2, COLUMNS_SHOWN) as f32;
        let width = shown * COLUMN_WIDTH + (shown - 1.0) * COLUMN_GAP;
        column![
            ui::header("Compare answers", Some(subtitle)),
            container(text(crate::view::ellipsize(&c.prompt, 300)).font(self.msg_font()).size(13).color(colors.text_normal))
                .padding([6, 10])
                .width(Fill)
                .style(|_: &Theme| container::Style {
                    background: Some(crate::theme::colors().bg_mid.into()),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                }),
            scrollable(container(columns).padding(iced::Padding { bottom: 10.0, ..Default::default() }))
                .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::new())),
            ui::footer(footer),
        ]
        .spacing(ui::GAP)
        .width(Length::Fixed(width))
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agents_app() -> FluxChat {
        let mut app = crate::panes::flow_tests::app();
        app.user_id = "@me:x".into();
        app.prefs.agents = vec!["@hermes:x".into(), "@claw:x".into()];
        let member = |id: &str, name: &str| fluxchat_core::MemberInfo {
            user_id: id.into(), display_name: name.into(), avatar_url: None, power_level: 0, role: String::new(),
            presence: String::new(), status_msg: None, server_name: String::new(), last_active_secs: None,
        };
        app.members_by_room.insert("!r:x".into(), vec![member("@me:x", "Me"), member("@hermes:x", "Hermes"), member("@claw:x", "Claw")]);
        app.messages_by_room.insert("!r:x".into(), Vec::new());
        app
    }

    fn msg(id: &str, sender: &str, body: &str, secs: u64) -> MessageInfo {
        MessageInfo { event_id: id.into(), sender: sender.into(), body: body.into(), timestamp: secs, ..Default::default() }
    }

    #[tokio::test]
    async fn one_task_goes_to_each_agent_and_their_answers_line_up() {
        let mut app = agents_app();
        assert!(app.can_compare("!r:x"));
        assert!(!app.can_compare("thread::!r:x::$a"), "not from a thread");
        let _ = app.update(Message::ToggleCompare("!r:x".into()));
        assert!(app.compare_armed("!r:x"));
        let _ = app.send_comparison("!r:x".into(), "Which index is faster?".into());
        assert!(!app.compare_armed("!r:x"), "one task, then back to normal");
        assert!(matches!(app.modal, Some(crate::state::Modal::Compare)));
        let c = app.compare.list[0].clone();
        let bodies: Vec<&str> = c.entries.iter().map(|e| e.body.as_str()).collect();
        assert_eq!(bodies, ["@Hermes Which index is faster?", "@Claw Which index is faster?"]);

        // The requests land; Hermes answers in a thread, Claw in the room.
        let now = now_ms() / 1000;
        let mut to_hermes = msg("$h", "@me:x", "@Hermes Which index is faster?", now);
        to_hermes.thread_count = Some(1);
        let to_claw = msg("$c", "@me:x", "@Claw Which index is faster?", now);
        let mut claw_answer = msg("$ca", "@claw:x", "The B-tree one.", now + 5);
        claw_answer.reply_to_id = Some("$c".into());
        app.messages_by_room.insert("!r:x".into(), vec![to_hermes.clone(), to_claw.clone(), claw_answer]);
        let tk = thread_key("!r:x", "$h");
        let mut steps = msg("$s", "@hermes:x", "🔎 search indexes\n📊 run benchmark", now + 2);
        steps.thread_root = Some("$h".into());
        let mut hermes_answer = msg("$ha", "@hermes:x", "GIN, by 3x.", now + 9);
        hermes_answer.thread_root = Some("$h".into());
        app.messages_by_room.insert(tk.clone(), vec![to_hermes.clone(), steps, hermes_answer]);

        let hermes = app.compare_answer(&c, &c.entries[0]);
        assert_eq!(hermes.thread.as_deref(), Some(tk.as_str()));
        assert_eq!(hermes.messages.iter().map(|m| m.body.as_str()).collect::<Vec<_>>(), ["GIN, by 3x."], "step logs left out");
        let claw = app.compare_answer(&c, &c.entries[1]);
        assert_eq!(claw.messages.iter().map(|m| m.body.as_str()).collect::<Vec<_>>(), ["The B-tree one."]);
        assert!(claw.thread.is_none());
        // Its requests aren't followed into panes: the dialog has them.
        assert!(app.is_comparison_request("!r:x", &to_hermes));
        assert!(!app.is_comparison_request("!r:x", &msg("$x", "@me:x", "something else", now)));
        assert_eq!(app.last_comparison_in(&tk), Some(0));
    }

    #[tokio::test]
    async fn the_composer_sends_through_compare_when_armed() {
        let mut app = agents_app();
        app.selected_room_id = Some("!r:x".into());
        let _ = app.update(Message::ToggleCompare("!r:x".into()));
        // Commands still go to the one agent you're talking to.
        app.input_text = "!status".into();
        let _ = app.update(Message::SendMessage);
        assert!(app.compare.list.is_empty());
        app.input_text = "Summarise the outage".into();
        let _ = app.update(Message::SendMessage);
        assert_eq!(app.compare.list.len(), 1);
        assert!(app.input_text.is_empty());
        assert_eq!(app.compare.list[0].prompt, "Summarise the outage");
    }

    #[tokio::test]
    async fn agents_over_budget_are_left_out() {
        let mut app = agents_app();
        app.prefs.agent_budgets.insert("@claw:x".into(), crate::budgets::Budget { usd: 1.0, period: crate::budgets::Period::Month });
        app.run_history.push(crate::history::RunRecord {
            key: "!r:x".into(), room: "!r:x".into(), agent: "@claw:x".into(), title: "t".into(), state: "done".into(),
            started: now_ms() - 1000, ended: Some(now_ms()), steps: 0, model: None, cost: None, cost_usd: Some(2.0),
        });
        let _ = app.send_comparison("!r:x".into(), "go".into());
        let c = &app.compare.list[0];
        assert_eq!(c.entries.len(), 1);
        assert_eq!(c.skipped, vec!["@claw:x".to_string()]);
    }
}

