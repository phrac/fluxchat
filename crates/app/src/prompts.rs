//! Prompt history and snippets: what you've sent, searchable with Ctrl+R
//! (the palette's `<` mode), and named prompts you save with `/snippet`.
//! Both hold what you wrote, so they're kept encrypted (`AppDb::set_private`,
//! available from sign-in); a plaintext copy from before is moved there.

use crate::state::FluxChat;

/// Sent messages kept for recall.
const HISTORY_LIMIT: usize = 300;
/// Prompts listed in the palette at once.
const PALETTE_PROMPTS: usize = 40;
const HISTORY_KEY: &str = "prompt_history";
const SNIPPETS_KEY: &str = "snippets";

impl FluxChat {
    /// After sign-in (the private store is unlocked then).
    pub(crate) fn load_prompts(&mut self) {
        let db = &self.core.db;
        let load = |key: &str| -> Option<String> {
            if let Some(bytes) = db.get_private(key) {
                return String::from_utf8(bytes).ok();
            }
            // Kept in plain settings by an earlier version: move it.
            let old = db.get_setting(key)?;
            if db.set_private(key, old.as_bytes()) {
                db.remove_setting(key);
            }
            Some(old)
        };
        if let Some(json) = load(HISTORY_KEY) {
            self.prompt_history = serde_json::from_str(&json).unwrap_or_default();
        }
        if let Some(json) = load(SNIPPETS_KEY) {
            self.snippets = serde_json::from_str(&json).unwrap_or_default();
        }
    }

    /// Remember something you sent (newest last; a repeat moves to the end).
    pub(crate) fn remember_prompt(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.prompt_history.retain(|p| p != text);
        self.prompt_history.push(text.to_string());
        if self.prompt_history.len() > HISTORY_LIMIT {
            let extra = self.prompt_history.len() - HISTORY_LIMIT;
            self.prompt_history.drain(..extra);
        }
        if let Ok(json) = serde_json::to_string(&self.prompt_history) {
            self.core.db.set_private(HISTORY_KEY, json.as_bytes());
        }
    }

    /// Save (or replace) a named snippet; an empty text removes it.
    pub(crate) fn set_snippet(&mut self, name: &str, text: &str) {
        self.snippets.retain(|(n, _)| n != name);
        if !text.trim().is_empty() {
            self.snippets.push((name.to_string(), text.to_string()));
            self.snippets.sort_by_key(|(n, _)| n.to_lowercase());
        }
        if let Ok(json) = serde_json::to_string(&self.snippets) {
            self.core.db.set_private(SNIPPETS_KEY, json.as_bytes());
        }
    }

    /// Palette prompts for `query` (the palette's `<` mode): matching
    /// snippets, then your history, newest first. `(text, snippet name)`.
    pub(crate) fn matching_prompts(&self, query: &str) -> Vec<(String, Option<String>)> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let hit = |s: &str| {
            let s = s.to_lowercase();
            words.iter().all(|w| s.contains(w.as_str()))
        };
        self.snippets.iter()
            .filter(|(n, t)| hit(n) || hit(t))
            .map(|(n, t)| (t.clone(), Some(n.clone())))
            .chain(self.prompt_history.iter().rev()
                .filter(|p| hit(p))
                .map(|p| (p.clone(), None)))
            .take(PALETTE_PROMPTS)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn history_dedupes_and_snippets_come_first() {
        let mut app = crate::panes::flow_tests::app();
        app.remember_prompt("check the odds");
        app.remember_prompt("!goal deep dive");
        app.remember_prompt("check the odds");
        assert_eq!(app.prompt_history, ["!goal deep dive", "check the odds"]);
        app.set_snippet("odds", "pull tonight's lines and compare books");
        let found = app.matching_prompts("odds");
        assert_eq!(found[0], ("pull tonight's lines and compare books".to_string(), Some("odds".to_string())));
        assert_eq!(found[1], ("check the odds".to_string(), None));
        app.set_snippet("odds", "");
        assert!(app.snippets.is_empty());
    }
}
