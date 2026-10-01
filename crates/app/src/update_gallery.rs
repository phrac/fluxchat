//! Media gallery: state transitions (view in `view_gallery.rs`).

use iced::Task;

use fluxchat_core::MessageInfo;

use crate::message::Message;
use crate::state::{FluxChat, Gallery, Modal};

/// Events automatic loading scans before waiting for "Load older" — in an
/// encrypted room the server can't pick out attachments, so filling the view
/// can mean reading a lot of ordinary messages.
const GALLERY_BURST_EVENTS: usize = 2000;
/// Timeline pages "Jump to message" loads while looking for an event older
/// than the loaded history (20 events each).
const JUMP_SEARCH_PAGES: u16 = 100;

/// Whether a message belongs on the Media tab (images, stickers, videos).
pub(crate) fn is_gallery_media(m: &MessageInfo) -> bool {
    m.image_mxc.is_some() || m.is_video
}

/// Whether a message belongs on the Files tab (files and audio).
pub(crate) fn is_gallery_file(m: &MessageInfo) -> bool {
    m.file_mxc.is_some() || m.audio_mxc.is_some()
}

/// Whether a file name suggests an animation (GIF / WebP): only those are
/// re-decoded as they scroll into view; the lightbox checks every image.
fn looks_animated(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".gif") || name.ends_with(".webp")
}

/// Add `new` to the gallery's items: one per event, newest first.
fn merge_items(items: &mut Vec<MessageInfo>, new: Vec<MessageInfo>) {
    let mut seen: std::collections::HashSet<String> = items.iter().map(|m| m.event_id.clone()).collect();
    for m in new {
        if seen.insert(m.event_id.clone()) {
            items.push(m);
        }
    }
    // Stable: same-second items keep the server's (newest first) order.
    items.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
}

