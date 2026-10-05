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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        let pos = self.run_history.iter().position(|r| r.key == key && r.started == run.since);
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
                    cost: None,
                });
                if self.run_history.len() > HISTORY_LIMIT {
                    let extra = self.run_history.len() - HISTORY_LIMIT;
                    self.run_history.drain(..extra);
                }
                true
            }
            None => false,
            Some(i) => {
                let runtime = self.agent_runtime(key).cloned();
                let fresh_title = key.starts_with("thread::").then(|| self.thread_title(key));
                let rec = &mut self.run_history[i];
                let was = (rec.state.clone(), rec.steps, rec.ended);
                rec.state = label;
                rec.steps = rec.steps.max(run.steps);
                if !live && rec.ended.is_none() {
                    rec.ended = Some(now_ms());
                    if let Some(rt) = runtime {
                        rec.model = rt.model;
                        rec.cost = rt.cost;
                    }
                }
                if let Some(t) = fresh_title.filter(|t| rec.title == "Thread" && t != "Thread") {
                    rec.title = t;
                }
                let rec = &self.run_history[i];
                was != (rec.state.clone(), rec.steps, rec.ended)
            }
        };
        if changed {
            self.save_run_history();
        }
    }

    /// History matching `query` (title, room, agent), newest first.
    pub(crate) fn matching_history(&self, query: &str) -> Vec<&RunRecord> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.run_history.iter().rev()
            .filter(|r| {
                let room = self.rooms.iter().find(|x| x.room_id == r.room).map(|x| x.name.as_str()).unwrap_or("");
                let hay = format!("{} {} {} {}", r.title, room, r.agent, r.state).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }

    /// Whether the request behind run `key` is just a command, no task
    /// (`!usage`; not `!goal ship it`).
    fn request_is_lone_command(&self, key: &str, run: &RunInfo) -> bool {
        let room = real_room_id(key);
        let body = match fluxchat_core::timeline::parse_thread_key(key) {
            Some((_, root)) => [key, room].iter()
                .filter_map(|k| self.messages_by_room.get(*k)?.iter().find(|m| m.event_id == root))
                .map(|m| m.body.clone())
                .next(),
            None => self.messages_by_room.get(key)
                .and_then(|msgs| msgs.iter().find(|m| m.sender == self.user_id && crate::agents::sent_ms(m) == run.since))
                .map(|m| m.body.clone()),
        };
        body.is_some_and(|b| is_lone_command(&b))
    }

    /// Close the history entry of a run that went quiet while "working"
    /// (`stalled`): ended at its last sign of life.
    pub(crate) fn end_stalled_run(&mut self, key: &str, run: &RunInfo) {
        let Some(i) = self.run_history.iter().position(|r| r.key == key && r.started == run.since && r.ended.is_none()) else {
            return;
        };
        let last = self.last_sign_of_life(key, run);
        let rec = &mut self.run_history[i];
        rec.state = STALLED.to_string();
        rec.ended = Some(last);
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
}
