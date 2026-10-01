//! Media gallery modal: a month-grouped thumbnail grid (Media) and a file
//! list (Files) for a room or thread. Rows have fixed heights so what's in
//! view can be worked out from the scroll offset — only those images load.

use std::sync::atomic::{AtomicBool, Ordering};

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Color, Element, Fill, Length, Theme};

use fluxchat_core::MessageInfo;

use crate::message::Message;
use crate::state::{FluxChat, GalleryTab, Modal};
use crate::theme;
use crate::ui::{self, Tone};
use crate::update_gallery::{is_gallery_file, is_gallery_media};
use crate::view::{thin_scrollbar, MONO};

/// Grid viewport height assumed until the first scroll event reports it.
pub(crate) const DEFAULT_VIEWPORT: f32 = 480.0;
const COLS: usize = 6;
const TILE: f32 = 124.0;
const TILE_GAP: f32 = 6.0;
const ROW_H: f32 = TILE + TILE_GAP;
const MONTH_H: f32 = 34.0;
const FILE_ROW_H: f32 = 52.0;
/// Room for the scrollbar beside the grid.
const SCROLL_GUTTER: f32 = 12.0;
const GRID_W: f32 = COLS as f32 * TILE + (COLS - 1) as f32 * TILE_GAP;
const MIN_HEIGHT: f32 = 360.0;
const MAX_HEIGHT: f32 = 760.0;
/// Space kept around the dialog (window height minus this, at most).
const WINDOW_MARGIN: f32 = 120.0;

fn gallery_scroll_id() -> iced::widget::Id {
    iced::widget::Id::new("gallery_scroll")
}

/// Scroll the grid back to the top (reopening and tab switches reuse it).
pub(crate) fn scroll_to_top<T>() -> iced::Task<T> {
    iced::widget::operation::snap_to(gallery_scroll_id(), iced::widget::scrollable::RelativeOffset::START)
}

/// Whether ←/→ step through the gallery (a lightbox opened from it is up).
/// Read by the global key listener, which can't see the app state.
static LIGHTBOX_NAV: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_lightbox_nav(on: bool) {
    LIGHTBOX_NAV.store(on, Ordering::Relaxed);
}

pub(crate) fn lightbox_nav() -> bool {
    LIGHTBOX_NAV.load(Ordering::Relaxed)
}

/// A timestamp in local time.
fn local_time(unix_secs: u64) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::TimeZone;
    chrono::Utc.timestamp_opt(unix_secs as i64, 0).single().map(|utc| utc.with_timezone(&chrono::Local))
}

/// Consecutive runs of items from the same month: ("October 2026", start, end).
/// Runs on every scroll event, so only one label per run is formatted.
fn month_sections(items: &[&MessageInfo]) -> Vec<(String, usize, usize)> {
    use chrono::Datelike;
    let mut sections: Vec<(String, usize, usize)> = Vec::new();
    let mut current: Option<(i32, u32)> = None;
    for (i, m) in items.iter().enumerate() {
        let date = local_time(m.timestamp);
        let month = date.map(|d| (d.year(), d.month()));
        match sections.last_mut() {
            Some((_, _, end)) if month == current => *end = i + 1,
            _ => {
                current = month;
                let label = date.map(|d| d.format("%B %Y").to_string()).unwrap_or_default();
                sections.push((label, i, i + 1));
            }
        }
    }
    sections
}

impl FluxChat {
    /// The gallery items on its current tab, newest first.
    fn gallery_tab_items(&self) -> Vec<&MessageInfo> {
        let Some(g) = &self.gallery else { return Vec::new() };
        g.items.iter()
            .filter(|m| match g.tab {
                GalleryTab::Media => is_gallery_media(m),
                GalleryTab::Files => is_gallery_file(m),
            })
            .collect()
    }

    /// Height of the current tab's list (month headings and rows).
    pub(crate) fn gallery_layout_height(&self) -> f32 {
        let Some(g) = &self.gallery else { return 0.0 };
        month_sections(&self.gallery_tab_items()).iter()
            .map(|(_, start, end)| {
                let n = end - start;
                MONTH_H + match g.tab {
                    GalleryTab::Media => n.div_ceil(COLS) as f32 * ROW_H,
                    GalleryTab::Files => n as f32 * FILE_ROW_H,
                }
            })
            .sum()
    }