impl FluxChat {
    pub(crate) fn handle_gallery_message(&mut self, message: Message) -> Option<Task<Message>> {
        let task = match message {
            Message::OpenGallery(tab) => {
                let Some(key) = self.selected_room_id.clone() else { return Some(Task::none()) };
                if self.gallery.as_ref().is_none_or(|g| g.view_key != key) {
                    self.gallery = Some(Gallery {
                        view_key: key.clone(),
                        viewport: crate::view_gallery::DEFAULT_VIEWPORT,
                        ..Default::default()
                    });
                }
                // What's loaded shows at once; the server fills in the rest.
                let loaded = self.loaded_gallery_items(&key);
                if let Some(g) = self.gallery.as_mut() {
                    g.tab = tab;
                    g.offset = 0.0;
                    g.burst = 0;
                    g.error = None;
                    merge_items(&mut g.items, loaded);
                }
                self.modal = Some(Modal::Gallery);
                Task::batch([crate::view_gallery::scroll_to_top(), self.gallery_refresh()])
            }
            Message::GalleryTabChanged(tab) => {
                if let Some(g) = self.gallery.as_mut() {
                    g.tab = tab;
                    g.offset = 0.0;
                }
                Task::batch([crate::view_gallery::scroll_to_top(), self.gallery_refresh()])
            }
            Message::GalleryScrolled(offset, viewport) => {
                if let Some(g) = self.gallery.as_mut() {
                    g.offset = offset;
                    g.viewport = viewport;
                }
                self.gallery_refresh()
            }
            Message::GalleryLoadMore => {
                if let Some(g) = self.gallery.as_mut() {
                    g.burst = 0;
                    g.error = None;
                }
                self.gallery_load_page()
            }
            Message::GalleryPageLoaded(key, result) => {
                let Some(g) = self.gallery.as_mut().filter(|g| g.view_key == key) else {
                    return Some(Task::none());
                };
                g.loading = false;
                match result {
                    Ok(page) => {
                        g.burst += page.scanned;
                        g.done = page.next.is_none();
                        g.next = page.next;
                        merge_items(&mut g.items, page.items);
                        self.gallery_refresh()
                    }
                    Err(e) => {
                        g.error = Some(e);
                        Task::none()
                    }
                }
            }
            Message::GalleryOpen(event_id) => {
                let Some(g) = &self.gallery else { return Some(Task::none()) };
                let key = g.view_key.clone();
                let Some(item) = g.items.iter().find(|m| m.event_id == event_id).cloned() else {
                    return Some(Task::none());
                };
                if self.media_hidden(&key, &item) {
                    // First click shows it; the next one opens it.
                    self.revealed_media.insert(event_id);
                    return Some(self.resolve_images(&key, std::slice::from_ref(&item)));
                }
                match item.image_mxc.clone().filter(|_| !item.is_video) {
                    Some(url) => {
                        let frames = self.ensure_gif_frames(&url);
                        self.modal = Some(Modal::Lightbox { url, from_gallery: true });
                        Task::batch([frames, self.resolve_images(&key, std::slice::from_ref(&item))])
                    }
                    // Videos play in the timeline.
                    None => self.update(Message::GalleryJump(event_id)),
                }
            }
            Message::GalleryStep(delta) => {
                let Some(Modal::Lightbox { url, from_gallery: true }) = &self.modal else {
                    return Some(Task::none());
                };
                let Some(g) = &self.gallery else { return Some(Task::none()) };
                let images: Vec<&MessageInfo> = g.items.iter()
                    .filter(|m| m.image_mxc.is_some() && !m.is_video && !self.media_hidden(&g.view_key, m))
                    .collect();
                let Some(pos) = images.iter().position(|m| m.image_mxc.as_ref() == Some(url)) else {
                    return Some(Task::none());
                };
                let target = pos as i64 + i64::from(delta);
                let Some(next) = usize::try_from(target).ok().and_then(|i| images.get(i)) else {
                    return Some(Task::none());
                };
                let next = (*next).clone();
                let key = g.view_key.clone();
                let url = next.image_mxc.clone().unwrap_or_default();
                let frames = self.ensure_gif_frames(&url);
                self.modal = Some(Modal::Lightbox { url, from_gallery: true });
                Task::batch([frames, self.resolve_images(&key, std::slice::from_ref(&next))])
            }
            Message::GalleryJump(event_id) => {
                let Some(g) = &self.gallery else { return Some(Task::none()) };
                let key = g.view_key.clone();
                let thread_root = g.items.iter().find(|m| m.event_id == event_id).and_then(|m| m.thread_root.clone());
                self.modal = None;
                match thread_root.filter(|_| !key.starts_with("thread::")) {
                    // A thread reply found from the room: open its thread.
                    Some(root) => {
                        let room = fluxchat_core::timeline::real_room_id(&key).to_string();
                        let thread = fluxchat_core::timeline::thread_key(&room, &root);
                        let open = self.update(Message::OpenThread(room, root));
                        Task::batch([open, self.jump_to_message(thread, event_id)])
                    }
                    None => self.jump_to_message(key, event_id),
                }
            }
            _ => return None,
        };
        Some(task)
    }

    /// Media and file messages of a view's loaded history, newest first.
    fn loaded_gallery_items(&self, key: &str) -> Vec<MessageInfo> {
        self.messages_by_room.get(key)
            .map(|msgs| msgs.iter().rev()
                .filter(|m| m.send_state.is_none() && (is_gallery_media(m) || is_gallery_file(m)))
                .cloned()
                .collect())
            .unwrap_or_default()
    }

