//! tmux-style panes and windows. Each window is an iced `pane_grid` whose
//! panes show a view key (room or thread) — or nothing yet, until a room is
//! picked. The focused pane of the active window is the main view
//! (`selected_room_id`): it gets the full timeline, panels and composer;
//! other panes get a lighter view with their own composer.
//!
//! Keys (tmux defaults, prefix Ctrl+B): `%` split side by side, `"` split
//! stacked, `x` close pane, `z` zoom, `o` next pane, arrows / h j k l move
//! focus, `c` new window, `n` / `p` next / previous window, `0`–`9` select
//! window, `&` close window, `?` help.

use std::sync::atomic::{AtomicBool, Ordering};

use iced::widget::pane_grid::{self, Axis, Direction};
use iced::Task;
use serde::{Deserialize, Serialize};

use crate::message::{Message, SyncEvent};
use crate::state::FluxChat;

/// What a pane shows.
#[derive(Debug, Clone, Default)]
pub struct PaneSlot {
    /// Room or thread key; `None` = waiting for a room to be picked.
    pub key: Option<String>,
}

/// A tmux "window": its own pane layout and focused pane.
#[derive(Debug)]
pub struct Window {
    pub grid: pane_grid::State<PaneSlot>,
    pub focus: pane_grid::Pane,
}

impl Window {
    pub fn new(key: Option<String>) -> Self {
        let (grid, focus) = pane_grid::State::new(PaneSlot { key });
        Self { grid, focus }
    }

    pub fn focused_key(&self) -> Option<&str> {
        self.grid.get(self.focus).and_then(|s| s.key.as_deref())
    }

    /// The pane showing `key`, if any.
    pub fn pane_with(&self, key: &str) -> Option<pane_grid::Pane> {
        self.grid.iter().find(|(_, s)| s.key.as_deref() == Some(key)).map(|(p, _)| *p)
    }
}

/// Armed after Ctrl+B until the next key (read by text-editor key bindings,
/// which would otherwise type the command key into the composer).
pub static PREFIX_ARMED: AtomicBool = AtomicBool::new(false);

pub fn prefix_armed() -> bool {
    PREFIX_ARMED.load(Ordering::Relaxed)
}

pub fn set_prefix_armed(armed: bool) {
    PREFIX_ARMED.store(armed, Ordering::Relaxed);
}