    /// Media items whose row is in view or within a screen below it.
    pub(crate) fn gallery_visible_items(&self) -> Vec<MessageInfo> {
        let Some(g) = &self.gallery else { return Vec::new() };
        if g.tab != GalleryTab::Media {
            return Vec::new();
        }
        let top = g.offset - ROW_H;
        let bottom = g.offset + g.viewport * 2.0;
        let items = self.gallery_tab_items();
        let mut out = Vec::new();
        let mut y = 0.0;
        for (_, start, end) in month_sections(&items) {
            y += MONTH_H;
            for row_start in (start..end).step_by(COLS) {
                if y + ROW_H >= top && y <= bottom {
                    out.extend(items[row_start..(row_start + COLS).min(end)].iter().map(|m| (*m).clone()));
                }
                y += ROW_H;
            }
            if y > bottom {
                break;
            }
        }
        out
    }

    /// The image (or video thumbnail) to show for an item, if loaded.
    fn gallery_thumb(&self, m: &MessageInfo) -> Option<&iced::widget::image::Handle> {
        m.image_mxc.as_ref()
            .and_then(|mxc| self.image_cache.get(mxc))
            .or_else(|| m.video_mxc.as_ref().and_then(|v| self.image_cache.get(&format!("video_thumb:{v}"))))
    }

