//! Reviewing what a coding agent changes: the diffs in its message (code
//! blocks, or a `.diff` / `.patch` file) in a dialog — file list, numbered
//! and coloured lines — with Approve / Request changes, or the agent's own
//! choices when it's waiting on an approval.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Fill, Length, Task, Theme};

use crate::diff::{FileDiff, LineKind};
use crate::message::Message;
use crate::state::FluxChat;
use crate::ui::{self, Tone};

/// Lines shown per file (the rest is summarised).
const MAX_LINES_SHOWN: usize = 4000;
/// Patch files bigger than this aren't opened.
const MAX_PATCH_BYTES: u64 = 2 * 1024 * 1024;

/// The review dialog's state.
#[derive(Debug, Clone, Default)]
pub struct Review {
    pub view_key: String,
    pub event_id: String,
    pub files: Vec<FileDiff>,
    pub selected: usize,
    pub comment: String,
    pub loading: bool,
}

/// A message's diff summary: (files, added, removed), from its code blocks.
pub fn summary(msg: &fluxchat_core::MessageInfo) -> Option<(usize, usize, usize)> {
    // Cheap check first: this runs while rendering agent messages.
    if !(msg.body.contains("@@") || msg.body.contains("+++ ")) {
        return None;
    }
    let files: Vec<FileDiff> = crate::diff::diffs_in(&msg.body, msg.formatted_body.as_deref())
        .iter().flat_map(|d| crate::diff::parse(d)).collect();
    (!files.is_empty()).then(|| (files.len(), files.iter().map(|f| f.added).sum(), files.iter().map(|f| f.removed).sum()))
}

/// A `.diff` / `.patch` attachment.
pub fn is_patch_file(msg: &fluxchat_core::MessageInfo) -> bool {
    msg.file_mxc.is_some() && (
        matches!(msg.file_mimetype.as_deref(), Some("text/x-diff" | "text/x-patch"))
            || [".diff", ".patch"].iter().any(|ext| msg.body.to_lowercase().ends_with(ext))
    ) && msg.file_size.is_none_or(|s| s <= MAX_PATCH_BYTES)
}

impl FluxChat {
    /// "Review changes · 3 files +40 −12" under an agent's message with a diff.
    pub(crate) fn view_review_button<'a>(&self, view_key: &str, msg: &fluxchat_core::MessageInfo) -> Option<Element<'a, Message>> {
        if !self.prefs.agents.contains(&msg.sender) {
            return None;
        }
        let label = match summary(msg) {
            Some((files, added, removed)) => format!(
                "Review changes \u{00b7} {files} file{} +{added} \u{2212}{removed}", if files == 1 { "" } else { "s" }),
            None if is_patch_file(msg) => "Review changes".to_string(),
            None => return None,
        };
        Some(row![
            Space::new().width(Length::Fixed(self.msg_content_indent())),
            button(row![
                ui::icon(crate::icons::SVG_CODE, 13.0, crate::theme::colors().accent),
                text(label).font(self.sidebar_font()).size(12).color(crate::theme::colors().accent),
            ].spacing(6).align_y(Alignment::Center))
            .on_press(Message::OpenReview(view_key.to_string(), msg.event_id.clone()))
            .padding([3, 8])
            .style(ui::row_style(false)),
        ].into())
    }

    /// Open the review of a message's diffs (downloading a patch file first).
    pub(crate) fn open_review(&mut self, view_key: String, event_id: String) -> Task<Message> {
        let Some(msg) = self.messages_by_room.get(&view_key).and_then(|m| m.iter().find(|m| m.event_id == event_id)).cloned() else {
            return Task::none();
        };
        let files: Vec<FileDiff> = crate::diff::diffs_in(&msg.body, msg.formatted_body.as_deref())
            .iter().flat_map(|d| crate::diff::parse(d)).collect();
        let fetch = files.is_empty() && is_patch_file(&msg);
        self.review = Some(Review { view_key: view_key.clone(), event_id: event_id.clone(), files, loading: fetch, ..Default::default() });
        self.modal = Some(crate::state::Modal::Review);
        if !fetch {
            return Task::none();
        }
        let core = self.core.clone();
        let room = fluxchat_core::timeline::real_room_id(&view_key).to_string();
        Task::perform(
            async move {
                let client = core.get_client().await.map_err(|e| e.to_string())?;
                let (bytes, _) = fluxchat_core::media::get_event_media_bytes(&client, &room, &event_id).await.map_err(|e| e.to_string())?;
                Ok::<_, String>((event_id, String::from_utf8_lossy(&bytes).into_owned()))
            },
            |r| match r {
                Ok((id, text)) => Message::ReviewLoaded(id, text),
                Err(e) => Message::ShowError(format!("Couldn't open the patch: {e}")),
            },
        )
    }

