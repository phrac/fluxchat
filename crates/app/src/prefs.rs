//! User preferences shown in Settings → Preferences, persisted as one JSON
//! blob under the `preferences` setting. (Timestamps, compact mode and
//! join/leave visibility predate this and keep their own settings keys.)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    // --- Timeline ---
    /// 12-hour clock (2:05 PM) instead of 24-hour.
    pub twelve_hour: bool,
    /// Fade the nick and drop the avatar on consecutive messages.
    pub group_messages: bool,
    /// Display-name and avatar changes.
    pub show_profile_changes: bool,
    /// Room name / topic / avatar / power-level changes.
    pub show_room_changes: bool,
    /// "Message deleted" placeholders.
    pub show_redactions: bool,
    pub show_read_receipts: bool,
    pub show_typing: bool,
    /// Emoji-only messages at a larger size.
    pub big_emoji: bool,
    pub url_previews: bool,
    pub autoplay_gifs: bool,
    /// Code blocks start expanded instead of collapsed to a few lines.
    pub expand_code_blocks: bool,

    // --- Composer ---
    /// Ctrl/Cmd+Enter sends; Enter inserts a newline.
    pub ctrl_enter_to_send: bool,
    pub use_markdown: bool,
    /// `:shortcode` completion popup.
    pub emoji_suggestions: bool,
    /// Replace `:)` style emoticons with emoji when sending.
    pub auto_replace_emoji: bool,

    // --- Privacy ---
    /// Public read receipts (off = private receipts).
    pub send_read_receipts: bool,
    pub send_typing: bool,
    pub share_presence: bool,

    // --- Voice & video ---
    /// Microphone by name; `None` = system default.
    pub audio_input: Option<String>,
    /// Speaker by name; `None` = system default.
    pub audio_output: Option<String>,
    pub echo_cancellation: bool,
    pub noise_suppression: bool,
    pub auto_gain: bool,

    // --- AI agents (see agents.rs) ---
    /// User IDs treated as AI agents (marked on their profile card).
    pub agents: Vec<String>,
    /// The harness (profile id, harness.rs) running each agent; unset = Hermes.
    pub agent_harness: std::collections::HashMap<String, String>,
    /// Open the thread an agent starts on your request as it starts.
    pub follow_agent_threads: bool,
    /// What a message sent while an agent is working does.
    pub agent_busy_mode: crate::agents::BusyMode,
    /// Notify when an agent run of 30 s or more finishes (when you're not looking).
    pub notify_agent_done: bool,
}

impl Prefs {
    /// The audio part, in the form core's capture/playback code uses.
    pub fn audio(&self) -> fluxchat_core::audio_devices::AudioPrefs {
        fluxchat_core::audio_devices::AudioPrefs {
            input: self.audio_input.clone(),
            output: self.audio_output.clone(),
            echo_cancellation: self.echo_cancellation,
            noise_suppression: self.noise_suppression,
            auto_gain: self.auto_gain,
        }
    }
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            twelve_hour: false,
            group_messages: true,
            show_profile_changes: true,
            show_room_changes: true,
            show_redactions: true,
            show_read_receipts: true,
            show_typing: true,
            big_emoji: true,
            url_previews: true,
            autoplay_gifs: true,
            expand_code_blocks: false,
            ctrl_enter_to_send: false,
            use_markdown: true,
            emoji_suggestions: true,
            auto_replace_emoji: false,
            send_read_receipts: true,
            send_typing: true,
            share_presence: true,
            audio_input: None,
            audio_output: None,
            echo_cancellation: true,
            noise_suppression: true,
            auto_gain: true,
            agents: Vec::new(),
            agent_harness: std::collections::HashMap::new(),
            follow_agent_threads: true,
            agent_busy_mode: crate::agents::BusyMode::Send,
            notify_agent_done: true,
        }
    }
}

/// Settings key the preferences are stored under.
pub const PREFS_KEY: &str = "preferences";

/// Classic emoticons replaced when `auto_replace_emoji` is on.
const EMOTICONS: &[(&str, &str)] = &[
    (":)", "\u{1f642}"), (":-)", "\u{1f642}"), (":D", "\u{1f600}"), (":-D", "\u{1f600}"),
    (";)", "\u{1f609}"), (";-)", "\u{1f609}"), (":(", "\u{1f641}"), (":-(", "\u{1f641}"),
    (":P", "\u{1f61b}"), (":-P", "\u{1f61b}"), (":p", "\u{1f61b}"), (":O", "\u{1f62e}"),
    (":o", "\u{1f62e}"), (":'(", "\u{1f622}"), ("<3", "\u{2764}\u{fe0f}"), (":|", "\u{1f610}"),
    ("B)", "\u{1f60e}"), ("xD", "\u{1f606}"), ("XD", "\u{1f606}"),
];

/// Replace whitespace-separated emoticons with emoji, leaving code
/// (anything between backticks) untouched.
pub fn replace_emoticons(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;
    for (i, segment) in text.split('`').enumerate() {
        if i > 0 {
            out.push('`');
            in_code = !in_code;
        }
        if in_code {
            out.push_str(segment);
            continue;
        }
        let mut first = true;
        for word in segment.split(' ') {
            if !first {
                out.push(' ');
            }
            first = false;
            let replaced = EMOTICONS.iter().find(|(from, _)| *from == word).map(|(_, to)| *to);
            out.push_str(replaced.unwrap_or(word));
        }
    }
    out
}

/// Whether a message consists only of emoji (1–5 of them), for big-emoji display.
pub fn is_emoji_only(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let mut count = 0;
    let mut rest = trimmed;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        // Longest emoji (multi-codepoint sequences) matching the start.
        let mut matched = 0;
        for end in rest.char_indices().map(|(i, c)| i + c.len_utf8()).take(12) {
            if emojis::get(&rest[..end]).is_some() {
                matched = end;
            }
        }
        if matched == 0 {
            return false;
        }
        count += 1;
        if count > 5 {
            return false;
        }
        rest = &rest[matched..];
    }
    count > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoticons_replaced_outside_code_only() {
        assert_eq!(replace_emoticons("hi :) there"), "hi \u{1f642} there");
        assert_eq!(replace_emoticons("see `a :) b` ok :D"), "see `a :) b` ok \u{1f600}");
        assert_eq!(replace_emoticons("http://x:)"), "http://x:)");
    }

    #[test]
    fn emoji_only_detection() {
        assert!(is_emoji_only("\u{1f680}"));
        assert!(is_emoji_only("\u{1f44d} \u{1f525}"));
        assert!(is_emoji_only("\u{2764}\u{fe0f}"));
        assert!(!is_emoji_only("ok \u{1f44d}"));
        assert!(!is_emoji_only(""));
        assert!(!is_emoji_only("\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}\u{1f600}"));
    }
}