    pub(crate) fn view_gallery_modal(&self) -> Element<'_, Message> {
        let Some(g) = &self.gallery else {
            return ui::empty_state(crate::icons::SVG_GALLERY, "Nothing to show", None);
        };
        let real_room = fluxchat_core::timeline::real_room_id(&g.view_key);
        let room_name = self.rooms.iter().find(|r| r.room_id == real_room)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| real_room.to_string());
        let in_thread = g.view_key.starts_with("thread::");
        let subtitle = if in_thread { format!("{room_name} \u{203a} thread") } else { room_name };

        let media_count = g.items.iter().filter(|m| is_gallery_media(m)).count();
        let file_count = g.items.iter().filter(|m| is_gallery_file(m)).count();
        let count = |n: usize| if g.done { n.to_string() } else { format!("{n}+") };
        let tabs = ui::tabs(vec![
            (format!("Media  {}", count(media_count)), Message::GalleryTabChanged(GalleryTab::Media), g.tab == GalleryTab::Media),
            (format!("Files  {}", count(file_count)), Message::GalleryTabChanged(GalleryTab::Files), g.tab == GalleryTab::Files),
        ]);

        let items = self.gallery_tab_items();
        let body: Element<Message> = if items.is_empty() && (g.loading || !g.started) {
            ui::empty_state(crate::icons::SVG_GALLERY, "Loading\u{2026}", None)
        } else if items.is_empty() && g.done {
            let (title, detail) = match g.tab {
                GalleryTab::Media => ("No images or videos", "Pictures and videos posted here will show up here"),
                GalleryTab::Files => ("No files", "Files and audio posted here will show up here"),
            };
            ui::empty_state(crate::icons::SVG_GALLERY, title, Some(detail.into()))
        } else {
            let mut list = column![].spacing(0).width(Length::Fixed(GRID_W));
            for (label, start, end) in month_sections(&items) {
                list = list.push(
                    container(text(label).font(MONO).size(12).color(theme::colors().text_dim))
                        .height(MONTH_H)
                        .align_y(iced::alignment::Vertical::Center),
                );
                let section = &items[start..end];
                match g.tab {
                    GalleryTab::Media => {
                        for chunk in section.chunks(COLS) {
                            let mut r = row![].spacing(TILE_GAP);
                            for m in chunk {
                                r = r.push(self.gallery_tile(&g.view_key, m));
                            }
                            list = list.push(container(r).height(ROW_H));
                        }
                    }
                    GalleryTab::Files => {
                        for m in section {
                            list = list.push(self.gallery_file_row(&g.view_key, m));
                        }
                    }
                }
            }
            list = list.push(self.gallery_footer(in_thread));
            scrollable(container(list).padding(iced::Padding { right: SCROLL_GUTTER, ..Default::default() }))
                .id(gallery_scroll_id())
                .height(Fill)
                .on_scroll(|vp| Message::GalleryScrolled(vp.absolute_offset().y, vp.bounds().height))
                .direction(iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                ))
                .style(thin_scrollbar)
                .into()
        };

        let height = (self.window_size.1 - WINDOW_MARGIN).clamp(MIN_HEIGHT, MAX_HEIGHT);
        column![
            ui::header("Gallery", Some(subtitle)),
            tabs,
            body,
        ]
        .spacing(ui::GAP)
        .width(Length::Fixed(GRID_W + SCROLL_GUTTER))
        .height(Length::Fixed(height))
        .into()
    }

    /// A square thumbnail: the image, a placeholder while it loads, or a
    /// "Show" cover for media the preview setting hides.
    fn gallery_tile<'a>(&'a self, view_key: &str, m: &MessageInfo) -> Element<'a, Message> {
        let c = theme::colors();
        let placeholder = |svg: &'static [u8], label: Option<&'static str>| -> Element<'a, Message> {
            let mut col = column![ui::icon(svg, 22.0, c.text_dim)].spacing(6).align_x(iced::Alignment::Center);
            if let Some(l) = label {
                col = col.push(text(l).font(MONO).size(11).color(c.text_dim));
            }
            container(col)
                .width(TILE).height(TILE)
                .center(TILE)
                .style(|_: &Theme| container::Style {
                    background: Some(theme::colors().bg_mid.into()),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into()
        };
        let hidden = self.media_hidden(view_key, m);
        let frames = m.image_mxc.as_ref()
            .and_then(|mxc| self.gif_frames_cache.get(mxc))
            .filter(|_| self.prefs.autoplay_gifs && !m.is_video);
        let mut content: Element<Message> = if hidden {
            placeholder(crate::icons::SVG_EYE_OFF, Some("Show"))
        } else if let Some(frames) = frames {
            iced_gif::gif(frames.as_ref())
                .content_fit(iced::ContentFit::Cover)
                .width(TILE)
                .height(TILE)
                .into()
        } else if let Some(handle) = self.gallery_thumb(m) {
            iced::widget::image(handle.clone())
                .content_fit(iced::ContentFit::Cover)
                .width(TILE)
                .height(TILE)
                .into()
        } else if m.is_video {
            placeholder(crate::icons::SVG_PLAY, None)
        } else {
            placeholder(crate::icons::SVG_IMAGE, None)
        };
        if m.is_video && !hidden {
            // Play badge in the corner (black scrim: it sits on the image).
            let badge = container(ui::icon(crate::icons::SVG_PLAY, 12.0, Color::WHITE))
                .padding(6)
                .style(|_: &Theme| container::Style {
                    background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.55).into()),
                    border: iced::Border { radius: 12.0.into(), ..Default::default() },
                    ..Default::default()
                });
            content = iced::widget::stack![
                content,
                container(badge).width(TILE).height(TILE).padding(6)
                    .align_x(iced::alignment::Horizontal::Left)
                    .align_y(iced::alignment::Vertical::Bottom),
            ]
            .into();
        }
        let tile = button(content)
            .on_press(Message::GalleryOpen(m.event_id.clone()))
            .padding(0)
            .width(TILE)
            .height(TILE)
            .style(|_, status| button::Style {
                border: iced::Border {
                    color: if matches!(status, button::Status::Hovered) { theme::colors().accent } else { Color::TRANSPARENT },
                    width: 2.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });
        let tip = format!("{}\n{} \u{00b7} {}", m.body, m.sender_name, crate::view_messages::local_day_label(m.timestamp));
        ui::tooltip_at(tile, tip, iced::widget::tooltip::Position::Bottom)
    }

    /// A file row: icon, name, size · sender · date, Download and Jump.
    fn gallery_file_row<'a>(&'a self, view_key: &str, m: &MessageInfo) -> Element<'a, Message> {
        let c = theme::colors();
        let svg = if m.audio_mxc.is_some() { crate::icons::SVG_HEADPHONES } else { crate::icons::SVG_FILE };
        let mut detail: Vec<String> = Vec::new();
        if let Some(size) = m.file_size {
            detail.push(crate::view_messages::format_file_size(size));
        }
        detail.push(m.sender_name.clone());
        detail.push(crate::view_messages::local_day_label(m.timestamp));
        let real_room = fluxchat_core::timeline::real_room_id(view_key).to_string();
        container(
            row![
                container(ui::icon(svg, 16.0, c.text_muted))
                    .width(32).height(32)
                    .center(32)
                    .style(|_: &Theme| container::Style {
                        background: Some(theme::colors().bg_mid.into()),
                        border: iced::Border { radius: 6.0.into(), ..Default::default() },
                        ..Default::default()
                    }),
                column![
                    text(m.body.clone()).font(MONO).size(13).color(c.text_bright)
                        .wrapping(iced::widget::text::Wrapping::None),
                    text(detail.join("  \u{00b7}  ")).font(MONO).size(11).color(c.text_dim)
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(2)
                .width(Fill)
                .clip(true),
                ui::icon_only(crate::icons::SVG_DOWNLOAD, "Download", Tone::Ghost,
                    Some(Message::DownloadFile(m.event_id.clone(), real_room))),
                ui::icon_only(crate::icons::SVG_CHEVRON_RIGHT, "Jump to message", Tone::Ghost,
                    Some(Message::GalleryJump(m.event_id.clone()))),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center),
        )
        .height(FILE_ROW_H)
        .align_y(iced::alignment::Vertical::Center)
        .into()
    }

    /// Under the list: loading, an error with Retry, "Load older", or the
    /// start of the room.
    fn gallery_footer(&self, in_thread: bool) -> Element<'_, Message> {
        let Some(g) = &self.gallery else { return Space::new().into() };
        let dim = |s: &str| -> Element<'_, Message> {
            text(s.to_string()).font(MONO).size(12).color(theme::colors().text_dim).into()
        };
        let content: Element<Message> = if g.loading {
            dim("Loading older\u{2026}")
        } else if let Some(e) = &g.error {
            row![
                ui::status_line(e),
                ui::btn("Retry", Tone::Secondary, Some(Message::GalleryLoadMore)),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .into()
        } else if g.done {
            dim(if in_thread { "Start of the thread" } else { "Start of the room" })
        } else {
            ui::btn("Load older", Tone::Secondary, Some(Message::GalleryLoadMore)).into()
        };
        container(content).width(Fill).padding([14, 0]).center_x(Fill).into()
    }

    /// Lightbox controls when it was opened from the gallery: previous /
    /// next and Jump to message.
    pub(crate) fn gallery_lightbox_controls(&self, url: &str) -> Vec<Element<'_, Message>> {
        let Some(g) = &self.gallery else { return Vec::new() };
        let images: Vec<&MessageInfo> = g.items.iter()
            .filter(|m| m.image_mxc.is_some() && !m.is_video && !self.media_hidden(&g.view_key, m))
            .collect();
        let Some(pos) = images.iter().position(|m| m.image_mxc.as_deref() == Some(url)) else { return Vec::new() };
        let event_id = images[pos].event_id.clone();
        vec![
            ui::icon_only(crate::icons::SVG_CHEVRON_LEFT, "Newer (\u{2190})", Tone::Secondary,
                (pos > 0).then_some(Message::GalleryStep(-1))),
            ui::icon_only(crate::icons::SVG_CHEVRON_RIGHT, "Older (\u{2192})", Tone::Secondary,
                (pos + 1 < images.len()).then_some(Message::GalleryStep(1))),
            ui::icon_only(crate::icons::SVG_CHAT, "Jump to message", Tone::Secondary,
                Some(Message::GalleryJump(event_id))),
        ]
    }

    /// The gallery item showing `url`, for the lightbox's title.
    pub(crate) fn gallery_item_for(&self, url: &str) -> Option<&MessageInfo> {
        self.gallery.as_ref()?.items.iter().find(|m| m.image_mxc.as_deref() == Some(url))
    }
}

/// Whether the open modal is a lightbox from the gallery.
pub(crate) fn is_gallery_lightbox(modal: Option<&Modal>) -> bool {
    matches!(modal, Some(Modal::Lightbox { from_gallery: true, .. }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_group_into_month_runs() {
        use chrono::TimeZone;
        let at = |y, m, d| MessageInfo {
            timestamp: chrono::Local.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap().timestamp() as u64,
            ..Default::default()
        };
        let items = [at(2026, 10, 1), at(2026, 10, 1), at(2026, 9, 30), at(2025, 9, 30)];
        let refs: Vec<&MessageInfo> = items.iter().collect();
        let sections = month_sections(&refs);
        assert_eq!(sections, vec![
            ("October 2026".to_string(), 0, 2),
            ("September 2026".to_string(), 2, 3),
            ("September 2025".to_string(), 3, 4),
        ]);
    }
}