    pub(crate) fn review_loaded(&mut self, event_id: String, patch: String) {
        if let Some(r) = self.review.as_mut().filter(|r| r.event_id == event_id) {
            r.files = crate::diff::parse(&patch);
            r.loading = false;
        }
    }

    /// The agent's open prompt the review answers: the newest one from the
    /// message's sender in the same view, at or after the diff.
    fn review_prompt(&self, r: &Review) -> Option<(String, Vec<(String, String)>)> {
        let msgs = self.messages_by_room.get(&r.view_key)?;
        let at = msgs.iter().position(|m| m.event_id == r.event_id)?;
        let sender = &msgs[at].sender;
        msgs[at..].iter().rev()
            .filter(|m| m.sender == *sender)
            .find_map(|m| Some(m.event_id.clone()).zip(Some(self.agent_prompt(m)).filter(|c| !c.is_empty())))
    }

    /// Approve / request changes: answer the agent's prompt if it has one
    /// open (its choice), else reply in the view.
    pub(crate) fn finish_review(&mut self, approve: bool) -> Task<Message> {
        let Some(r) = self.review.take() else { return Task::none() };
        self.modal = None;
        let comment = r.comment.trim();
        let text = match (approve, comment.is_empty()) {
            (true, true) => "Approved \u{2014} go ahead.".to_string(),
            (true, false) => format!("Approved \u{2014} go ahead. {comment}"),
            (false, true) => "Please don't apply this; let's revise it first.".to_string(),
            (false, false) => format!("Changes requested: {comment}"),
        };
        Task::done(Message::AgentSend(r.view_key, text))
    }

    pub(crate) fn view_review_modal(&self) -> Element<'_, Message> {
        let colors = crate::theme::colors();
        let Some(r) = &self.review else { return Space::new().into() };
        let added: usize = r.files.iter().map(|f| f.added).sum();
        let removed: usize = r.files.iter().map(|f| f.removed).sum();
        let sender = self.messages_by_room.get(&r.view_key)
            .and_then(|m| m.iter().find(|m| m.event_id == r.event_id))
            .map(|m| m.sender_name.clone()).unwrap_or_default();
        let subtitle = if r.loading {
            "Loading the patch\u{2026}".to_string()
        } else {
            format!("{} file{} \u{00b7} +{added} \u{2212}{removed} \u{00b7} from {sender}",
                r.files.len(), if r.files.len() == 1 { "" } else { "s" })
        };

