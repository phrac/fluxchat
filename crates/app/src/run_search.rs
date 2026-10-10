//! Full-text search across agent run transcripts (Runs dialog).
//!
//! Each recorded run keeps its text: the messages of its thread (or, for a
//! request answered in the room, the room's messages from the request until
//! the next run there), as "Name: body" lines, capped at `TEXT_MAX_BYTES`.
//! Taken from the loaded view whenever it updates; a thread run that ends
//! without its thread loaded (background rooms) is fetched from the server
//! (`core::transcripts`), as are older runs without text the first time you
//! search. Stored encrypted (`AppDb::set_private`, one entry per run, written
//! on Tick), loaded in the background after sign-in.

use std::collections::{HashMap, HashSet};

use futures_util::StreamExt;
use iced::Task;

use fluxchat_core::timeline::parse_thread_key;
use fluxchat_core::MessageInfo;

use crate::history::RunRecord;
use crate::message::Message;
use crate::state::FluxChat;

/// Text kept per run: the start (the request) and the end (the answer).
const TEXT_MAX_BYTES: usize = 64 * 1024;
const TEXT_HEAD_BYTES: usize = 16 * 1024;
/// Threads fetched from the server at once.
const FETCH_CONCURRENCY: usize = 4;
/// Snippet shown under a matching run: characters around the match.
const SNIPPET_CHARS: usize = 90;

#[derive(Debug)]
struct RunText {
    text: String,
    /// For case-insensitive matching.
    lower: String,
}

impl RunText {
    fn new(text: String) -> Self {
        let lower = text.to_lowercase();
        RunText { text, lower }
    }
}

#[derive(Debug, Default)]
pub struct RunTexts {
    /// By run id (`text_id`).
    texts: HashMap<String, RunText>,
    /// Changed since the last write.
    dirty: HashSet<String>,
    /// Thread keys to fetch from the server, and those being fetched.
    wanted: Vec<String>,
    fetching: HashSet<String>,
    /// Older runs without text were queued (once per session).
    backfilled: bool,
}

/// A run's id: its key and start (a thread can hold several runs).
pub(crate) fn text_id(r: &RunRecord) -> String {
    format!("{}@{}", r.key, r.started)
}

fn storage_key(id: &str) -> String {
    format!("run_text:{id}")
}

/// Keep the head and the tail of a long text.
fn cap(text: String) -> String {
    if text.len() <= TEXT_MAX_BYTES {
        return text;
    }
    let head = text.floor_char_boundary(TEXT_HEAD_BYTES);
    let tail = text.ceil_char_boundary(text.len() - (TEXT_MAX_BYTES - TEXT_HEAD_BYTES));
    format!("{}\n…\n{}", &text[..head], &text[tail..])
}

/// "Name: body" lines of the messages sent in `[from, until)` (ms).
fn text_of(msgs: &[MessageInfo], from: u64, until: u64) -> String {
    let mut out = String::new();
    for m in msgs {
        let at = crate::agents::sent_ms(m);
        if at < from || at >= until || m.is_system() || m.event_id.starts_with('~') || m.body.trim().is_empty() {
            continue;
        }
        let name = if m.sender_name.is_empty() { m.sender.as_str() } else { m.sender_name.as_str() };
        out.push_str(name);
        out.push_str(": ");
        out.push_str(m.body.trim());
        out.push('\n');
    }
    cap(out)
}