/// Saved layout (settings key `pane_layout`).
#[derive(Debug, Serialize, Deserialize)]
enum SavedNode {
    Split { vertical: bool, ratio: f32, a: Box<SavedNode>, b: Box<SavedNode> },
    Pane(Option<String>),
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedWindow {
    layout: SavedNode,
    focus: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedLayout {
    windows: Vec<SavedWindow>,
    active: usize,
}

const LAYOUT_KEY: &str = "pane_layout";

fn save_node(grid: &pane_grid::State<PaneSlot>, node: &pane_grid::Node) -> SavedNode {
    match node {
        pane_grid::Node::Split { axis, ratio, a, b, .. } => SavedNode::Split {
            vertical: *axis == Axis::Vertical,
            ratio: *ratio,
            a: Box::new(save_node(grid, a)),
            b: Box::new(save_node(grid, b)),
        },
        pane_grid::Node::Pane(p) => SavedNode::Pane(grid.get(*p).and_then(|s| s.key.clone())),
    }
}

fn to_config(node: SavedNode, keep: &dyn Fn(&str) -> bool) -> pane_grid::Configuration<PaneSlot> {
    match node {
        SavedNode::Split { vertical, ratio, a, b } => pane_grid::Configuration::Split {
            axis: if vertical { Axis::Vertical } else { Axis::Horizontal },
            ratio: ratio.clamp(0.1, 0.9),
            a: Box::new(to_config(*a, keep)),
            b: Box::new(to_config(*b, keep)),
        },
        SavedNode::Pane(key) => pane_grid::Configuration::Pane(PaneSlot { key: key.filter(|k| keep(k)) }),
    }
}

impl FluxChat {
    pub(crate) fn window(&self) -> &Window {
        &self.windows[self.active_window.min(self.windows.len() - 1)]
    }

    fn window_mut(&mut self) -> &mut Window {
        let i = self.active_window.min(self.windows.len() - 1);
        &mut self.windows[i]
    }

    /// Every view key shown in any pane of any window.
    pub(crate) fn pane_keys(&self) -> impl Iterator<Item = &str> {
        self.windows.iter().flat_map(|w| w.grid.iter().filter_map(|(_, s)| s.key.as_deref()))
    }

    /// Whether `key` is visible in some pane (focused or not).
    pub(crate) fn shown_in_pane(&self, key: &str) -> bool {
        self.pane_keys().any(|k| k == key)
    }

    /// Scrollable id of a pane's message list.
    pub(crate) fn pane_scroll_id(key: &str) -> iced::widget::Id {
        iced::widget::Id::from(format!("pane-scroll:{key}"))
    }

    /// Persist the window/pane layout.
    pub(crate) fn save_layout(&self) {
        let saved = SavedLayout {
            windows: self.windows.iter().map(|w| SavedWindow {
                layout: save_node(&w.grid, w.grid.layout()),
                focus: w.focused_key().map(str::to_string),
            }).collect(),
            active: self.active_window,
        };
        if let Ok(json) = serde_json::to_string(&saved) {
            let _ = self.core.db.set_setting(LAYOUT_KEY, &json);
        }
    }

    /// Rebuild the saved layout once the room list is known. Returns false
    /// when there's nothing saved. Panes whose room is gone come back empty.
    pub(crate) fn restore_layout(&mut self) -> Option<Task<Message>> {
        let json = self.core.db.get_setting(LAYOUT_KEY)?;
        let saved: SavedLayout = serde_json::from_str(&json).ok()?;
        if saved.windows.is_empty() {
            return None;
        }
        let rooms: std::collections::HashSet<String> = self.rooms.iter().map(|r| r.room_id.clone()).collect();
        let keep = |k: &str| rooms.contains(fluxchat_core::timeline::real_room_id(k));
        let mut windows = Vec::new();
        for w in saved.windows {
            let grid = pane_grid::State::with_configuration(to_config(w.layout, &keep));
            let focus = w.focus.as_deref()
                .and_then(|f| grid.iter().find(|(_, s)| s.key.as_deref() == Some(f)).map(|(p, _)| *p))
                .or_else(|| grid.iter().next().map(|(p, _)| *p));
            if let Some(focus) = focus {
                windows.push(Window { grid, focus });
            }
        }
        if windows.is_empty() {
            return None;
        }
        self.active_window = saved.active.min(windows.len() - 1);
        self.windows = windows;
        // Load every pane's view; the focused one becomes the main view.
        let keys: Vec<String> = self.pane_keys().map(str::to_string).collect();
        let mut tasks: Vec<Task<Message>> = keys.into_iter().map(|k| self.ensure_pane_view(k)).collect();
        if let Some(focus) = self.window().focused_key().map(str::to_string) {
            self.selected_room_id = None; // force SelectRoom to run fully
            tasks.push(self.update(Message::SelectRoom(focus)));
        }
        Some(Task::batch(tasks))
    }

    /// Make sure a pane's view is loading (timeline + members) and has a composer.
    pub(crate) fn ensure_pane_view(&mut self, key: String) -> Task<Message> {
        self.pane_inputs.entry(key.clone()).or_default();
        if self.messages_by_room.contains_key(&key) && self.members_by_room.contains_key(&key) {
            return Task::none();
        }
        let core = self.core.clone();
        Task::perform(
            async move {
                use futures_util::FutureExt;
                let client = core.get_client().await.ok()?;
                if !core.timelines.request_snapshot(&key).await {
                    // Boxed: these SDK futures nest deeply enough to overflow
                    // rustc's layout query depth in release builds.
                    let opened = fluxchat_core::timeline::get_or_create_for_key(&client, &core.timelines, &key).boxed().await;
                    if let Err(e) = opened {
                        tracing::warn!("Failed to open {key} in a pane: {e}");
                    }
                }
                let room = fluxchat_core::timeline::real_room_id(&key).to_string();
                let members = fluxchat_core::messages::get_room_members(&client, &room).boxed().await.ok()?;
                Some((key, members))
            },
            |r| match r {
                Some((key, members)) => Message::SyncEvent(SyncEvent::MembersUpdated(key, members)),
                None => Message::Noop,
            },
        )
    }

    /// Point the focused pane at `key` (from SelectRoom). If another pane in
    /// this window already shows it, focus that pane instead.
    pub(crate) fn focus_key_in_pane(&mut self, key: &str) {
        let w = self.window_mut();
        if w.focused_key() == Some(key) {
            return;
        }
        if let Some(p) = w.pane_with(key) {
            w.focus = p;
            return;
        }
        let focus = w.focus;
        if let Some(slot) = w.grid.get_mut(focus) {
            slot.key = Some(key.to_string());
        }
        self.layout_dirty = true;
    }

    /// Show `key` in a pane beside the focused one without moving focus:
    /// reuse a pane already showing a thread of the same room (for threads),
    /// else split the focused pane side by side.
    pub(crate) fn show_beside(&mut self, key: String) -> Task<Message> {
        let is_thread = key.starts_with("thread::");
        let room = fluxchat_core::timeline::real_room_id(&key).to_string();
        let w = self.window_mut();
        if w.pane_with(&key).is_none() {
            let reuse = is_thread.then(|| {
                w.grid.iter().find(|(p, s)| **p != w.focus && s.key.as_deref().is_some_and(|k| {
                    k.starts_with("thread::") && fluxchat_core::timeline::real_room_id(k) == room
                })).map(|(p, _)| *p)
            }).flatten();
            match reuse {
                Some(p) => {
                    if let Some(slot) = w.grid.get_mut(p) {
                        slot.key = Some(key.clone());
                    }
                }
                None => {
                    let focus = w.focus;
                    let _ = w.grid.split(Axis::Vertical, focus, PaneSlot { key: Some(key.clone()) });
                }
            }
        }
        self.layout_dirty = true;
        self.ensure_pane_view(key)
    }

    /// Handle pane / window messages. Returns Some(task) if handled.
    pub(crate) fn handle_pane_message(&mut self, message: Message) -> Option<Task<Message>> {
        let task = match message {
            Message::PaneClicked(pane) => {
                // Already focused: nothing to do (and no view rebuild that
                // could swallow the click on whatever was pressed).
                self.pending_pane_focus = (pane != self.window().focus).then_some(pane);
                Task::none()
            }
            Message::InView(key, inner) => {
                let focus = match self.window().pane_with(&key) {
                    Some(p) if self.selected_room_id.as_deref() != Some(key.as_str()) => {
                        self.pending_pane_focus = None;
                        self.focus_pane(p)
                    }
                    _ => Task::none(),
                };
                Task::batch([focus, self.update(*inner)])
            }
            Message::FillPane(pane, key) => {
                let w = self.window_mut();
                if w.grid.get(pane).is_none() {
                    return Some(Task::none());
                }
                if pane == w.focus {
                    // The focused pane is the main view.
                    return Some(self.update(Message::SelectRoom(key)));
                }
                if let Some(other) = w.pane_with(&key) {
                    // Already open in this window: go there instead of duplicating.
                    return Some(self.focus_pane(other));
                }
                if let Some(slot) = w.grid.get_mut(pane) {
                    slot.key = Some(key.clone());
                }
                self.layout_dirty = true;
                self.ensure_pane_view(key)
            }
            Message::PaneFocus(pane) => {
                self.pending_pane_focus = None;
                if pane == self.window().focus || self.window().grid.get(pane).is_none() {
                    Task::none()
                } else {
                    self.focus_pane(pane)
                }
            }
            Message::PaneResized(e) => {
                self.window_mut().grid.resize(e.split, e.ratio);
                self.layout_dirty = true;
                Task::none()
            }
            Message::PaneDragged(e) => {
                if let pane_grid::DragEvent::Dropped { pane, target } = e {
                    self.window_mut().grid.drop(pane, target);
                    self.layout_dirty = true;
                }
                Task::none()
            }
            Message::PaneSplit(axis) => {
                let w = self.window_mut();
                let focus = w.focus;
                if w.grid.maximized().is_some() {
                    w.grid.restore();
                }
                // The new pane is empty and shows its own room picker. Focus
                // stays on the room you're reading (moving it would shrink
                // that room to a compact pane and land you on a blank one).
                if w.grid.split(axis, focus, PaneSlot::default()).is_some() {
                    self.layout_dirty = true;
                }
                Task::none()
            }
            Message::PaneClose(target) => {
                let w = self.window_mut();
                let pane = target.unwrap_or(w.focus);
                if w.grid.len() > 1 {
                    let closed = w.grid.close(pane);
                    if let Some((slot, sibling)) = closed {
                        if pane == w.focus {
                            w.focus = sibling;
                        }
                        self.layout_dirty = true;
                        let release = self.release_view(slot.key);
                        let refocus = self.focus_pane(self.window().focus);
                        Task::batch([release, refocus])
                    } else {
                        Task::none()
                    }
                } else if self.windows.len() > 1 {
                    // Last pane of a window: close the window (like tmux).
                    self.update(Message::WindowClose)
                } else {
                    Task::none()
                }
            }
            Message::PaneZoom => {
                let w = self.window_mut();
                if w.grid.maximized().is_some() {
                    w.grid.restore();
                } else {
                    let focus = w.focus;
                    w.grid.maximize(focus);
                }
                Task::none()
            }
            Message::PaneFocusDirection(dir) => {
                let w = self.window();
                match w.grid.adjacent(w.focus, dir) {
                    Some(p) => self.focus_pane(p),
                    None => Task::none(),
                }
            }
            Message::PaneCycle => {
                let w = self.window();
                let panes: Vec<pane_grid::Pane> = layout_order(w.grid.layout());
                let idx = panes.iter().position(|p| *p == w.focus).unwrap_or(0);
                match panes.get((idx + 1) % panes.len().max(1)).copied() {
                    Some(p) => self.focus_pane(p),
                    None => Task::none(),
                }
            }
            Message::WindowNew => {
                self.windows.push(Window::new(None));
                self.active_window = self.windows.len() - 1;
                self.layout_dirty = true;
                // An empty window shows the welcome view (jump to room, …);
                // focusing its pane keeps the draft of the view we're leaving.
                let p = self.window().focus;
                self.focus_pane(p)
            }
            Message::WindowSelect(i) => {
                if i < self.windows.len() && i != self.active_window {
                    self.active_window = i;
                    self.layout_dirty = true;
                    let p = self.window().focus;
                    self.focus_pane(p)
                } else {
                    Task::none()
                }
            }
            Message::WindowNext => {
                let n = self.windows.len();
                self.update(Message::WindowSelect((self.active_window + 1) % n))
            }
            Message::WindowPrev => {
                let n = self.windows.len();
                self.update(Message::WindowSelect((self.active_window + n - 1) % n))
            }
            Message::WindowClose => {
                if self.windows.len() <= 1 {
                    return Some(Task::none());
                }
                let closed = self.windows.remove(self.active_window);
                self.active_window = self.active_window.min(self.windows.len() - 1);
                self.layout_dirty = true;
                // Refocus first: release_view keeps whatever is still the main view.
                let p = self.window().focus;
                let mut tasks = vec![self.focus_pane(p)];
                tasks.extend(closed.grid.iter().map(|(_, s)| s.key.clone()).collect::<Vec<_>>()
                    .into_iter().map(|k| self.release_view(k)));
                Task::batch(tasks)
            }
            Message::PaneInputAction(key, action) => {
                self.pane_inputs.entry(key).or_default().perform(action);
                Task::none()
            }
            Message::PaneSend(key) => {
                let Some(content) = self.pane_inputs.get_mut(&key) else { return Some(Task::none()) };
                let mut body = content.text().trim_end_matches('\n').to_string();
                if body.trim().is_empty() {
                    return Some(Task::none());
                }
                *content = iced::widget::text_editor::Content::new();
                if self.prefs.auto_replace_emoji {
                    body = crate::prefs::replace_emoticons(&body);
                }
                let markdown = self.prefs.use_markdown;
                let core = self.core.clone();
                let k = key.clone();
                Task::batch([
                    Task::perform(
                        async move {
                            let client = core.get_client().await.map_err(|e| e.to_string())?;
                            // Thread keys get the m.thread relation from their Timeline.
                            fluxchat_core::timeline::send_text(&client, &core.timelines, &k, &body, None, markdown)
                                .await.map_err(|e| e.to_string())
                        },
                        |r| match r {
                            Ok(()) => Message::Noop,
                            Err(e) => Message::ShowError(format!("Couldn't send: {e}")),
                        },
                    ),
                    iced::widget::operation::snap_to_end(Self::pane_scroll_id(&key)),
                ])
            }
            Message::TmuxPrefix => {
                set_prefix_armed(true);
                self.tmux_prefix = true;
                Task::none()
            }
            Message::TmuxKey(key) => {
                set_prefix_armed(false);
                self.tmux_prefix = false;
                match tmux_command(&key) {
                    Some(m) => self.update(m),
                    None => Task::none(),
                }
            }
            _ => return None,
        };
        Some(task)
    }

    /// Finish a click on an unfocused pane (on mouse release): focus it and
    /// put the cursor in its composer, so the click selects where you type.
    pub(crate) fn commit_pane_focus(&mut self, pane: pane_grid::Pane) -> Task<Message> {
        let w = self.window();
        if pane == w.focus || w.grid.get(pane).is_none() {
            return Task::none(); // closed or refocused by the click itself
        }
        let has_view = w.grid.get(pane).is_some_and(|s| s.key.is_some());
        let task = self.focus_pane(pane);
        if has_view && self.modal.is_none() && !self.quick_switcher_open {
            Task::batch([task, iced::widget::operation::focus(crate::view::input_id())])
        } else {
            task
        }
    }

    /// Focus a pane: it becomes the main view (or the welcome / picker if empty).
    fn focus_pane(&mut self, pane: pane_grid::Pane) -> Task<Message> {
        let w = self.window_mut();
        w.focus = pane;
        let key = w.grid.get(pane).and_then(|s| s.key.clone());
        self.layout_dirty = true;
        match key {
            Some(k) if self.selected_room_id.as_deref() != Some(k.as_str()) => self.update(Message::SelectRoom(k)),
            Some(_) => Task::none(),
            None => {
                // Keep the composer draft of the view we're leaving.
                if let Some(prev) = self.selected_room_id.take() {
                    let draft = std::mem::take(&mut self.input_text);
                    if !draft.trim().is_empty() {
                        self.drafts.insert(prev, draft);
                    }
                    self.input_content = iced::widget::text_editor::Content::new();
                }
                Task::none()
            }
        }
    }

    /// A pane stopped showing `key`: drop thread timelines nothing shows anymore.
    fn release_view(&mut self, key: Option<String>) -> Task<Message> {
        match key {
            Some(k) if k.starts_with("thread::")
                && !self.shown_in_pane(&k)
                && self.selected_room_id.as_deref() != Some(k.as_str()) =>
            {
                self.pane_inputs.remove(&k);
                self.close_thread_views(vec![k])
            }
            _ => Task::none(),
        }
    }
}

/// Panes in reading order (for `o` cycling).
fn layout_order(node: &pane_grid::Node) -> Vec<pane_grid::Pane> {
    match node {
        pane_grid::Node::Split { a, b, .. } => {
            let mut v = layout_order(a);
            v.extend(layout_order(b));
            v
        }
        pane_grid::Node::Pane(p) => vec![*p],
    }
}

/// Route a side pane's message-row message. Actions that work on "the
/// current room" (react, reply, edit, delete, pin, menus…) go through
/// `InView` so the pane is focused first; hover, selection, links and
/// media controls pass straight through, so hovering a pane never moves focus.
pub fn in_view(key: &str, message: Message) -> Message {
    match message {
        Message::ToggleReaction(..)
        | Message::StartReply(..)
        | Message::StartEdit(..)
        | Message::RetrySend(..)
        | Message::PinMessage(..)
        | Message::OpenEmojiReact(..)
        | Message::ToggleEmojiPicker
        | Message::EmojiSelect(..)
        | Message::DeleteMessage(..)
        | Message::JumpToEvent(..)
        | Message::OpenContextMenu(..)
        | Message::OpenModal(..) => Message::InView(key.to_string(), Box::new(message)),
        other => other,
    }
}

/// The command for a key pressed after the Ctrl+B prefix.
pub fn tmux_command(key: &str) -> Option<Message> {
    Some(match key {
        "%" => Message::PaneSplit(Axis::Vertical),
        "\"" => Message::PaneSplit(Axis::Horizontal),
        "x" => Message::PaneClose(None),
        "z" => Message::PaneZoom,
        "o" => Message::PaneCycle,
        "ArrowLeft" | "h" => Message::PaneFocusDirection(Direction::Left),
        "ArrowRight" | "l" => Message::PaneFocusDirection(Direction::Right),
        "ArrowUp" | "k" => Message::PaneFocusDirection(Direction::Up),
        "ArrowDown" | "j" => Message::PaneFocusDirection(Direction::Down),
        "c" => Message::WindowNew,
        "n" => Message::WindowNext,
        "p" => Message::WindowPrev,
        "&" => Message::WindowClose,
        "?" => Message::ShowTmuxHelp,
        d if d.len() == 1 && d.chars().all(|c| c.is_ascii_digit()) => {
            Message::WindowSelect(d.parse().ok()?)
        }
        _ => return None,
    })
}

/// Key name for `tmux_command` from an iced key (None = ignore, e.g. modifiers).
pub fn key_name(key: &iced::keyboard::Key) -> Option<String> {
    use iced::keyboard::{key::Named, Key};
    match key {
        Key::Character(c) => Some(c.to_string()),
        Key::Named(Named::ArrowLeft) => Some("ArrowLeft".into()),
        Key::Named(Named::ArrowRight) => Some("ArrowRight".into()),
        Key::Named(Named::ArrowUp) => Some("ArrowUp".into()),
        Key::Named(Named::ArrowDown) => Some("ArrowDown".into()),
        Key::Named(Named::Shift | Named::Control | Named::Alt | Named::Super) => None,
        Key::Named(Named::Escape) => Some("Escape".into()),
        _ => Some(String::new()),
    }
}

/// Help text for `?`.
pub const TMUX_HELP: &str = "Panes (Ctrl+B, then): % split side by side \u{00b7} \" split stacked \u{00b7} x close \u{00b7} z zoom \u{00b7} o next \u{00b7} arrows/hjkl move \u{00b7} c new window \u{00b7} n/p next/prev window \u{00b7} 0\u{2013}9 window \u{00b7} & close window";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_keys_map_to_commands() {
        assert!(matches!(tmux_command("%"), Some(Message::PaneSplit(Axis::Vertical))));
        assert!(matches!(tmux_command("\""), Some(Message::PaneSplit(Axis::Horizontal))));
        assert!(matches!(tmux_command("3"), Some(Message::WindowSelect(3))));
        assert!(matches!(tmux_command("h"), Some(Message::PaneFocusDirection(Direction::Left))));
        assert!(tmux_command("q").is_none());
    }

    #[test]
    fn layout_round_trips() {
        let (mut grid, first) = pane_grid::State::new(PaneSlot { key: Some("!a:x".into()) });
        let (second, _) = grid.split(Axis::Vertical, first, PaneSlot { key: Some("!b:x".into()) }).unwrap();
        grid.split(Axis::Horizontal, second, PaneSlot { key: None });
        let saved = save_node(&grid, grid.layout());
        let json = serde_json::to_string(&saved).unwrap();
        let back: SavedNode = serde_json::from_str(&json).unwrap();
        let restored = pane_grid::State::with_configuration(to_config(back, &|k| k != "!b:x"));
        assert_eq!(restored.len(), 3);
        let keys: Vec<Option<String>> = restored.iter().map(|(_, s)| s.key.clone()).collect();
        assert!(keys.contains(&Some("!a:x".into())));
        // A pane whose room is gone comes back empty.
        assert!(!keys.contains(&Some("!b:x".into())));
    }
}

/// Pane operations driven through the real `update()`, with no client.
#[cfg(test)]
mod flow_tests {
    use super::*;
    use crate::state::FluxChat;