        // File list.
        let mut list = column![].spacing(2);
        for (i, f) in r.files.iter().enumerate() {
            list = list.push(
                button(column![
                    text(f.path.rsplit('/').next().unwrap_or(&f.path).to_string()).font(crate::view::MONO).size(12)
                        .color(colors.text_bright).wrapping(iced::widget::text::Wrapping::None),
                    row![
                        text(format!("+{}", f.added)).font(crate::view::MONO).size(11).color(colors.green),
                        text(format!("\u{2212}{}", f.removed)).font(crate::view::MONO).size(11).color(colors.red),
                        text(f.path.clone()).font(crate::view::MONO).size(11).color(colors.text_dim)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(6),
                ].spacing(2))
                .on_press(Message::ReviewSelectFile(i))
                .padding([5, 8])
                .width(Fill)
                .clip(true)
                .style(ui::row_style(i == r.selected)),
            );
        }

        // The selected file's diff.
        let mut lines = column![].spacing(0);
        if let Some(f) = r.files.get(r.selected) {
            let mut shown = 0usize;
            'hunks: for h in &f.hunks {
                lines = lines.push(container(text(h.header.clone()).font(crate::view::MONO).size(11).color(colors.accent))
                    .padding([3, 8]).width(Fill)
                    .style(move |_: &Theme| container::Style {
                        background: Some(crate::theme::with_alpha(colors.accent, 0.08).into()),
                        ..Default::default()
                    }));
                for l in &h.lines {
                    if shown >= MAX_LINES_SHOWN {
                        lines = lines.push(text(format!("\u{2026} {} more lines", f.added + f.removed)).font(crate::view::MONO).size(11).color(colors.text_dim));
                        break 'hunks;
                    }
                    shown += 1;
                    let (sign, tint) = match l.kind {
                        LineKind::Added => ("+", Some(colors.green)),
                        LineKind::Removed => ("\u{2212}", Some(colors.red)),
                        LineKind::Context => (" ", None),
                    };
                    let num = |n: Option<u32>| text(n.map(|n| n.to_string()).unwrap_or_default()).font(crate::view::MONO).size(11)
                        .color(colors.text_dim).width(Length::Fixed(38.0)).align_x(iced::alignment::Horizontal::Right);
                    lines = lines.push(container(row![
                        num(l.old),
                        num(l.new),
                        text(sign).font(crate::view::MONO).size(12).color(tint.unwrap_or(colors.text_dim)).width(Length::Fixed(12.0)),
                        text(l.text.clone()).font(crate::view::MONO).size(12).color(colors.text_normal)
                            .wrapping(iced::widget::text::Wrapping::None),
                    ].spacing(8)).padding([0, 8]).width(Fill)
                    .style(move |_: &Theme| container::Style {
                        background: tint.map(|c| crate::theme::with_alpha(c, 0.12).into()),
                        ..Default::default()
                    }));
                }
            }
        }
        let body: Element<Message> = if r.loading {
            ui::empty_state(crate::icons::SVG_CLOCK, "Loading the patch\u{2026}", None)
        } else if r.files.is_empty() {
            ui::empty_state(crate::icons::SVG_CODE, "No changes found", Some("This message doesn't contain a diff fluxchat can read.".into()))
        } else {
            row![
                container(scrollable(list)).width(Length::Fixed(220.0)).height(Fill),
                container(scrollable(lines).direction(iced::widget::scrollable::Direction::Both {
                    vertical: iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                    horizontal: iced::widget::scrollable::Scrollbar::new().width(4).scroller_width(4),
                }))
                .width(Fill).height(Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(colors.code_bg.into()),
                    border: iced::Border { color: colors.border, width: 1.0, radius: 6.0.into() },
                    ..Default::default()
                }),
            ].spacing(10).into()
        };

        // Answer: the agent's own choices when it's waiting, else a reply.
        let footer: Element<Message> = match self.review_prompt(r) {
            Some((prompt_id, choices)) => {
                let mut b = row![text("The agent is waiting:").font(self.sidebar_font()).size(12).color(colors.text_dim)]
                    .spacing(8).align_y(Alignment::Center);
                for (emoji, label) in choices {
                    let tone = if label.to_lowercase().contains("deny") { Tone::Danger } else { Tone::Primary };
                    b = b.push(ui::btn(format!("{emoji} {label}"), tone, Some(Message::Batch(vec![
                        Message::CloseModal,
                        Message::ToggleReaction(prompt_id.clone(), emoji.clone()),
                    ]))));
                }
                b.into()
            }
            None => row![
                ui::input("Comment (optional)", &r.comment, Message::ReviewComment).width(Fill),
                ui::btn("Request changes", Tone::Secondary, Some(Message::ReviewFinish(false))),
                ui::btn("Approve", Tone::Primary, Some(Message::ReviewFinish(true))),
            ].spacing(8).align_y(Alignment::Center).into(),
        };

        column![
            ui::header("Review changes", Some(subtitle)),
            container(body).height(Length::Fixed(520.0)),
            footer,
        ]
        .spacing(ui::GAP)
        .width(Length::Fixed(980.0))
        .into()
    }
}