impl FluxChat {
    /// After sign-in: read the stored texts in the background.
    pub(crate) fn load_run_texts(&self) -> Task<Message> {
        let ids: Vec<String> = self.run_history.iter().map(text_id).collect();
        if ids.is_empty() {
            return Task::none();
        }
        let core = self.core.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    ids.into_iter()
                        .filter_map(|id| {
                            let bytes = core.db.get_private(&storage_key(&id))?;
                            Some((id, String::from_utf8(bytes).ok()?))
                        })
                        .collect::<Vec<_>>()
                })
                .await
                .unwrap_or_default()
            },
            Message::RunTextsLoaded,
        )
    }

    pub(crate) fn run_texts_loaded(&mut self, texts: Vec<(String, String)>) {
        for (id, text) in texts {
            // Captured meanwhile: newer.
            self.run_texts.texts.entry(id).or_insert_with(|| RunText::new(text));
        }
    }

    fn set_run_text(&mut self, id: String, text: String) {
        if text.is_empty() || self.run_texts.texts.get(&id).is_some_and(|t| t.text == text) {
            return;
        }
        self.run_texts.texts.insert(id.clone(), RunText::new(text));
        self.run_texts.dirty.insert(id);
    }

    /// Runs recorded in view `key`, with where each one's messages end (the
    /// next run there starts).
    fn runs_in(&self, key: &str) -> Vec<(String, u64, u64)> {
        let mut starts: Vec<(String, u64)> = self.run_history.iter()
            .filter(|r| r.key == key)
            .map(|r| (text_id(r), r.started))
            .collect();
        starts.sort_by_key(|(_, s)| *s);
        let ends: Vec<u64> = starts.iter().skip(1).map(|(_, s)| *s).chain(std::iter::once(u64::MAX)).collect();
        starts.into_iter().zip(ends).map(|((id, s), e)| (id, s, e)).collect()
    }

    /// View `key`'s messages changed: update the texts of its runs.
    pub(crate) fn capture_run_texts(&mut self, key: &str) {
        let runs = self.runs_in(key);
        if runs.is_empty() {
            return;
        }
        let Some(msgs) = self.messages_by_room.get(key) else { return };
        // A thread's root is its first run's request: start from the top.
        let threaded = parse_thread_key(key).is_some();
        let texts: Vec<(String, String)> = runs.iter().enumerate()
            .map(|(i, (id, from, until))| {
                let from = if threaded && i == 0 { 0 } else { *from };
                (id.clone(), text_of(msgs, from, *until))
            })
            .collect();
        for (id, text) in texts {
            self.set_run_text(id, text);
        }
    }

    /// A thread run just ended: fetch its text if the thread isn't loaded.
    pub(crate) fn want_run_text(&mut self, key: &str) {
        if parse_thread_key(key).is_some() && !self.messages_by_room.contains_key(key) && !self.run_texts.wanted.iter().any(|k| k == key) {
            self.run_texts.wanted.push(key.to_string());
        }
    }

    /// First search: queue older thread runs that have no text.
    pub(crate) fn backfill_run_texts(&mut self) -> Task<Message> {
        if self.run_texts.backfilled {
            return Task::none();
        }
        self.run_texts.backfilled = true;
        let missing: Vec<String> = self.run_history.iter()
            .filter(|r| r.key.starts_with("thread::") && !self.run_texts.texts.contains_key(&text_id(r)))
            .map(|r| r.key.clone())
            .collect();
        for key in missing {
            if self.messages_by_room.contains_key(&key) {
                self.capture_run_texts(&key);
            } else if !self.run_texts.wanted.contains(&key) {
                self.run_texts.wanted.push(key);
            }
        }
        self.fetch_run_texts()
    }

    /// Fetch the wanted threads (on Tick and when a search starts).
    pub(crate) fn fetch_run_texts(&mut self) -> Task<Message> {
        let keys: Vec<String> = std::mem::take(&mut self.run_texts.wanted).into_iter()
            .filter(|k| self.run_texts.fetching.insert(k.clone()))
            .collect();
        if keys.is_empty() {
            return Task::none();
        }
        let core = self.core.clone();
        let fetches = futures_util::stream::iter(keys).map(move |key| {
            let core = core.clone();
            async move {
                let msgs = async {
                    let client = core.get_client().await.ok()?;
                    let (room, root) = parse_thread_key(&key)?;
                    fluxchat_core::transcripts::thread_messages(&client, room, root).await
                        .map_err(|e| tracing::warn!("Couldn't fetch the transcript of {key}: {e}"))
                        .ok()
                }.await;
                (key, msgs.unwrap_or_default())
            }
        }).buffer_unordered(FETCH_CONCURRENCY);
        Task::run(fetches, |(key, msgs)| Message::RunTextFetched(key, msgs))
    }

    /// A thread fetched from the server: the texts of its runs.
    pub(crate) fn run_text_fetched(&mut self, key: String, msgs: Vec<MessageInfo>) {
        self.run_texts.fetching.remove(&key);
        // Loaded since: the view's messages are fresher (edits applied).
        if self.messages_by_room.contains_key(&key) {
            self.capture_run_texts(&key);
            return;
        }
        let texts: Vec<(String, String)> = self.runs_in(&key).into_iter().enumerate()
            .map(|(i, (id, from, until))| (id, text_of(&msgs, if i == 0 { 0 } else { from }, until)))
            .collect();
        for (id, text) in texts {
            self.set_run_text(id, text);
        }
    }

    /// Write changed texts (Tick).
    pub(crate) fn flush_run_texts(&mut self) {
        for id in std::mem::take(&mut self.run_texts.dirty) {
            if let Some(t) = self.run_texts.texts.get(&id) {
                self.core.db.set_private(&storage_key(&id), t.text.as_bytes());
            }
        }
    }

    /// History dropped these runs: their texts go too.
    pub(crate) fn forget_run_texts(&mut self, gone: &[RunRecord]) {
        for r in gone {
            let id = text_id(r);
            self.run_texts.texts.remove(&id);
            self.run_texts.dirty.remove(&id);
            self.core.db.remove_private(&storage_key(&id));
        }
    }

    /// The run's text has every one of `words` (lowercase) — or, with
    /// `others`, every word not found in `others`.
    pub(crate) fn run_text_has(&self, r: &RunRecord, words: &[&str], others: &str) -> bool {
        let text = self.run_texts.texts.get(&text_id(r));
        words.iter().all(|w| others.contains(w) || text.is_some_and(|t| t.lower.contains(w)))
    }

    /// Where the first of `words` is in the run's text: a line around it.
    pub(crate) fn run_snippet(&self, r: &RunRecord, words: &[&str]) -> Option<String> {
        let t = self.run_texts.texts.get(&text_id(r))?;
        let at = words.iter().find_map(|w| t.lower.find(w))?;
        // Lowercasing can change byte lengths; then show the lowercase text.
        let src = if t.lower.len() == t.text.len() && t.text.is_char_boundary(at) { &t.text } else { &t.lower };
        let line_start = src[..at].rfind('\n').map_or(0, |i| i + 1);
        let line_end = src[at..].find('\n').map_or(src.len(), |i| at + i);
        let line = &src[line_start..line_end];
        let offset = src[line_start..at].chars().count();
        let skip = offset.saturating_sub(SNIPPET_CHARS / 3);
        let snippet: String = line.chars().skip(skip).take(SNIPPET_CHARS).collect();
        let more = line.chars().count() > skip + SNIPPET_CHARS;
        Some(format!("{}{}{}", if skip > 0 { "…" } else { "" }, snippet.trim(), if more { "…" } else { "" }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(id: &str, sender: &str, secs: u64, body: &str) -> MessageInfo {
        MessageInfo { event_id: id.into(), sender: sender.into(), sender_name: sender.trim_start_matches('@').split(':').next().unwrap_or_default().into(),
            timestamp: secs, body: body.into(), ..Default::default() }
    }

    fn record(key: &str, started_secs: u64) -> RunRecord {
        RunRecord {
            key: key.into(), room: "!r:x".into(), agent: "@bot:x".into(), title: "Deploy".into(),
            state: "done".into(), started: started_secs * 1000, ended: Some(started_secs * 1000 + 5000),
            steps: 0, model: None, cost: None, cost_usd: None, ..Default::default()
        }
    }

    #[tokio::test]
    async fn runs_are_found_by_what_was_said() {
        let mut app = crate::panes::flow_tests::app();
        let tk = "thread::!r:x::$ask";
        app.run_history = vec![record(tk, 100), record(tk, 200), record("!r:x", 50)];
        app.messages_by_room.insert(tk.into(), vec![
            message("$ask", "@me:x", 100, "Upgrade the Postgres cluster"),
            message("$a1", "@bot:x", 150, "Done: pg 17 is live on db-3"),
            message("$ask2", "@me:x", 200, "now rotate the TLS certs"),
            message("$a2", "@bot:x", 210, "Rotated certificates on the load balancer"),
        ]);
        app.capture_run_texts(tk);
        let (first, second) = (&app.run_history[0], &app.run_history[1]);
        assert!(app.run_text_has(first, &["postgres", "db-3"], ""));
        assert!(!app.run_text_has(first, &["certificates"], ""), "the next run's messages are its own");
        assert!(app.run_text_has(second, &["certificates"], ""));
        // A word in the title counts too: the rest must be in the text.
        assert!(app.run_text_has(second, &["deploy", "tls"], "deploy"));
        assert_eq!(app.run_snippet(first, &["db-3"]).as_deref(), Some("bot: Done: pg 17 is live on db-3"));
        // Searching goes through matching_history.
        let found: Vec<u64> = app.matching_history("load balancer").iter().map(|r| r.started).collect();
        assert_eq!(found, vec![200_000]);
        // Stored on flush, read back on load.
        app.flush_run_texts();
        assert!(app.run_texts.dirty.is_empty());
    }

    #[tokio::test]
    async fn a_thread_fetched_from_the_server_fills_its_runs() {
        let mut app = crate::panes::flow_tests::app();
        let tk = "thread::!r:x::$ask";
        app.run_history = vec![record(tk, 100)];
        app.want_run_text(tk);
        assert_eq!(app.run_texts.wanted, vec![tk.to_string()]);
        app.run_text_fetched(tk.into(), vec![message("$ask", "@me:x", 100, "Summarise the incident"), message("$a", "@bot:x", 120, "Root cause: a full disk")]);
        assert!(app.run_text_has(&app.run_history[0], &["full disk"], ""));
    }

    #[test]
    fn long_texts_keep_their_start_and_end() {
        let text = format!("REQUEST {}ANSWER", "x".repeat(200_000));
        let capped = cap(text);
        assert!(capped.len() <= TEXT_MAX_BYTES + 8);
        assert!(capped.starts_with("REQUEST") && capped.ends_with("ANSWER"));
    }
}
