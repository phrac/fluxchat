//! Agent run history: every run fluxchat saw live (so durations are real),
//! with its agent, room, title, start / end, steps, model and cost at the
//! end. Kept encrypted (`AppDb::set_private`), searchable in the Runs
//! dialog, and any run's thread can be exported as a Markdown transcript.

use serde::{Deserialize, Serialize};

use iced::Task;

use fluxchat_core::timeline::real_room_id;

use crate::agents::{RunInfo, RunState};
use crate::message::Message;
use crate::state::FluxChat;

const HISTORY_KEY: &str = "agent_run_history";
/// Runs kept (oldest dropped).
const HISTORY_LIMIT: usize = 500;
/// History state of a run that went quiet while working (`stalled`).
pub(crate) const STALLED: &str = "stalled";
/// History state of a run you stopped (the agent answered your stop command).
pub(crate) const STOPPED: &str = "stopped";

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RunRecord {
    /// Thread key (or room key) of the run.
    pub key: String,
    pub room: String,
    pub agent: String,
    pub title: String,
    /// "working" / "needs you" / "done" / "failed" / "stalled".
    pub state: String,
    /// ms since the epoch.
    pub started: u64,
    pub ended: Option<u64>,
    pub steps: usize,
    pub model: Option<String>,
    pub cost: Option<String>,
    /// The session's spend when last seen (agent protocol `cost_usd`, a
    /// running total) — what budgets add up (`budgets::spent`).
    #[serde(default)]
    pub cost_usd: Option<f64>,
    /// How the run ended, one line (agent protocol `result`, on the final
    /// status edit). Absent for agents that don't report it.
    #[serde(default)]
    pub result: Option<String>,
    /// The protocol's run id, when the agent gives one: one record per run,
    /// however its start was first dated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

impl RunRecord {
    /// "4m", "1h 20m" — start to end (or to now while going).
    pub fn duration(&self, now_ms: u64) -> String {
        let secs = self.ended.unwrap_or(now_ms).saturating_sub(self.started) / 1000;
        match secs {
            0..=59 => format!("{secs}s"),
            60..=3599 => format!("{}m", secs / 60),
            _ => format!("{}h {}m", secs / 3600, secs % 3600 / 60),
        }
    }
}

/// Whether record `r` (of the same key) is run `run`'s: by its start, or by
/// the protocol's run id when its start was dated differently.
fn same_run(r: &RunRecord, run: &RunInfo) -> bool {
    r.started == run.since || (r.run.is_some() && r.run == run.run_id)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl FluxChat {
    /// After sign-in (the private store is unlocked then).
    pub(crate) fn load_run_history(&mut self) {
        if let Some(bytes) = self.core.db.get_private(HISTORY_KEY) {
            self.run_history = serde_json::from_slice(&bytes).unwrap_or_default();
        }
        // Older builds recorded lone commands (`!usage`) as runs.
        let before = self.run_history.len();
        self.run_history.retain(|r| !is_lone_command(&r.title));
        if self.run_history.len() != before {
            self.save_run_history();
        }
    }

    fn save_run_history(&self) {
        if let Ok(json) = serde_json::to_vec(&self.run_history) {
            self.core.db.set_private(HISTORY_KEY, &json);
        }
    }

    /// Note a run's latest state (from `refresh_agent_state`): live runs are
    /// recorded, and a recorded run that ends gets its end, model and cost.
    /// Runs only ever seen finished (old history) aren't.
    pub(crate) fn record_run(&mut self, key: &str, run: &RunInfo) {
        let live = matches!(run.state, RunState::Working | RunState::NeedsYou);
        let label = run.state.label().to_string();
        let pos = self.run_history.iter().position(|r| r.key == key && r.started == run.since)
            // The same protocol run under another start: its status message
            // loaded a page before its request did, so it was first dated
            // from the status. Left as two records, the first stayed
            // "working" until it stalled, and its session-total cost was
            // counted as spend a second time.
            .or_else(|| run.run_id.as_ref().and_then(|id| self.run_history.iter()
                .position(|r| r.key == key && r.run.as_ref() == Some(id))));
        // Only an earlier start moves the record. Dated later than its record
        // (the request scrolled out of what's loaded, so the run is dated
        // from its status message again), it keeps its start — and its saved
        // transcript, which is filed under it: forgetting that on every look
        // left the run unsearchable.
        let moved = pos.is_some_and(|i| run.since < self.run_history[i].started);
        if let Some(i) = pos.filter(|_| moved) {
            // Its saved transcript is filed under the old start.
            let old = self.run_history[i].clone();
            self.forget_run_texts(std::slice::from_ref(&old));
            self.run_history[i].started = run.since;
        }
        let started = pos.map_or(run.since, |i| self.run_history[i].started);
        let changed = match pos {
            // A lone command (`!usage`, `!status`) gets the same reactions,
            // but it isn't a task.
            None if live && self.request_is_lone_command(key, run) => false,
            None if live => {
                let title = if key.starts_with("thread::") { self.thread_title(key) } else { "Request in the room".into() };
                self.run_history.push(RunRecord {
                    key: key.to_string(),
                    room: real_room_id(key).to_string(),
                    agent: run.agent.clone(),
                    title,
                    state: label,
                    started: run.since,
                    ended: None,
                    steps: run.steps,
                    model: None,
                    cost: run.cost_usd.map(crate::budgets::format_usd),
                    cost_usd: run.cost_usd,
                    result: run.result.clone(),
                    run: run.run_id.clone(),
                });
                if self.run_history.len() > HISTORY_LIMIT {
                    let extra = self.run_history.len() - HISTORY_LIMIT;
                    let gone: Vec<RunRecord> = self.run_history.drain(..extra).collect();
                    self.forget_run_texts(&gone);
                }
                true
            }
            None => false,
            // Ended stays ended against another live sighting: a working
            // reaction left behind (Hermes after a restart, or on a request
            // it was told to stop) reads as live again whenever the agent
            // types, and the record flipped back to "working" with the end
            // it already had — which nothing closed again.
            Some(i) if live && self.run_history[i].ended.is_some() => moved,
            Some(i) => {
                // Only what this run's agent said in this run's own view since
                // it started: the room's newest runtime may be another
                // agent's, or another session's running total.
                let runtime = self.run_runtime(key, &run.agent, started);
                let fresh_title = key.starts_with("thread::").then(|| self.thread_title(key));
                let rec = &mut self.run_history[i];
                let was = (rec.state.clone(), rec.steps, rec.ended, rec.cost_usd, rec.result.clone());
                // A run that outlasted the silence and then finished is no
                // longer "stalled": it takes its real outcome, and its end.
                let reopened = rec.state == STALLED;
                rec.state = label;
                rec.steps = rec.steps.max(run.steps);
                // The final edit's `result` (and nothing else overwrites it).
                if rec.result.is_none() {
                    rec.result = run.result.clone();
                }
                if let Some(c) = run.cost_usd {
                    let c = rec.cost_usd.map_or(c, |old| old.max(c));
                    rec.cost_usd = Some(c);
                    rec.cost = Some(crate::budgets::format_usd(c));
                }
                if rec.run.is_none() {
                    rec.run = run.run_id.clone();
                }
                if !live && (rec.ended.is_none() || reopened) {
                    // When it ended, where that's known — "now" is only when
                    // this app noticed, which can be hours later.
                    let now = now_ms();
                    rec.ended = Some(run.ended.unwrap_or(now).clamp(rec.started, now.max(rec.started)));
                    if let Some(rt) = runtime {
                        if rt.model.is_some() {
                            rec.model = rt.model;
                        }
                        if rec.cost_usd.is_none() {
                            rec.cost_usd = rt.cost.as_deref().and_then(crate::budgets::parse_usd);
                            rec.cost = rt.cost;
                        }
                    }
                }
                if let Some(t) = fresh_title.filter(|t| rec.title == "Thread" && t != "Thread") {
                    rec.title = t;
                }
                let rec = &self.run_history[i];
                moved || was != (rec.state.clone(), rec.steps, rec.ended, rec.cost_usd, rec.result.clone())
            }
        };
        if changed {
            self.save_run_history();
            self.check_budget(&run.agent);
            // Ended: keep its transcript searchable, loaded or not.
            if self.run_history.iter().any(|r| r.key == key && r.started == started && r.ended.is_some()) {
                self.want_run_text(key);
            }
        }
    }

    /// History matching `query` (title, room, agent, state, and what was
    /// said in the run), newest first.
    pub(crate) fn matching_history(&self, query: &str) -> Vec<&RunRecord> {
        let query = query.to_lowercase();
        let words: Vec<&str> = query.split_whitespace().collect();
        self.run_history.iter().rev()
            .filter(|r| {
                let room = self.rooms.iter().find(|x| x.room_id == r.room).map(|x| x.name.as_str()).unwrap_or("");
                let hay = format!("{} {} {} {} {}", r.title, room, r.agent, r.state,
                    r.result.clone().unwrap_or_default()).to_lowercase();
                self.run_text_has(r, &words, &hay)
            })
            .collect()
    }

    /// Whether the request behind run `key` is just a command, no task
    /// (`!usage`; not `!goal ship it`).
    fn request_is_lone_command(&self, key: &str, run: &RunInfo) -> bool {
        let room = real_room_id(key);
        let body = match fluxchat_core::timeline::parse_thread_key(key) {
            Some((_, root)) => {
                let session = || [key, room].into_iter()
                    .filter_map(|k| self.messages_by_room.get(k))
                    .flatten()
                    .filter(|m| m.event_id == root || m.thread_root.as_deref() == Some(root));
                // The request itself when the run is dated from one of yours
                // in the session (a follow-up), else the root. Going by the
                // root alone, a `!usage` sent in a session was recorded as a
                // run, and no task was in a thread that began with a command.
                session().find(|m| m.sender == self.user_id && crate::agents::sent_ms(m) == run.since)
                    .or_else(|| session().find(|m| m.event_id == root))
                    .map(|m| m.body.clone())
            }
            None => self.messages_by_room.get(key)
                .and_then(|msgs| msgs.iter().find(|m| m.sender == self.user_id && crate::agents::sent_ms(m) == run.since))
                .map(|m| m.body.clone()),
        };
        body.is_some_and(|b| is_lone_command(&b))
    }

    /// End run `run`'s open record as `state`, at `ended` (never before it
    /// started).
    pub(crate) fn end_run_record(&mut self, key: &str, run: &RunInfo, state: &str, ended: u64) {
        let Some(rec) = self.run_history.iter_mut().find(|r| r.key == key && r.ended.is_none() && same_run(r, run)) else {
            return;
        };
        rec.state = state.to_string();
        rec.ended = Some(ended.max(rec.started));
        self.save_run_history();
        // Ended: keep its transcript searchable, loaded or not.
        self.want_run_text(key);
    }

    /// End the record of a run that stopped being reported without a final
    /// state (see `refresh_agent_state`).
    pub(crate) fn close_run_record(&mut self, key: &str, run: &RunInfo) {
        self.end_run_record(key, run, RunState::Done.label(), now_ms());
    }

    /// Close the history entry of a run you stopped (`stopped`): ended when
    /// you said so.
    pub(crate) fn end_stopped_run(&mut self, key: &str, run: &RunInfo) {
        if let Some(at) = run.stopped {
            self.end_run_record(key, run, STOPPED, at);
        }
    }

    /// Close the history entry of a run that went quiet while "working"
    /// (`stalled`): ended at its last sign of life.
    pub(crate) fn end_stalled_run(&mut self, key: &str, run: &RunInfo) {
        if self.run_history.iter().any(|r| r.key == key && r.ended.is_none() && same_run(r, run)) {
            let last = self.last_sign_of_life(key, run);
            self.end_run_record(key, run, STALLED, last);
        }
    }

    /// When record `r`'s agent was last heard in its session (ms): its last
    /// word there since the run began, before the session's next run (as far
    /// as the session's messages are loaded), else the start.
    fn last_heard(&self, r: &RunRecord) -> u64 {
        let until = self.run_history.iter()
            .filter(|x| x.key == r.key && x.started > r.started)
            .map(|x| x.started)
            .min()
            .unwrap_or(u64::MAX);
        [r.key.as_str(), r.room.as_str()].into_iter()
            .filter_map(|view| Some((view, self.messages_by_room.get(view)?)))
            .filter_map(|(view, msgs)| crate::agents::last_word(
                msgs.iter().filter(|m| crate::agents::in_session(m, view, &r.key) && crate::agents::sent_ms(m) < until),
                &r.agent,
                r.started,
            ))
            .max()
            .unwrap_or(r.started)
    }

    /// Close the records left open with no run behind them (on Tick): the
    /// app quit mid-run and the request has scrolled out of what loads, the
    /// session's thread was closed on a follow-up the room can't see, the
    /// agent was unmarked… Nothing reports on such a run again, and a record
    /// with no end is in no list: the run was simply gone from History. Once
    /// its room has been read and its agent not heard for `STALE_RUN_MS`, it
    /// counts as stalled, at the agent's last word. (If the request turns up
    /// finished after all, `record_run` gives it its real outcome.)
    pub(crate) fn settle_lost_records(&mut self) {
        let now = now_ms();
        let stale = crate::agents::STALE_RUN_MS;
        let lost: Vec<(usize, u64)> = self.run_history.iter().enumerate()
            .filter(|(_, r)| r.ended.is_none() && now.saturating_sub(r.started) > stale)
            .filter(|(_, r)| self.messages_by_room.contains_key(&r.room))
            .filter(|(_, r)| !self.run_info(&r.key)
                .is_some_and(|run| matches!(run.state, RunState::Working | RunState::NeedsYou) && same_run(r, run)))
            .map(|(i, r)| (i, self.last_heard(r)))
            .filter(|(_, last)| now.saturating_sub(*last) > stale)
            .collect();
        if lost.is_empty() {
            return;
        }
        for (i, last) in lost {
            let rec = &mut self.run_history[i];
            rec.state = STALLED.to_string();
            rec.ended = Some(last.max(rec.started));
            let key = rec.key.clone();
            self.want_run_text(&key);
        }
        self.save_run_history();
    }

    /// The run's thread as Markdown: a header with the run, then each message.
    pub(crate) fn transcript(&self, key: &str) -> Option<String> {
        let msgs = self.messages_by_room.get(key)?;
        let rec = self.run_history.iter().rev().find(|r| r.key == key);
        let room = self.rooms.iter().find(|r| r.room_id == real_room_id(key)).map(|r| r.name.clone()).unwrap_or_default();
        let title = rec.map(|r| r.title.clone()).unwrap_or_else(|| self.thread_title(key));
        let stamp = |secs: u64| format!("{} {}", crate::view_messages::local_day_label(secs),
            crate::view_messages::format_clock(crate::view::local_time_of_day(secs), false, self.prefs.twelve_hour));
        let mut out = format!("# {title}\n\n");
        let mut meta = vec![format!("Room: {room}")];
        if let Some(r) = rec {
            meta.push(format!("Agent: {}", r.agent));
            meta.push(format!("Started: {}", stamp(r.started / 1000)));
            meta.push(format!("State: {} ({})", r.state, r.duration(now_ms())));
            if let Some(res) = r.result.as_ref().filter(|s| !s.trim().is_empty()) {
                meta.push(format!("Result: {res}"));
            }
            meta.extend(r.model.as_ref().map(|m| format!("Model: {m}")));
            meta.extend(r.cost.as_ref().map(|c| format!("Cost: {c}")));
        }
        out.push_str(&meta.join("  \n"));
        out.push_str("\n\n---\n");
        for m in msgs.iter().filter(|m| !m.is_system() && !m.event_id.starts_with('~')) {
            out.push_str(&format!("\n**{}** · {}\n\n{}\n", m.sender_name, stamp(m.timestamp), m.body.trim()));
        }
        Some(out)
    }

    /// Save a run's transcript (loading its thread first if needed).
    pub(crate) fn export_run(&mut self, key: String) -> Task<Message> {
        let Some(md) = self.transcript(&key) else {
            // Not loaded: open its timeline; the export runs when it arrives.
            self.pending_export = Some(key.clone());
            let core = self.core.clone();
            return Task::perform(
                async move {
                    let client = core.get_client().await.ok()?;
                    if !core.timelines.request_snapshot(&key).await {
                        let _ = fluxchat_core::timeline::get_or_create_for_key(&client, &core.timelines, &key).await;
                    }
                    Some(())
                },
                |_| Message::Noop,
            );
        };
        let name = crate::view::ellipsize(&self.thread_title(&key), 40)
            .chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect::<String>();
        Task::perform(
            async move {
                let handle = rfd::AsyncFileDialog::new()
                    .set_title("Export run transcript")
                    .set_file_name(format!("{}.md", name.trim()))
                    .save_file()
                    .await
                    .ok_or("cancelled")?;
                handle.write(md.as_bytes()).await.map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            },
            |r| match r {
                Ok(()) => Message::Noop,
                Err(e) if e == "cancelled" => Message::Noop,
                Err(e) => Message::ShowError(format!("Couldn't export: {e}")),
            },
        )
    }

    /// A view's messages arrived: finish an export waiting on them.
    pub(crate) fn export_if_pending(&mut self, key: &str) -> Task<Message> {
        if self.pending_export.as_deref() == Some(key) {
            self.pending_export = None;
            return self.export_run(key.to_string());
        }
        Task::none()
    }
}

/// `!usage`, `/status`: a command with nothing after it.
fn is_lone_command(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with('!') || t.starts_with('/')) && t.len() > 1 && !t.contains(char::is_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn only_runs_seen_live_are_recorded_and_they_get_an_end() {
        let mut app = crate::panes::flow_tests::app();
        let started = now_ms() - 90_000;
        let done = RunInfo { agent: "@bot:x".into(), ..RunInfo::new(RunState::Done, started) };
        app.record_run("thread::!r:x::$old", &done);
        assert!(app.run_history.is_empty(), "never seen live: not recorded");
        let working = RunInfo { agent: "@bot:x".into(), steps: 2, ..RunInfo::new(RunState::Working, started) };
        app.record_run("thread::!r:x::$a", &working);
        assert_eq!(app.run_history.len(), 1);
        assert_eq!(app.run_history[0].ended, None);
        app.record_run("thread::!r:x::$a", &RunInfo { state: RunState::Done, steps: 5, ..working });
        let rec = &app.run_history[0];
        assert_eq!((rec.state.as_str(), rec.steps), ("done", 5));
        assert!(rec.ended.is_some() && rec.duration(now_ms()).ends_with('m'));
        assert_eq!(app.matching_history("bot").len(), 1);
        assert!(app.matching_history("nothing like it").is_empty());
    }

    #[tokio::test]
    async fn the_final_result_lands_on_the_record_and_searches() {
        let mut app = crate::panes::flow_tests::app();
        let started = now_ms() - 90_000;
        let working = RunInfo { agent: "@bot:x".into(), ..RunInfo::new(RunState::Working, started) };
        app.record_run("thread::!r:x::$a", &working);
        // The result arrives with the final edit and never changes after.
        let done = RunInfo { state: RunState::Done, result: Some("Deployed v0.2.0".into()), ..working.clone() };
        app.record_run("thread::!r:x::$a", &done);
        assert_eq!(app.run_history[0].result.as_deref(), Some("Deployed v0.2.0"));
        app.record_run("thread::!r:x::$a", &RunInfo { result: Some("overwritten?".into()), ..done.clone() });
        assert_eq!(app.run_history[0].result.as_deref(), Some("Deployed v0.2.0"));
        // Searchable by the result's words.
        assert_eq!(app.matching_history("deployed").len(), 1);
    }

    #[tokio::test]
    async fn a_lone_command_isnt_a_run_but_a_command_with_a_task_is() {
        let mut app = crate::panes::flow_tests::app();
        app.user_id = "@me:x".into();
        let started = (now_ms() / 1000 - 5) * 1000;
        let ask = |id: &str, body: &str| fluxchat_core::MessageInfo {
            event_id: id.into(), sender: "@me:x".into(), body: body.into(), timestamp: started / 1000, ..Default::default()
        };
        app.messages_by_room.insert("!r:x".into(), vec![ask("$u", "!usage"), ask("$g", "!goal ship the release")]);
        let working = RunInfo { agent: "@bot:x".into(), ..RunInfo::new(RunState::Working, started) };
        app.record_run("thread::!r:x::$u", &working);
        assert!(app.run_history.is_empty());
        app.record_run("thread::!r:x::$g", &working);
        assert_eq!(app.run_history.len(), 1);
    }

    #[tokio::test]
    async fn a_protocol_run_dated_from_its_status_then_its_request_is_one_record() {
        let mut app = crate::panes::flow_tests::app();
        let now = now_ms();
        let key = "thread::!r:x::$root";
        // Its status message loaded a page before its request did…
        let from_status = RunInfo {
            agent: "@bot:x".into(), run_id: Some("$req".into()), cost_usd: Some(3.6),
            ..RunInfo::new(RunState::Working, now - 30_000)
        };
        app.record_run(key, &from_status);
        // …then the request: the same run, now dated from when it was asked.
        let from_request = RunInfo { since: now - 90_000, ..from_status.clone() };
        app.record_run(key, &from_request);
        assert_eq!(app.run_history.len(), 1, "one run, one record");
        assert_eq!(app.run_history[0].started, now - 90_000);
        // It ends as that one record, and spend counts its total once.
        app.record_run(key, &RunInfo { state: RunState::Done, ..from_request.clone() });
        assert_eq!(app.run_history.len(), 1);
        assert_eq!(app.run_history[0].state, "done");
        assert_eq!(app.spent("@bot:x", 0), 3.6);
        // Another run in the same session is another record.
        app.record_run(key, &RunInfo { run_id: Some("$next".into()), ..RunInfo::new(RunState::Working, now - 5000) });
        assert_eq!(app.run_history.len(), 2);
    }

    #[tokio::test]
    async fn a_run_dated_later_than_its_record_keeps_its_start_and_its_transcript() {
        let mut app = crate::panes::flow_tests::app();
        let now = now_ms() / 1000 * 1000;
        let key = "thread::!r:x::$root";
        let from_request = RunInfo {
            agent: "@bot:x".into(), run_id: Some("$req".into()),
            ..RunInfo::new(RunState::Working, now - 90_000)
        };
        app.record_run(key, &from_request);
        app.run_text_fetched(key.into(), vec![fluxchat_core::MessageInfo {
            event_id: "$req".into(), sender: "@me:x".into(), body: "Upgrade the Postgres cluster".into(),
            timestamp: (now - 90_000) / 1000, ..Default::default()
        }]);
        assert!(app.run_text_has(&app.run_history[0], &["postgres"], ""));
        // The request scrolls out of what's loaded (a restart, a gap in
        // sync): the run is dated from its status message again, on every look.
        let from_status = RunInfo { since: now - 30_000, ..from_request.clone() };
        for _ in 0..3 {
            app.record_run(key, &from_status);
        }
        assert_eq!(app.run_history.len(), 1);
        assert_eq!(app.run_history[0].started, now - 90_000);
        assert!(app.run_text_has(&app.run_history[0], &["postgres"], ""), "still found by what was said");
    }

    #[tokio::test]
    async fn an_ended_record_stays_ended_until_the_run_really_ends() {
        let mut app = crate::panes::flow_tests::app();
        let now = now_ms();
        let key = "thread::!r:x::$a";
        let started = now - 5 * 60 * 60 * 1000;
        let working = RunInfo { agent: "@bot:x".into(), ..RunInfo::new(RunState::Working, started) };
        app.record_run(key, &working);
        // Quiet for hours: stalled, at its last sign of life.
        app.end_stalled_run(key, &working);
        assert_eq!((app.run_history[0].state.as_str(), app.run_history[0].ended), (STALLED, Some(started)));
        // Its 👀 is still on the request, and the agent types (in another
        // session): live for a moment — not a "working" record with an end.
        app.record_run(key, &working);
        assert_eq!((app.run_history[0].state.as_str(), app.run_history[0].ended), (STALLED, Some(started)));
        // It finishes after all: its real outcome, and its real end.
        let ended = now - 60_000;
        app.record_run(key, &RunInfo { state: RunState::Done, ended: Some(ended), ..working.clone() });
        assert_eq!((app.run_history[0].state.as_str(), app.run_history[0].ended), ("done", Some(ended)));
        // A run you stopped keeps its 👀 too: no new life in that either.
        let key = "thread::!r:x::$b";
        app.record_run(key, &working);
        app.end_stopped_run(key, &RunInfo { stopped: Some(started + 60_000), ..working.clone() });
        app.record_run(key, &working);
        assert_eq!((app.run_history[1].state.as_str(), app.run_history[1].ended), (STOPPED, Some(started + 60_000)));
    }

    #[tokio::test]
    async fn a_command_sent_in_a_session_isnt_a_run_and_a_task_after_a_command_is() {
        let mut app = crate::panes::flow_tests::app();
        app.user_id = "@me:x".into();
        let t0 = (now_ms() / 1000 - 60) * 1000;
        let say = |id: &str, body: &str, at: u64, root: Option<&str>| fluxchat_core::MessageInfo {
            event_id: id.into(), sender: "@me:x".into(), body: body.into(), timestamp: at / 1000,
            thread_root: root.map(str::to_string), ..Default::default()
        };
        let run = |at: u64| RunInfo { agent: "@bot:x".into(), ..RunInfo::new(RunState::Working, at) };
        // A session, and a `!usage` sent in it (the chip menu's "Usage and cost").
        app.messages_by_room.insert("!r:x".into(), vec![
            say("$task", "ship the release", t0, None),
            say("$usage", "!usage", t0 + 20_000, Some("$task")),
        ]);
        app.record_run("thread::!r:x::$task", &run(t0 + 20_000));
        assert!(app.run_history.is_empty(), "a command with nothing after it isn't a task");
        app.record_run("thread::!r:x::$task", &run(t0));
        assert_eq!(app.run_history.len(), 1);
        // A thread that began with a command: a task sent in it is a run.
        app.messages_by_room.insert("!r:x".into(), vec![
            say("$new", "!new", t0, None),
            say("$fix", "fix the build", t0 + 30_000, Some("$new")),
        ]);
        app.record_run("thread::!r:x::$new", &run(t0));
        assert_eq!(app.run_history.len(), 1);
        app.record_run("thread::!r:x::$new", &run(t0 + 30_000));
        assert_eq!(app.run_history.len(), 2);
    }

    #[tokio::test]
    async fn a_record_with_no_run_behind_it_ends_as_stalled() {
        let mut app = crate::panes::flow_tests::app();
        let now = now_ms();
        let hour = 60 * 60 * 1000;
        let open = |key: &str, started: u64| RunRecord {
            key: key.into(), room: "!r:x".into(), agent: "@bot:x".into(), title: "t".into(),
            state: "working".into(), started, ..Default::default()
        };
        // Left open when the app quit; the request is further back than loads.
        app.run_history = vec![open("thread::!r:x::$ask", now - 5 * hour), open("thread::!r:x::$fresh", now - 10 * 60_000)];
        // Its room hasn't been read yet: nothing to go by.
        app.settle_lost_records();
        assert!(app.run_history.iter().all(|r| r.ended.is_none()));
        // Read: the agent's last word in that session was four hours ago.
        let word = fluxchat_core::MessageInfo {
            event_id: "$w".into(), sender: "@bot:x".into(), body: "halfway".into(), timestamp: (now - 4 * hour) / 1000,
            thread_root: Some("$ask".into()), ..Default::default()
        };
        let ended = crate::agents::sent_ms(&word);
        app.messages_by_room.insert("!r:x".into(), vec![word]);
        app.settle_lost_records();
        assert_eq!((app.run_history[0].state.as_str(), app.run_history[0].ended), (STALLED, Some(ended)));
        assert_eq!(app.run_history[1].ended, None, "ten minutes in isn't lost");
        assert_eq!(app.matching_history("").iter().filter(|r| r.ended.is_some()).count(), 1, "and it's in History now");

        // A run still going — quiet for hours, but typing — isn't lost.
        let mut app = crate::panes::flow_tests::app();
        app.user_id = "@me:x".into();
        app.prefs.agents = vec!["@bot:x".into()];
        app.rooms = vec![fluxchat_core::RoomInfo { room_id: "!r:x".into(), name: "r".into(), is_direct: true, ..Default::default() }];
        app.messages_by_room.insert("!r:x".into(), vec![fluxchat_core::MessageInfo {
            event_id: "$ask".into(), sender: "@me:x".into(), body: "go".into(), timestamp: (now - 5 * hour) / 1000,
            reactions: vec![fluxchat_core::ReactionInfo { emoji: "\u{1f440}".into(), count: 1, users: vec!["@bot:x".into()] }],
            ..Default::default()
        }]);
        app.note_agent_typing("!r:x", &["@bot:x".to_string()]);
        app.refresh_agent_state("!r:x");
        assert_eq!(app.run_history.len(), 1);
        app.settle_lost_records();
        assert_eq!(app.run_history[0].ended, None);
    }

    #[tokio::test]
    async fn a_run_ended_when_it_ended_not_when_this_app_noticed() {
        let mut app = crate::panes::flow_tests::app();
        let started = now_ms() - 10 * 60 * 60 * 1000;
        let working = RunInfo { agent: "@bot:x".into(), ..RunInfo::new(RunState::Working, started) };
        app.record_run("thread::!r:x::$a", &working);
        // Three minutes of work, noticed ten hours later.
        app.record_run("thread::!r:x::$a", &RunInfo { state: RunState::Done, ended: Some(started + 180_000), ..working.clone() });
        assert_eq!(app.run_history[0].ended, Some(started + 180_000));
        assert_eq!(app.run_history[0].duration(now_ms()), "3m");
        // An end before the start (clocks) or after now is clamped.
        app.record_run("thread::!r:x::$b", &working);
        app.record_run("thread::!r:x::$b", &RunInfo { state: RunState::Done, ended: Some(started - 5000), ..working.clone() });
        assert_eq!(app.run_history[1].ended, Some(started));
    }
}
