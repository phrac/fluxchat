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

/// Where a room or thread opens (`open_at`). A plain click on a thread is
/// `Auto`; the right-click menus, middle-click and Cmd/Ctrl+click pick the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A thread beside its room in a pane of its own (a thread pane already
    /// open is never replaced), a room beside the focused pane (`show_beside`).
    Auto,
    /// Like `Auto`, but a thread reuses the room's open thread pane (and
    /// replaces a focused thread of that room): agent thread-follows, so a
    /// stream of agent sessions doesn't stack panes.
    Follow,
    /// A new pane to the right of / below the focused one.
    Right,
    Below,
    /// In the focused pane.
    Here,
    /// In a new window.
    NewWindow,
}

/// A tmux "window": its own pane layout and focused pane.
#[derive(Debug)]
pub struct Window {
    pub grid: pane_grid::State<PaneSlot>,
    pub focus: pane_grid::Pane,
    /// A "watch runs" window: agent runs that start get a pane in it.
    pub follow_runs: bool,
}

impl Window {
    pub fn new(key: Option<String>) -> Self {
        let (grid, focus) = pane_grid::State::new(PaneSlot { key });
        Self { grid, focus, follow_runs: false }
    }

    pub fn focused_key(&self) -> Option<&str> {
        self.grid.get(self.focus).and_then(|s| s.key.as_deref())
    }

    /// The pane showing `key`, if any.
    pub fn pane_with(&self, key: &str) -> Option<pane_grid::Pane> {
        self.grid.iter().find(|(_, s)| s.key.as_deref() == Some(key)).map(|(p, _)| *p)
    }

    /// The first empty pane in reading order.
    fn empty_pane(&self) -> Option<pane_grid::Pane> {
        layout_order(self.grid.layout()).into_iter()
            .find(|p| self.grid.get(*p).is_some_and(|s| s.key.is_none()))
    }