    /// Load the images in (and just below) view, and the next page when the
    /// end of the list is near.
    fn gallery_refresh(&mut self) -> Task<Message> {
        let Some(g) = &self.gallery else { return Task::none() };
        let key = g.view_key.clone();
        let near_end = self.gallery_layout_height() < g.offset + g.viewport * 2.0;
        let may_load = g.burst < GALLERY_BURST_EVENTS && g.error.is_none();
        let visible = self.gallery_visible_items();
        let mut tasks = vec![self.resolve_images(&key, &visible)];
        // GIFs scrolled back into view animate again — unless the frame
        // cache is full, where re-decoding would evict others in view.
        if self.prefs.autoplay_gifs && self.gif_frames_cache.len() < crate::update::GIF_CACHE_LIMIT {
            for m in visible.iter().filter(|m| looks_animated(&m.body)) {
                if let Some(mxc) = &m.image_mxc {
                    tasks.push(self.ensure_gif_frames(mxc));
                }
            }
        }
        let images = Task::batch(tasks);
        let more = if near_end && may_load {
            self.gallery_load_page()
        } else {
            Task::none()
        };
        Task::batch([images, more])
    }

    /// Fetch the next (older) page of the gallery's view from the server.
    fn gallery_load_page(&mut self) -> Task<Message> {
        let Some(g) = self.gallery.as_mut() else { return Task::none() };
        if g.loading || g.done {
            return Task::none();
        }
        g.loading = true;
        let from = if g.started { g.next.clone() } else { None };
        g.started = true;
        let key = g.view_key.clone();
        let reply_key = key.clone();
        let core = self.core.clone();
        Task::perform(
            async move {
                let client = core.get_client().await.map_err(|e| e.to_string())?;
                let page = match fluxchat_core::timeline::parse_thread_key(&key) {
                    Some((room, root)) => fluxchat_core::gallery::thread_media_page(&client, room, root, from).await,
                    None => fluxchat_core::gallery::room_media_page(&client, &key, from).await,
                };
                page.map_err(|e| e.to_string())
            },
            move |result| Message::GalleryPageLoaded(reply_key.clone(), result),
        )
    }

    /// Show a view and scroll to one of its messages, loading older history
    /// until it turns up (see `continue_jump_search`).
    fn jump_to_message(&mut self, key: String, event_id: String) -> Task<Message> {
        let select = if self.selected_room_id.as_deref() != Some(key.as_str()) {
            self.update(Message::SelectRoom(key.clone()))
        } else {
            Task::none()
        };
        let loaded = self.messages_by_room.get(&key)
            .is_some_and(|msgs| msgs.iter().any(|m| m.event_id == event_id));
        self.read_anchor_pending = None;
        let jump = if loaded {
            Task::done(Message::JumpToEvent(event_id))
        } else {
            self.pending_jump = Some((key.clone(), event_id));
            self.jump_search_pages = JUMP_SEARCH_PAGES;
            self.start_back_pagination(key, 0)
        };
        Task::batch([select, jump])
    }

    /// After a page of history for `key`: keep paginating while a jump's
    /// event hasn't turned up. `Some` when this took over the page's outcome.
    pub(crate) fn continue_jump_search(&mut self, key: &str, outcome: Option<bool>) -> Option<Task<Message>> {
        let (jump_key, event_id) = self.pending_jump.as_ref()?;
        if jump_key != key || self.jump_search_pages == 0 {
            return None;
        }
        let found = self.messages_by_room.get(key)
            .is_some_and(|msgs| msgs.iter().any(|m| m.event_id == *event_id));
        if found {
            // MessagesUpdated already scrolled to it.
            self.jump_search_pages = 0;
            return None;
        }
        match outcome {
            Some(false) if self.jump_search_pages > 1 => {
                self.jump_search_pages -= 1;
                Some(self.start_back_pagination(key.to_string(), 0))
            }
            // A failed page: the next scroll up retries, and the jump still
            // happens if the event arrives.
            None => {
                self.jump_search_pages = 0;
                None
            }
            _ => {
                self.jump_search_pages = 0;
                self.pending_jump = None;
                Some(self.update(Message::ShowError("Couldn't find that message in the room's history".into())))
            }
        }
    }