    fn app() -> FluxChat {
        let dir = std::env::temp_dir().join(format!(
            "fluxchat-pane-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = fluxchat_core::db::AppDb::open(&dir.join("t.db")).unwrap();
        FluxChat::new(std::sync::Arc::new(fluxchat_core::state::AppState::new(db)))
    }

    fn keys(app: &FluxChat) -> Vec<Option<String>> {
        app.window().grid.iter().map(|(_, s)| s.key.clone()).collect()
    }

    fn pane_of(app: &FluxChat, key: Option<&str>) -> pane_grid::Pane {
        *app.window().grid.iter().find(|(_, s)| s.key.as_deref() == key).unwrap().0
    }

    /// Press + release on a pane's title-bar control, as iced delivers it:
    /// `PaneClicked` on press, the button's message on release, then the
    /// global left-release.
    fn click(app: &mut FluxChat, pane: pane_grid::Pane, msg: Message) {
        let _ = app.update(Message::PaneClicked(pane));
        let _ = app.update(msg);
        let _ = app.update(Message::SidebarDragEnd);
    }

    fn split_stacked_from_title_bar(app: &mut FluxChat) {
        let a = app.window().focus;
        click(app, a, Message::Batch(vec![Message::PaneFocus(a), Message::PaneSplit(Axis::Horizontal)]));
    }

    #[tokio::test]
    async fn split_keeps_focus_and_opens_no_palette() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        split_stacked_from_title_bar(&mut app);
        assert_eq!(app.window().grid.len(), 2);
        assert_eq!(app.window().focused_key(), Some("!a:x"));
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
        assert!(!app.quick_switcher_open);
    }

    #[tokio::test]
    async fn close_new_empty_pane_restores_room() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        split_stacked_from_title_bar(&mut app);
        let empty = pane_of(&app, None);
        click(&mut app, empty, Message::PaneClose(Some(empty)));
        assert_eq!(keys(&app), vec![Some("!a:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn ctrl_b_x_closes_focused_pane() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenInPane("!b:x".into()));
        let _ = app.update(Message::TmuxPrefix);
        let _ = app.update(Message::TmuxKey("x".into()));
        assert_eq!(keys(&app), vec![Some("!b:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
    }

    #[tokio::test]
    async fn picking_a_room_in_an_empty_pane_fills_that_pane() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        split_stacked_from_title_bar(&mut app);
        let empty = pane_of(&app, None);
        click(&mut app, empty, Message::FillPane(empty, "!b:x".into()));
        let mut k = keys(&app);
        k.sort();
        assert_eq!(k, vec![Some("!a:x".to_string()), Some("!b:x".to_string())]);
        // The click also focuses the pane it filled.
        assert_eq!(app.window().focus, empty);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
    }

    #[tokio::test]
    async fn closing_an_unfocused_pane_keeps_the_main_view() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenInPane("!b:x".into()));
        let pb = pane_of(&app, Some("!b:x"));
        click(&mut app, pb, Message::PaneClose(Some(pb)));
        assert_eq!(keys(&app), vec![Some("!a:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn closing_a_pane_from_its_title_bar_never_refocuses_it() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenInPane("!b:x".into()));
        let pb = pane_of(&app, Some("!b:x"));
        click(&mut app, pb, Message::PaneClose(Some(pb)));
        assert!(app.window().grid.get(app.window().focus).is_some());
        assert!(app.pending_pane_focus.is_none());
    }

    #[tokio::test]
    async fn new_window_keeps_draft_and_closing_it_returns() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        app.input_text = "half-written".into();
        let _ = app.update(Message::WindowNew);
        assert_eq!(app.windows.len(), 2);
        assert_eq!(app.selected_room_id, None);
        assert_eq!(app.drafts.get("!a:x").map(String::as_str), Some("half-written"));
        let _ = app.update(Message::WindowClose);
        assert_eq!(app.windows.len(), 1);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
        assert_eq!(app.input_text, "half-written");
    }

    #[tokio::test]
    async fn closing_last_pane_of_second_window_closes_the_window() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::WindowNew);
        let _ = app.update(Message::PaneClose(None));
        assert_eq!(app.windows.len(), 1);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn threads_open_beside_reuse_one_pane_and_close_cleanly() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t1".into()));
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        assert_eq!(app.window().grid.len(), 2);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"), "focus stays on the room");
        // A second thread of the same room reuses the thread pane.
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t2".into()));
        let t2 = fluxchat_core::timeline::thread_key("!a:x", "$t2");
        assert_eq!(app.window().grid.len(), 2);
        assert!(app.shown_in_pane(&t2) && !app.shown_in_pane(&t1));
        // Close it from its title bar.
        let pt = pane_of(&app, Some(t2.as_str()));
        click(&mut app, pt, Message::PaneClose(Some(pt)));
        assert_eq!(keys(&app), vec![Some("!a:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn actions_in_a_side_pane_apply_to_that_pane() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenInPane("!b:x".into()));
        let pb = pane_of(&app, Some("!b:x"));
        // Delete from b's hover bar: press in b, release on the button.
        let wrapped = in_view("!b:x", Message::DeleteMessage("$e".into()));
        click(&mut app, pb, wrapped);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
        assert!(app.pending_redactions.get("!b:x").is_some_and(|p| p.contains("$e")));
        assert!(!app.pending_redactions.contains_key("!a:x"));
        // Hover / links aren't wrapped, so they never move focus.
        assert!(matches!(in_view("!b:x", Message::OpenUrl("u".into())), Message::OpenUrl(_)));
    }
}