    /// Leave zoom unless `pane` is the zoomed one: whatever is about to be
    /// focused or shown must be on screen (tmux unzooms the same way).
    fn unzoom_for(&mut self, pane: pane_grid::Pane) {
        if self.grid.maximized().is_some_and(|m| m != pane) {
            self.grid.restore();
        }
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
    /// The focused pane is zoomed (zoom always follows focus).
    #[serde(default)]
    zoomed: bool,
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

    /// Whether `key` is on screen: in a pane of the active window.
    pub(crate) fn visible(&self, key: &str) -> bool {
        self.window().pane_with(key).is_some()
    }

    /// Whether any view of `room_id` (the room or one of its threads) is on screen.
    pub(crate) fn room_visible(&self, room_id: &str) -> bool {
        self.window().grid.iter().any(|(_, s)| {
            s.key.as_deref().is_some_and(|k| fluxchat_core::timeline::real_room_id(k) == room_id)
        })
    }

    /// Esc with nothing else to dismiss, in a thread pane: back to its room
    /// if the room is on screen (the pane stays open).
    pub(crate) fn escape_to_room(&mut self) -> Option<Task<Message>> {
        let key = self.window().focused_key()?;
        if !key.starts_with("thread::") {
            return None;
        }
        let room = fluxchat_core::timeline::real_room_id(key).to_string();
        let pane = self.window().pane_with(&room)?;
        Some(self.focus_pane(pane))
    }

    /// Whether `key` is visible in some pane (focused or not).
    pub(crate) fn shown_in_pane(&self, key: &str) -> bool {
        self.pane_keys().any(|k| k == key)
    }

    /// Scrollable id of a pane's message list.
    pub(crate) fn pane_scroll_id(key: &str) -> iced::widget::Id {
        iced::widget::Id::from(format!("pane-scroll:{key}"))
    }

    /// Scroll a side pane to its newest message. Its list is `anchor_bottom`,
    /// so offsets count from the bottom: `snap_to_end` would go to the *top*.
    pub(crate) fn pane_snap_bottom<T>(key: &str) -> Task<T> {
        iced::widget::operation::snap_to(Self::pane_scroll_id(key), iced::widget::scrollable::RelativeOffset::START)
    }

    /// Persist the window/pane layout.
    pub(crate) fn save_layout(&self) {
        let saved = SavedLayout {
            windows: self.windows.iter().map(|w| SavedWindow {
                layout: save_node(&w.grid, w.grid.layout()),
                focus: w.focused_key().map(str::to_string),
                zoomed: w.grid.maximized().is_some(),
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
            let mut grid = pane_grid::State::with_configuration(to_config(w.layout, &keep));
            let focus = w.focus.as_deref()
                .and_then(|f| grid.iter().find(|(_, s)| s.key.as_deref() == Some(f)).map(|(p, _)| *p))
                .or_else(|| layout_order(grid.layout()).first().copied());
            if let Some(focus) = focus {
                if w.zoomed && grid.len() > 1 {
                    grid.maximize(focus);
                }
                windows.push(Window { grid, focus, follow_runs: false });
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

    /// The window and pane showing `key`: the active window first, then the
    /// others in order.
    pub(crate) fn locate(&self, key: &str) -> Option<(usize, pane_grid::Pane)> {
        let active = self.active_window.min(self.windows.len() - 1);
        std::iter::once(active)
            .chain((0..self.windows.len()).filter(|&i| i != active))
            .find_map(|i| self.windows[i].pane_with(key).map(|p| (i, p)))
    }

    /// Point the focused pane at `key` (from SelectRoom). If a pane already
    /// shows it — in this window or another — focus that pane instead (a
    /// second copy of a room is never what a click means). If this window
    /// has an empty pane (just split, "Choose a room for this pane"), that's
    /// where a room picked from the list or switcher goes, not over the room
    /// you were reading.
    ///
    /// Returns the view the pane showed before, if it was replaced.
    pub(crate) fn focus_key_in_pane(&mut self, key: &str) -> Option<String> {
        if self.window().focused_key() == Some(key) {
            return None;
        }
        self.layout_dirty = true;
        if let Some((i, p)) = self.locate(key) {
            self.active_window = i;
            let w = self.window_mut();
            w.focus = p;
            w.unzoom_for(p);
            return None;
        }
        let w = self.window_mut();
        let target = match w.focused_key() {
            Some(_) => w.empty_pane().unwrap_or(w.focus),
            None => w.focus,
        };
        w.focus = target;
        w.unzoom_for(target);
        w.grid.get_mut(target).and_then(|slot| slot.key.replace(key.to_string()))
    }

    /// Open a room or thread view at `at`. One that's already open (in any
    /// window) is focused instead, wherever it was asked to open.
    pub(crate) fn open_at(&mut self, key: String, at: Placement) -> Task<Message> {
        if self.selected_room_id.as_deref() == Some(key.as_str()) {
            return Task::none();
        }
        if let Some((i, p)) = self.locate(&key) {
            if i != self.active_window {
                self.active_window = i;
                self.layout_dirty = true;
            }
            return self.focus_pane(p);
        }
        match at {
            Placement::NewWindow => {
                let window = self.update(Message::WindowNew);
                Task::batch([window, self.update(Message::SelectRoom(key))])
            }
            Placement::Here => self.replace_focused(key),
            Placement::Right | Placement::Below => {
                let axis = if at == Placement::Right { Axis::Vertical } else { Axis::Horizontal };
                let w = self.window_mut();
                let focus = w.focus;
                w.grid.restore(); // the new pane must be on screen
                let _ = w.grid.split(axis, focus, PaneSlot { key: Some(key.clone()) });
                self.layout_dirty = true;
                self.ensure_pane_view(key)
            }
            // A thread goes beside its room in a pane of its own (opened in
            // the focused pane first if its room isn't on screen); `Follow`
            // swaps the room's open thread pane instead (agent thread-follows,
            // so a stream of runs doesn't stack a pane per session). A room
            // beside the focused pane.
            Placement::Auto | Placement::Follow => {
                let follow = at == Placement::Follow;
                let room_id = fluxchat_core::timeline::real_room_id(&key).to_string();
                let select = if key.starts_with("thread::") && self.active_real_room() != Some(room_id.as_str()) {
                    self.update(Message::SelectRoom(room_id))
                } else {
                    Task::none()
                };
                Task::batch([select, self.show_beside(key, follow)])
            }
        }
    }

    /// A room / thread dropped on `pane`: an edge splits that side, the middle
    /// shows it in that pane. Either way it ends up focused. One that's
    /// already open is focused instead (as everywhere else).
    pub(crate) fn drop_on_pane(&mut self, key: String, pane: pane_grid::Pane, zone: crate::dnd::DropZone) -> Task<Message> {
        use crate::dnd::DropZone;
        if self.window().grid.get(pane).is_none() {
            return Task::none();
        }
        if self.locate(&key).is_some() {
            return self.open_at(key, Placement::Auto);
        }
        let w = self.window_mut();
        w.unzoom_for(pane);
        let target = match zone {
            DropZone::Center if pane == w.focus => return self.replace_focused(key),
            DropZone::Center => {
                let old = w.grid.get_mut(pane).and_then(|s| s.key.replace(key.clone()));
                self.layout_dirty = true;
                let release = self.release_view(old);
                return Task::batch([release, self.focus_pane(pane)]);
            }
            DropZone::Left | DropZone::Right => Axis::Vertical,
            DropZone::Top | DropZone::Bottom => Axis::Horizontal,
        };
        w.grid.restore();
        let Some((new, _)) = w.grid.split(target, pane, PaneSlot { key: Some(key) }) else {
            return Task::none();
        };
        // split() puts the new pane right / below; swap for left / top.
        if matches!(zone, DropZone::Left | DropZone::Top) {
            w.grid.swap(new, pane);
        }
        self.layout_dirty = true;
        self.focus_pane(new)
    }

    /// A room is gone (left, kicked, removed): close every pane showing it or
    /// one of its threads (a window's last pane is emptied instead), drop
    /// those thread views, and point the main view at whatever is focused now.
    pub(crate) fn forget_room_in_panes(&mut self, room_id: &str) -> Task<Message> {
        let of_room = |k: &str| fluxchat_core::timeline::real_room_id(k) == room_id;
        let mut gone: Vec<String> = Vec::new();
        for w in &mut self.windows {
            let panes: Vec<pane_grid::Pane> = w.grid.iter()
                .filter(|(_, s)| s.key.as_deref().is_some_and(of_room))
                .map(|(p, _)| *p)
                .collect();
            for p in panes {
                if w.grid.len() > 1 {
                    if let Some((slot, sibling)) = w.grid.close(p) {
                        if w.focus == p {
                            w.focus = sibling;
                        }
                        gone.extend(slot.key);
                    }
                } else if let Some(slot) = w.grid.get_mut(p) {
                    gone.extend(slot.key.take());
                }
            }
        }
        if gone.is_empty() && !self.selected_room_id.as_deref().is_some_and(of_room) {
            return Task::none();
        }
        self.layout_dirty = true;
        for k in &gone {
            self.pane_inputs.remove(k);
        }
        if self.selected_room_id.as_deref().is_some_and(of_room) {
            self.selected_room_id = None;
            self.input_text.clear();
            self.input_content = iced::widget::text_editor::Content::new();
        }
        let threads = self.close_thread_views(gone);
        let focus = self.window().focus;
        Task::batch([threads, self.focus_pane(focus)])
    }

    /// Show `key` in the focused pane — this pane, even when the window has
    /// an empty one (where a plain SelectRoom would put it).
    fn replace_focused(&mut self, key: String) -> Task<Message> {
        let w = self.window_mut();
        let focus = w.focus;
        let old = w.grid.get_mut(focus).and_then(|s| s.key.replace(key.clone()));
        self.layout_dirty = true;
        let select = self.update(Message::SelectRoom(key));
        Task::batch([select, self.release_view(old)])
    }

    /// Show `key` in a pane beside the focused one without moving focus:
    /// reuse an empty pane, else split the focused pane side by side. A
    /// thread never replaces a pane that already shows another thread — every
    /// thread gets a pane of its own. With `follow` (agent thread-follows,
    /// where a stream of sessions must not stack panes) a thread instead
    /// swaps the room's open thread pane (the one next to the focused pane
    /// first, so a room shown above its thread keeps its thread below it), or
    /// the focused pane when that is itself a thread of the room. Zoom is
    /// left so the result is on screen.
    pub(crate) fn show_beside(&mut self, key: String, follow: bool) -> Task<Message> {
        let is_thread = key.starts_with("thread::");
        let room = fluxchat_core::timeline::real_room_id(&key).to_string();
        let w = self.window_mut();
        let in_focused_thread = follow && w.focused_key().is_some_and(|k| {
            k.starts_with("thread::") && fluxchat_core::timeline::real_room_id(k) == room
        });
        if is_thread && in_focused_thread && w.pane_with(&key).is_none() {
            return self.replace_focused(key);
        }
        let mut replaced = None;
        if w.pane_with(&key).is_none() {
            let same_room_thread = |p: &pane_grid::Pane| *p != w.focus && w.grid.get(*p)
                .and_then(|s| s.key.as_deref())
                .is_some_and(|k| k.starts_with("thread::") && fluxchat_core::timeline::real_room_id(k) == room);
            let reuse = follow.then(|| {
                [Direction::Down, Direction::Right, Direction::Up, Direction::Left].into_iter()
                    .filter_map(|d| w.grid.adjacent(w.focus, d))
                    .find(&same_room_thread)
                    .or_else(|| w.grid.iter().map(|(p, _)| *p).find(&same_room_thread))
            }).flatten().or_else(|| w.empty_pane().filter(|p| *p != w.focus));
            w.grid.restore();
            match reuse {
                Some(p) => {
                    if let Some(slot) = w.grid.get_mut(p) {
                        replaced = slot.key.replace(key.clone());
                    }
                }
                None => {
                    let focus = w.focus;
                    let _ = w.grid.split(Axis::Vertical, focus, PaneSlot { key: Some(key.clone()) });
                }
            }
        }
        self.layout_dirty = true;
        Task::batch([self.ensure_pane_view(key), self.release_view(replaced)])
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
                if self.locate(&key).is_some() {
                    // Already open: go there instead of duplicating.
                    return Some(self.open_at(key, Placement::Auto));
                }
                // Picked for this pane: fill it and go there.
                let w = self.window_mut();
                if let Some(slot) = w.grid.get_mut(pane) {
                    slot.key = Some(key.clone());
                }
                self.layout_dirty = true;
                let load = self.ensure_pane_view(key);
                Task::batch([load, self.focus_pane(pane)])
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
                } else if w.grid.len() > 1 {
                    let focus = w.focus;
                    w.grid.maximize(focus);
                }
                self.layout_dirty = true;
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
            Message::DropOnPane(key, pane, zone) => self.drop_on_pane(key, pane, zone),
            Message::PaneSend(key) => {
                let Some(content) = self.pane_inputs.get(&key) else { return Some(Task::none()) };
                let mut body = content.text().trim_end_matches('\n').to_string();
                if body.trim().is_empty() {
                    return Some(Task::none());
                }
                // A task for an agent over its budget waits for Send anyway.
                if self.budget_blocks(&key, &body) {
                    self.budgets.hold = Some(key);
                    return Some(Task::none());
                }
                let Some(content) = self.pane_inputs.get_mut(&key) else { return Some(Task::none()) };
                *content = iced::widget::text_editor::Content::new();
                self.remember_prompt(&body);
                if let Some(text) = self.agent_outgoing(&key, &body) {
                    body = text;
                }
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
                    Self::pane_snap_bottom(&key),
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
    pub(crate) fn focus_pane(&mut self, pane: pane_grid::Pane) -> Task<Message> {
        let w = self.window_mut();
        w.focus = pane;
        w.unzoom_for(pane);
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
    pub(crate) fn release_view(&mut self, key: Option<String>) -> Task<Message> {
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
pub(crate) fn layout_order(node: &pane_grid::Node) -> Vec<pane_grid::Pane> {
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
/// current room" (react, reply, edit, delete, pin, open thread, menus…) go through
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
        | Message::OpenThread(..)
        | Message::OpenAt(..)
        | Message::ListClick(..)
        | Message::OpenReview(..)
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
pub(crate) mod flow_tests {
    use super::*;
    use crate::state::FluxChat;

    /// The app on a temp database, with no client (tasks aren't run).
    pub(crate) fn app() -> FluxChat {
        // A counter, not the clock: parallel tests can start in the same tick.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "fluxchat-pane-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        let _ = std::fs::remove_dir_all(&dir);
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
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Auto));
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
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Auto));
        let pb = pane_of(&app, Some("!b:x"));
        click(&mut app, pb, Message::PaneClose(Some(pb)));
        assert_eq!(keys(&app), vec![Some("!a:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn closing_a_pane_from_its_title_bar_never_refocuses_it() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Auto));
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
    async fn every_thread_gets_its_own_pane_and_they_close_cleanly() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t1".into()));
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        assert_eq!(app.window().grid.len(), 2);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"), "focus stays on the room");
        // A second thread of the same room opens in a pane of its own; the
        // first thread pane is never replaced.
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t2".into()));
        let t2 = fluxchat_core::timeline::thread_key("!a:x", "$t2");
        assert_eq!(app.window().grid.len(), 3, "a third pane, not a replacement");
        assert!(app.shown_in_pane(&t2) && app.shown_in_pane(&t1));
        // Close t2 from its title bar; t1 and the room remain.
        let pt = pane_of(&app, Some(t2.as_str()));
        click(&mut app, pt, Message::PaneClose(Some(pt)));
        let ks = keys(&app);
        assert_eq!(ks.len(), 2);
        assert!(ks.contains(&Some("!a:x".to_string())) && ks.contains(&Some(t1.clone())), "{ks:?}");
        assert!(!app.shown_in_pane(&t2));
    }

    #[tokio::test]
    async fn a_followed_thread_replaces_the_rooms_thread_pane() {
        // An agent thread-follow (Placement::Follow) keeps the old swap: one
        // thread pane per room, so a stream of runs doesn't stack panes.
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t1".into()));
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        let _ = app.update(Message::OpenAt(fluxchat_core::timeline::thread_key("!a:x", "$t2"), Placement::Follow));
        let t2 = fluxchat_core::timeline::thread_key("!a:x", "$t2");
        assert_eq!(app.window().grid.len(), 2, "reused, not split");
        assert!(app.shown_in_pane(&t2) && !app.shown_in_pane(&t1));
        assert!(!app.pane_inputs.contains_key(&t1), "the replaced thread is released");
    }

    #[tokio::test]
    async fn actions_in_a_side_pane_apply_to_that_pane() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Auto));
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

    #[tokio::test]
    async fn thread_opened_from_a_side_pane_splits_beside_that_pane() {
        // Left: room a over thread t1; right: room b. Focus on t1.
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Auto));
        let pa = pane_of(&app, Some("!a:x"));
        click(&mut app, pa, Message::PaneFocus(pa));
        split_stacked_from_title_bar(&mut app);
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        let below = pane_of(&app, None);
        let _ = app.update(Message::FillPane(below, t1.clone()));
        click(&mut app, below, Message::PaneFocus(below));
        assert_eq!(app.selected_room_id.as_deref(), Some(t1.as_str()));
        // A new thread's summary clicked in a's (unfocused) pane: the click
        // makes a the origin, and the thread gets a pane of its own beside it
        // — t1 below stays open.
        let t2 = fluxchat_core::timeline::thread_key("!a:x", "$t2");
        click(&mut app, pa, in_view("!a:x", Message::OpenThread("!a:x".into(), "$t2".into())));
        assert_eq!(app.window().grid.len(), 4, "a new pane, not a replacement");
        assert!(app.shown_in_pane(&t2) && app.shown_in_pane(&t1));
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"), "focus on the room it was opened from");
    }

    #[tokio::test]
    async fn a_thread_opened_from_the_sidebar_splits_beside_the_focused_thread() {
        // Room a over thread t1, focus on t1; open t2 from the sidebar.
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t1".into()));
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        let pt = pane_of(&app, Some(t1.as_str()));
        click(&mut app, pt, Message::PaneFocus(pt));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t2".into()));
        let t2 = fluxchat_core::timeline::thread_key("!a:x", "$t2");
        assert_eq!(app.window().grid.len(), 3, "a new pane, not a replacement");
        assert!(app.shown_in_pane(&t2) && app.shown_in_pane(&t1), "both threads stay open");
        assert_eq!(app.selected_room_id.as_deref(), Some(t1.as_str()), "focus stays where you were");
        assert!(app.window().pane_with("!a:x").is_some(), "the room pane stays");
        // A followed thread (agent run) still swaps the focused thread pane.
        let _ = app.update(Message::OpenAt(fluxchat_core::timeline::thread_key("!a:x", "$t3"), Placement::Follow));
        let t3 = fluxchat_core::timeline::thread_key("!a:x", "$t3");
        assert_eq!(pane_of(&app, Some(t3.as_str())), pt);
        assert!(!app.shown_in_pane(&t1), "the followed thread replaced the focused one");
    }

    #[tokio::test]
    async fn thread_roots_keep_their_summary_in_a_side_pane() {
        // Room a split beside a thread: a is now a side pane, not the main view.
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$root".into()));
        let pt = pane_of(&app, Some(fluxchat_core::timeline::thread_key("!a:x", "$root").as_str()));
        click(&mut app, pt, Message::PaneFocus(pt));
        assert_ne!(app.selected_room_id.as_deref(), Some("!a:x"));
        let root = fluxchat_core::MessageInfo { event_id: "$root".into(), thread_count: Some(3), ..Default::default() };
        let plain = fluxchat_core::MessageInfo { event_id: "$plain".into(), ..Default::default() };
        assert!(app.thread_summary_row("!a:x", &root).is_some());
        assert!(app.thread_summary_row("!a:x", &plain).is_none());
    }

    #[tokio::test]
    async fn thread_menu_opens_where_asked() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let pa = app.window().focus;
        let t = |r: &str| fluxchat_core::timeline::thread_key("!a:x", r);
        let at = |r: &str, p| Message::OpenAt(t(r), p);

        let _ = app.update(at("$below", Placement::Below));
        assert_eq!(app.window().grid.adjacent(pa, Direction::Down), Some(pane_of(&app, Some(t("$below").as_str()))));
        let _ = app.update(at("$right", Placement::Right));
        assert_eq!(app.window().grid.adjacent(pa, Direction::Right), Some(pane_of(&app, Some(t("$right").as_str()))));
        assert_eq!(app.window().grid.len(), 3, "explicit placement never reuses a thread pane");
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"), "focus stays where you were");

        // Already open: it's focused, not duplicated.
        let _ = app.update(at("$below", Placement::Right));
        assert_eq!(app.window().grid.len(), 3);
        assert_eq!(app.selected_room_id.as_deref(), Some(t("$below").as_str()));

        // Here: the focused pane shows it (the thread it replaces is released).
        let _ = app.update(at("$here", Placement::Here));
        assert_eq!(app.window().grid.len(), 3);
        assert_eq!(app.selected_room_id.as_deref(), Some(t("$here").as_str()));
        assert!(!app.shown_in_pane(&t("$below")));

        // New window: its only pane.
        let _ = app.update(at("$win", Placement::NewWindow));
        assert_eq!(app.windows.len(), 2);
        assert_eq!(keys(&app), vec![Some(t("$win"))]);
        assert_eq!(app.selected_room_id.as_deref(), Some(t("$win").as_str()));
    }

    #[tokio::test]
    async fn clicking_an_open_thread_goes_to_it() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t1".into()));
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t1".into()));
        assert_eq!(app.window().grid.len(), 2);
        assert_eq!(app.selected_room_id.as_deref(), Some(t1.as_str()));
    }

    #[tokio::test]
    async fn dropping_on_a_pane_edge_splits_that_side_and_the_middle_replaces() {
        use crate::dnd::DropZone;
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let pa = app.window().focus;
        let _ = app.update(Message::DropOnPane("!b:x".into(), pa, DropZone::Left));
        let pb = pane_of(&app, Some("!b:x"));
        assert_eq!(app.window().grid.adjacent(pa, Direction::Left), Some(pb));
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"), "the dropped room is focused");
        let _ = app.update(Message::DropOnPane("!c:x".into(), pa, DropZone::Bottom));
        assert_eq!(app.window().grid.adjacent(pa, Direction::Down), Some(pane_of(&app, Some("!c:x"))));
        // The middle of an unfocused pane: shown there (a's pane now holds d).
        let _ = app.update(Message::DropOnPane("!d:x".into(), pa, DropZone::Center));
        assert_eq!(pane_of(&app, Some("!d:x")), pa);
        assert!(!app.shown_in_pane("!a:x"));
        assert_eq!(app.selected_room_id.as_deref(), Some("!d:x"));
        // Something already open is focused, not duplicated.
        let _ = app.update(Message::DropOnPane("!b:x".into(), pa, DropZone::Right));
        assert_eq!(app.window().grid.len(), 3);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
    }

    #[tokio::test]
    async fn a_room_open_in_another_window_is_switched_to() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::WindowNew);
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        assert_eq!(app.active_window, 1);
        // Clicking a (from the room list, a notification, …) goes back to window 0.
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        assert_eq!(app.active_window, 0);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
        assert_eq!(app.windows[1].focused_key(), Some("!b:x"), "window 1 is left as it was");
        assert_eq!(app.pane_keys().filter(|k| *k == "!a:x").count(), 1);
    }

    #[tokio::test]
    async fn rooms_in_unfocused_panes_count_as_on_screen() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Right));
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
        assert!(app.visible("!b:x") && app.room_visible("!b:x"));
        let _ = app.update(Message::OpenThread("!c:x".into(), "$t".into()));
        assert!(app.room_visible("!c:x"), "a thread puts its room on screen");
        let _ = app.update(Message::WindowNew);
        assert!(!app.room_visible("!b:x"), "other windows aren't on screen");
    }

    #[tokio::test]
    async fn escape_in_a_thread_pane_goes_back_to_its_room() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t".into()));
        let t = fluxchat_core::timeline::thread_key("!a:x", "$t");
        let pt = pane_of(&app, Some(t.as_str()));
        click(&mut app, pt, Message::PaneFocus(pt));
        assert_eq!(app.selected_room_id.as_deref(), Some(t.as_str()));
        let _ = app.update(Message::Escape);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
        assert_eq!(app.window().grid.len(), 2, "the thread pane stays open");
        // In the room, Esc with nothing to dismiss does nothing.
        let _ = app.update(Message::Escape);
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn after_a_split_the_next_room_picked_fills_the_empty_pane() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        split_stacked_from_title_bar(&mut app);
        let empty = pane_of(&app, None);
        // From the room list (or the switcher): a stays, b goes in the new pane.
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        assert_eq!(pane_of(&app, Some("!b:x")), empty);
        assert!(app.shown_in_pane("!a:x"));
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
        // No empty pane left: the focused pane switches as usual.
        let _ = app.update(Message::SelectRoom("!c:x".into()));
        assert_eq!(pane_of(&app, Some("!c:x")), empty);
        assert!(app.shown_in_pane("!a:x"));
    }

    #[tokio::test]
    async fn zoom_never_hides_what_you_go_to() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Right));
        let pa = pane_of(&app, Some("!a:x"));
        let _ = app.update(Message::PaneZoom);
        assert_eq!(app.window().grid.maximized(), Some(pa));
        // Moving focus to a hidden pane unzooms (tmux does the same).
        let _ = app.update(Message::PaneFocusDirection(Direction::Right));
        assert_eq!(app.window().grid.maximized(), None);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
        // Opening something beside a zoomed pane unzooms so it's visible.
        let _ = app.update(Message::PaneZoom);
        let _ = app.update(Message::OpenThread("!b:x".into(), "$t".into()));
        assert_eq!(app.window().grid.maximized(), None);
        // A room picked into the zoomed pane itself keeps the zoom.
        let pb = app.window().focus;
        let _ = app.update(Message::PaneZoom);
        let _ = app.update(Message::OpenAt("!c:x".into(), Placement::Here));
        assert_eq!(app.window().grid.maximized(), Some(pb));
        // A lone pane can't be zoomed.
        let mut solo = super::flow_tests::app();
        let _ = solo.update(Message::SelectRoom("!a:x".into()));
        let _ = solo.update(Message::PaneZoom);
        assert_eq!(solo.window().grid.maximized(), None);
    }

    #[tokio::test]
    async fn zoom_is_restored_with_the_layout() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Right));
        let _ = app.update(Message::PaneZoom);
        app.save_layout();
        app.rooms = ["!a:x", "!b:x"].iter()
            .map(|id| fluxchat_core::RoomInfo { room_id: id.to_string(), ..Default::default() })
            .collect();
        app.windows = vec![Window::new(None)];
        let _ = app.restore_layout();
        assert_eq!(app.window().grid.len(), 2);
        assert_eq!(app.window().grid.maximized(), Some(app.window().focus));
        assert_eq!(app.selected_room_id.as_deref(), Some("!a:x"));
    }

    #[tokio::test]
    async fn leaving_a_room_closes_its_panes_everywhere() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Below));
        let _ = app.update(Message::OpenAt("!a:x".into(), Placement::Auto));
        let _ = app.update(Message::LeaveRoom("!a:x".into()));
        // a and its thread are gone; b is the main view, and nothing's stale.
        assert_eq!(keys(&app), vec![Some("!b:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
        assert_eq!(app.window().focused_key(), app.selected_room_id.as_deref());
    }

    #[tokio::test]
    async fn leaving_the_only_room_on_screen_moves_to_its_neighbour() {
        let mut app = app();
        app.rooms = ["!a:x", "!b:x"].iter()
            .map(|id| fluxchat_core::RoomInfo { room_id: id.to_string(), name: id.to_string(), ..Default::default() })
            .collect();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::LeaveRoom("!a:x".into()));
        assert_eq!(keys(&app), vec![Some("!b:x".to_string())]);
        assert_eq!(app.selected_room_id.as_deref(), Some("!b:x"));
    }

    #[tokio::test]
    async fn picking_a_room_open_in_another_window_goes_there() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::WindowNew);
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        split_stacked_from_title_bar(&mut app);
        let empty = pane_of(&app, None);
        let _ = app.update(Message::FillPane(empty, "!a:x".into()));
        assert_eq!(app.active_window, 0);
        assert_eq!(app.pane_keys().filter(|k| *k == "!a:x").count(), 1);
    }

    #[tokio::test]
    async fn a_thread_replaced_by_a_room_pick_is_released() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt(fluxchat_core::timeline::thread_key("!a:x", "$t"), Placement::Here));
        let t = fluxchat_core::timeline::thread_key("!a:x", "$t");
        assert!(app.pane_inputs.contains_key(&t) || app.selected_room_id.as_deref() == Some(t.as_str()));
        let _ = app.update(Message::SelectRoom("!b:x".into()));
        assert!(!app.shown_in_pane(&t));
        assert!(!app.pane_inputs.contains_key(&t));
    }

    #[tokio::test]
    async fn open_here_and_drops_go_exactly_where_asked_even_with_an_empty_pane() {
        use crate::dnd::DropZone;
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        split_stacked_from_title_bar(&mut app);
        let pa = pane_of(&app, Some("!a:x"));
        let empty = pane_of(&app, None);
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Here));
        assert_eq!(pane_of(&app, Some("!b:x")), pa);
        let _ = app.update(Message::DropOnPane("!c:x".into(), pa, DropZone::Center));
        assert_eq!(pane_of(&app, Some("!c:x")), pa);
        assert_eq!(pane_of(&app, None), empty, "the empty pane is still waiting");
        assert_eq!(app.selected_room_id.as_deref(), Some("!c:x"));
    }

    #[tokio::test]
    async fn a_followed_thread_reuses_the_thread_pane_next_to_its_room() {
        // Left: room a over thread t1; right: thread t9 of a too.
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let t9 = fluxchat_core::timeline::thread_key("!a:x", "$t9");
        let _ = app.update(Message::OpenAt(t9.clone(), Placement::Auto));
        let pa = pane_of(&app, Some("!a:x"));
        split_stacked_from_title_bar(&mut app);
        let t1 = fluxchat_core::timeline::thread_key("!a:x", "$t1");
        let below = pane_of(&app, None);
        let _ = app.update(Message::FillPane(below, t1.clone()));
        assert_eq!(app.selected_room_id.as_deref(), Some(t1.as_str()), "a picked pane is focused");
        click(&mut app, pa, Message::PaneFocus(pa));
        // A plain click gives t2 its own pane; the panes above/below stay.
        let _ = app.update(Message::OpenThread("!a:x".into(), "$t2".into()));
        let t2 = fluxchat_core::timeline::thread_key("!a:x", "$t2");
        assert_eq!(app.window().grid.len(), 4, "a new pane beside the room");
        assert!(app.shown_in_pane(&t2) && app.shown_in_pane(&t1) && app.shown_in_pane(&t9));
        // A followed thread swaps the thread pane next to the focused room.
        let _ = app.update(Message::OpenAt(fluxchat_core::timeline::thread_key("!a:x", "$t3"), Placement::Follow));
        let t3 = fluxchat_core::timeline::thread_key("!a:x", "$t3");
        assert_eq!(pane_of(&app, Some(t3.as_str())), below);
        assert!(!app.shown_in_pane(&t1), "the pane next to the room is the one replaced");
        assert!(app.shown_in_pane(&t9));
    }

    #[tokio::test]
    async fn closing_the_window_saves_the_layout_immediately() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::OpenAt("!b:x".into(), Placement::Auto));
        assert!(app.layout_dirty);
        let _ = app.update(Message::CloseRequested);
        assert!(!app.layout_dirty);
        let saved = app.core.db.get_setting(LAYOUT_KEY).expect("layout saved");
        assert!(saved.contains("!b:x"));
    }

    #[tokio::test]
    async fn pins_open_as_a_persisted_sidebar() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::TogglePins);
        assert!(app.pins_open);
        assert_eq!(app.core.db.get_setting("pins_open").as_deref(), Some("true"));
        let _ = app.update(Message::TogglePins);
        assert_eq!(app.core.db.get_setting("pins_open").as_deref(), Some("false"));
    }

    #[tokio::test]
    async fn back_pagination_is_one_page_at_a_time_and_stops() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        app.last_scroll_offset = 0.0;
        let _ = app.start_back_pagination("!a:x".into(), 5);
        assert!(app.paginating);
        assert_eq!(app.paginate_empty_budget, 5);
        // A second request while one is in flight is ignored.
        let _ = app.start_back_pagination("!a:x".into(), 1);
        assert_eq!(app.paginate_empty_budget, 5);

        // A page that showed nothing, still at the top: one more, budget down.
        let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(false)));
        assert!(app.paginating);
        assert_eq!(app.paginate_empty_budget, 4);

        // Scrolled away meanwhile: stop.
        app.last_scroll_offset = 500.0;
        let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(false)));
        assert!(!app.paginating);

        // Budget used up: stop even at the top.
        app.last_scroll_offset = 0.0;
        let _ = app.start_back_pagination("!a:x".into(), 0);
        let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(false)));
        assert!(!app.paginating);

        // Reached the start of history: never again for this view.
        let _ = app.start_back_pagination("!a:x".into(), 5);
        let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(true)));
        assert!(!app.paginating);
        let _ = app.start_back_pagination("!a:x".into(), 5);
        assert!(!app.paginating);
    }

    #[tokio::test]
    async fn an_underfilled_view_keeps_loading_until_it_fills() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        app.last_scroll_offset = 0.0;
        app.messages_underfilled = true;
        let _ = app.start_back_pagination("!a:x".into(), 0);
        // The page showed a row, but the view is still short: next page,
        // with a fresh budget.
        app.messages_by_room.insert("!a:x".into(), vec![fluxchat_core::MessageInfo {
            event_id: "$1".into(), body: "hi".into(), ..Default::default()
        }]);
        let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(false)));
        assert!(app.paginating);
        assert_eq!(app.paginate_empty_budget, crate::update::MAX_EMPTY_PAGES);

        // Pages of only hidden events (thread replies, edits) don't stop it
        // either, up to its own cap.
        for _ in 0..3 {
            let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(false)));
            assert!(app.paginating);
            assert_eq!(app.paginate_empty_budget, crate::update::MAX_EMPTY_PAGES);
        }
        app.underfill_pages = u8::MAX;
        let _ = app.update(Message::PaginateSettled("!a:x".into(), Some(false)));
        assert_eq!(app.paginate_empty_budget, crate::update::MAX_EMPTY_PAGES - 1);
        app.underfill_pages = 0;

        // A failed page is retried (after a delay) while it doesn't fill...
        let _ = app.update(Message::PaginateSettled("!a:x".into(), None));
        assert!(!app.paginating);
        let _ = app.update(Message::PaginateRetry("!a:x".into(), 2));
        assert!(app.paginating);
        assert_eq!(app.paginate_empty_budget, 2);

        // ...but not once it fills.
        let _ = app.update(Message::PaginateSettled("!a:x".into(), None));
        app.messages_underfilled = false;
        let _ = app.update(Message::PaginateRetry("!a:x".into(), 1));
        assert!(!app.paginating);
    }

    #[tokio::test]
    async fn scrolling_up_at_the_top_loads_more() {
        let mut app = app();
        let _ = app.update(Message::SelectRoom("!a:x".into()));
        let _ = app.update(Message::TimelineWheelUpAtTop);
        assert!(app.paginating);
        // Not again while that page is in flight.
        app.paginate_empty_budget = 0;
        let _ = app.update(Message::TimelineWheelUpAtTop);
        assert_eq!(app.paginate_empty_budget, 0);
    }

    #[tokio::test]
    async fn room_icon_upload_shows_progress_errors_and_the_new_icon() {
        let mut app = app();
        let _ = app.update(Message::RoomAvatarPicked("!a:x".into(), "image/png", vec![1, 2, 3]));
        assert!(app.room_avatar_status.as_deref().is_some_and(|s| s.starts_with("Uploading")));
        let _ = app.update(Message::RoomAvatarUploaded("!a:x".into(), Err("too big".into())));
        assert!(app.room_avatar_status.as_deref().is_some_and(|s| s.contains("too big")));
        let _ = app.update(Message::RoomAvatarUploaded("!a:x".into(), Ok(("mxc://x/new".into(), vec![1, 2, 3]))));
        assert!(app.room_avatar_status.is_none());
        // Shown from the uploaded bytes, without downloading it back.
        assert!(app.avatar_cache.contains_key("mxc://x/new"));
    }

    #[tokio::test]
    async fn update_check_result_arms_and_clears_the_chip() {
        let mut app = app();
        assert_eq!(app.update_available, None);
        // A failure stays silent: no chip, no state.
        let _ = app.update(Message::UpdateChecked(Err("HTTP 404".into())));
        assert_eq!(app.update_available, None);
        // A newer release arms the chip with its version.
        let _ = app.update(Message::UpdateChecked(Ok(Some("9.9.9".into()))));
        assert_eq!(app.update_available.as_deref(), Some("9.9.9"));
        // A later check finding nothing (repo downgraded, say) clears it.
        let _ = app.update(Message::UpdateChecked(Ok(None)));
        assert_eq!(app.update_available, None);
    }

    #[tokio::test]
    async fn room_list_collapses_and_remembers() {
        let mut app = app();
        let _ = app.update(Message::ToggleRoomList);
        assert!(app.room_list_collapsed);
        assert_eq!(app.core.db.get_setting("room_list_collapsed").as_deref(), Some("true"));
        let _ = app.update(Message::ToggleRoomList);
        assert!(!app.room_list_collapsed);
    }
}