    /// The modal to show when the current one closes: a lightbox opened from
    /// the gallery goes back to it.
    pub(crate) fn modal_after_close(&self) -> Option<Modal> {
        match &self.modal {
            Some(Modal::Lightbox { from_gallery: true, .. }) if self.gallery.is_some() => Some(Modal::Gallery),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use fluxchat_core::gallery::MediaPage;
    use fluxchat_core::MessageInfo;

    use crate::message::Message;
    use crate::panes::flow_tests::app;
    use crate::state::{GalleryTab, Modal};

    fn image(id: &str, ts: u64) -> MessageInfo {
        MessageInfo {
            event_id: id.into(), sender: "@a:x".into(), timestamp: ts,
            image_mxc: Some(format!("mxc://x/{id}")), ..Default::default()
        }
    }

    fn file(id: &str, ts: u64) -> MessageInfo {
        MessageInfo {
            event_id: id.into(), sender: "@a:x".into(), timestamp: ts,
            file_mxc: Some(format!("mxc://x/{id}")), ..Default::default()
        }
    }

    fn page(items: Vec<MessageInfo>, next: Option<&str>, scanned: usize) -> Message {
        Message::GalleryPageLoaded("!r:x".into(), Ok(MediaPage { items, next: next.map(Into::into), scanned }))
    }

    fn ids(app: &crate::state::FluxChat) -> Vec<String> {
        app.gallery.as_ref().unwrap().items.iter().map(|m| m.event_id.clone()).collect()
    }

    fn open_room(items: Vec<MessageInfo>) -> crate::state::FluxChat {
        let mut app = app();
        app.selected_room_id = Some("!r:x".into());
        let mut msgs = vec![MessageInfo { event_id: "$text".into(), body: "hi".into(), ..Default::default() }];
        msgs.extend(items);
        app.messages_by_room.insert("!r:x".into(), msgs);
        let _ = app.update(Message::OpenGallery(GalleryTab::Media));
        app
    }

    #[tokio::test]
    async fn loaded_media_shows_first_then_server_pages_merge() {
        let mut app = open_room(vec![file("$f", 50), image("$i", 100)]);
        assert!(matches!(app.modal, Some(Modal::Gallery)));
        assert_eq!(ids(&app), ["$i", "$f"], "loaded attachments, newest first; text left out");
        assert!(app.gallery.as_ref().unwrap().loading, "the first server page is on its way");

        // The server repeats what's loaded and adds older items.
        let _ = app.update(page(vec![image("$i", 100), image("$old", 10)], None, 40));
        let g = app.gallery.as_ref().unwrap();
        assert_eq!(ids(&app), ["$i", "$f", "$old"]);
        assert!(g.done && !g.loading);

        // A late page for another view is ignored.
        let _ = app.update(Message::GalleryPageLoaded("!other:x".into(), Ok(MediaPage {
            items: vec![image("$stray", 5)], next: None, scanned: 1,
        })));
        assert_eq!(ids(&app).len(), 3);
    }

    #[tokio::test]
    async fn reopening_the_same_view_keeps_what_was_loaded() {
        let mut app = open_room(vec![image("$i", 100)]);
        let _ = app.update(page(vec![image("$old", 10)], Some("t1"), 100));
        let _ = app.update(Message::CloseModal);
        let _ = app.update(Message::OpenGallery(GalleryTab::Files));
        let g = app.gallery.as_ref().unwrap();
        assert_eq!(g.tab, GalleryTab::Files);
        assert_eq!(ids(&app), ["$i", "$old"]);
        assert_eq!(g.next.as_deref(), Some("t1"));
    }

    #[tokio::test]
    async fn automatic_loading_stops_at_its_cap() {
        let mut app = open_room(vec![]);
        // A page of 2000 events with nothing in it (an encrypted room).
        let _ = app.update(page(vec![], Some("t1"), 2000));
        assert!(!app.gallery.as_ref().unwrap().loading, "waits for Load older");
        let _ = app.update(Message::GalleryLoadMore);
        assert!(app.gallery.as_ref().unwrap().loading);
    }

    #[tokio::test]
    async fn lightbox_steps_through_images_and_returns_to_the_gallery() {
        let mut app = open_room(vec![image("$c", 10), image("$b", 20), image("$a", 30)]);
        let _ = app.update(Message::GalleryOpen("$b".into()));
        let url = |app: &crate::state::FluxChat| match &app.modal {
            Some(Modal::Lightbox { url, from_gallery: true }) => url.clone(),
            other => panic!("expected a gallery lightbox, got {other:?}"),
        };
        assert_eq!(url(&app), "mxc://x/$b");
        let _ = app.update(Message::GalleryStep(1));
        assert_eq!(url(&app), "mxc://x/$c", "older");
        let _ = app.update(Message::GalleryStep(1));
        assert_eq!(url(&app), "mxc://x/$c", "stays on the last one");
        let _ = app.update(Message::GalleryStep(-2));
        assert_eq!(url(&app), "mxc://x/$a");

        let _ = app.update(Message::Escape);
        assert!(matches!(app.modal, Some(Modal::Gallery)), "Esc goes back to the gallery");
        let _ = app.update(Message::Escape);
        assert!(app.modal.is_none());
    }

    #[tokio::test]
    async fn opening_a_loaded_image_decodes_its_animation_once() {
        let mut app = open_room(vec![image("$g", 10)]);
        app.image_cache.insert("mxc://x/$g".into(), iced::widget::image::Handle::from_bytes(Vec::<u8>::new()));
        let _ = app.update(Message::GalleryOpen("$g".into()));
        assert!(app.image_cache_pending.contains("frames:mxc://x/$g"), "frames re-decoded from the image cache");
        // Not again while that's running (or once it turned out not animated).
        let _ = app.update(Message::Escape);
        let _ = app.update(Message::GalleryOpen("$g".into()));
        assert_eq!(app.image_cache_pending.iter().filter(|k| k.starts_with("frames:")).count(), 1);
    }

    #[tokio::test]
    async fn hidden_media_is_shown_before_it_opens() {
        use fluxchat_core::profile::{MediaPreviewConfig, MediaPreviews};
        let mut app = open_room(vec![image("$i", 100)]);
        let _ = app.update(Message::MediaPreviewsLoaded(MediaPreviewConfig {
            media: MediaPreviews::Off, invite_avatars: true,
        }));
        let _ = app.update(Message::GalleryOpen("$i".into()));
        assert!(matches!(app.modal, Some(Modal::Gallery)));
        assert!(app.revealed_media.contains("$i"));
        let _ = app.update(Message::GalleryOpen("$i".into()));
        assert!(matches!(app.modal, Some(Modal::Lightbox { .. })));
    }

    #[tokio::test]
    async fn jump_searches_older_history_and_gives_up_at_the_start() {
        let mut app = open_room(vec![]);
        let _ = app.update(page(vec![image("$old", 10)], None, 10));
        let _ = app.update(Message::GalleryJump("$old".into()));
        assert!(app.modal.is_none());
        assert_eq!(app.pending_jump, Some(("!r:x".into(), "$old".into())));
        assert!(app.jump_search_pages > 0);
        assert!(app.paginating, "loads older history to find it");

        // More history, still not there: keep going.
        let _ = app.update(Message::PaginateSettled("!r:x".into(), Some(false)));
        assert!(app.paginating);
        // The start of the room without it: stop and say so.
        let _ = app.update(Message::PaginateSettled("!r:x".into(), Some(true)));
        assert!(app.pending_jump.is_none());
        assert!(app.help_text.as_deref().is_some_and(|t| t.contains("Couldn't find")));
    }

    #[tokio::test]
    async fn jump_to_a_thread_reply_opens_its_thread() {
        let mut app = open_room(vec![]);
        let mut reply = image("$reply", 10);
        reply.thread_root = Some("$root".into());
        let _ = app.update(page(vec![reply], None, 10));
        let _ = app.update(Message::GalleryJump("$reply".into()));
        let thread = fluxchat_core::timeline::thread_key("!r:x", "$root");
        assert_eq!(app.selected_room_id.as_deref(), Some(thread.as_str()));
        assert_eq!(app.pending_jump, Some((thread, "$reply".into())));
    }
}
