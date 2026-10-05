//! Agent harness profiles: how a given agent framework behaves on Matrix.
//!
//! Each agent you mark is run by some harness (Hermes, OpenClaw, Claude Code
//! through a Matrix bridge, your own…), and they differ in the details: the
//! command prefix, which reactions mean working / done / failed, how
//! approvals are offered, whether progress and runtime info are posted. A
//! profile describes one, as TOML. Built-in profiles are written to
//! `data_dir/fluxchat/agents/` on first run and every `*.toml` there is
//! loaded, so a new harness is a new file — no rebuild (like themes).

use std::sync::{Arc, LazyLock, RwLock};

use serde::Deserialize;

/// The harness agents get unless one is chosen (the first fluxchat supported).
pub const DEFAULT_HARNESS: &str = "hermes";

const BUILT_IN: &[(&str, &str)] = &[
    ("hermes.toml", include_str!("../../../assets/agents/hermes.toml")),
    ("openclaw.toml", include_str!("../../../assets/agents/openclaw.toml")),
    ("claude-code.toml", include_str!("../../../assets/agents/claude-code.toml")),
    ("generic.toml", include_str!("../../../assets/agents/generic.toml")),
];

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Choice {
    pub emoji: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CommandSpec {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MenuItem {
    pub label: String,
    /// Command name, without prefix.
    pub command: String,
}

/// One harness. Every field has a default, so a profile only says what
/// differs from a plain chat bot.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct Harness {
    pub id: String,
    pub name: String,
    pub about: String,
    pub url: String,
    /// How commands go out: "!" → `!stop`; "/" → `/stop` as plain text.
    pub command_prefix: String,
    /// Outside DMs, start commands with the agent's user ID (OpenClaw).
    pub mention_commands: bool,
    /// Command names (no prefix); empty = the harness has none.
    pub stop: String,
    pub steer: String,
    pub queue: String,
    /// Reactions the agent puts on your request; empty = not used.
    pub working_reaction: String,
    pub done_reaction: String,
    pub failed_reaction: String,
    /// The agent typing in the room means it's working.
    pub typing_means_working: bool,
    /// Progress posted as a message of `{emoji} {step}` lines.
    pub progress_steps: bool,
    /// A `model · 42% · ~/dir` footer under final answers.
    pub runtime_footer: bool,
    /// Labels for the choices of its reaction prompts (others get generic
    /// labels or their line in the prompt).
    pub approvals: Vec<Choice>,
    /// Quick commands in the agent menu.
    pub menu: Vec<MenuItem>,
    /// Its commands, for autocomplete and routing.
    pub commands: Vec<CommandSpec>,
}

impl Default for Harness {
    fn default() -> Self {
        Harness {
            id: "generic".into(),
            name: "Generic agent".into(),
            about: String::new(),
            url: String::new(),
            command_prefix: "/".into(),
            mention_commands: false,
            stop: String::new(),
            steer: String::new(),
            queue: String::new(),
            working_reaction: String::new(),
            done_reaction: String::new(),
            failed_reaction: String::new(),
            typing_means_working: true,
            progress_steps: true,
            runtime_footer: false,
            approvals: Vec::new(),
            menu: Vec::new(),
            commands: Vec::new(),
        }
    }
}

impl Harness {
    /// `name` as this harness takes it: `!stop`, `/stop`.
    pub fn command(&self, name: &str) -> String {
        format!("{}{name}", self.command_prefix)
    }

    pub fn has_command(&self, name: &str) -> bool {
        self.commands.iter().any(|c| c.name == name)
    }
}

static HARNESSES: LazyLock<RwLock<Vec<Arc<Harness>>>> = LazyLock::new(|| RwLock::new(built_in()));

fn parse(src: &str) -> Option<Harness> {
    toml::from_str::<Harness>(src).ok().filter(|h| !h.id.is_empty())
}

fn built_in() -> Vec<Arc<Harness>> {
    BUILT_IN.iter().filter_map(|(_, src)| parse(src)).map(Arc::new).collect()
}

/// Write the built-in profiles to the data dir (first run only) and load
/// every profile there; a file with a built-in's id replaces it.
pub fn discover() {
    let mut all = built_in();
    if let Some(dir) = dirs::data_dir().map(|d| d.join("fluxchat").join("agents")) {
        let _ = std::fs::create_dir_all(&dir);
        for (file, src) in BUILT_IN {
            let path = dir.join(file);
            if !path.exists() {
                let _ = std::fs::write(&path, src);
            }
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .collect();
        files.sort();
        for path in files {
            match std::fs::read_to_string(&path).ok().as_deref().and_then(parse) {
                Some(h) => {
                    all.retain(|x| x.id != h.id);
                    all.push(Arc::new(h));
                }
                None => tracing::warn!("Ignoring agent profile {}: not valid", path.display()),
            }
        }
    }
    all.sort_by_key(|h| (h.id != DEFAULT_HARNESS, h.name.to_lowercase()));
    *HARNESSES.write().unwrap_or_else(|e| e.into_inner()) = all;
}

/// Every known harness (built-in and from the data dir).
pub fn all() -> Vec<Arc<Harness>> {
    HARNESSES.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The harness `id`, else the default one, else a plain generic profile.
pub fn get(id: &str) -> Arc<Harness> {
    let list = HARNESSES.read().unwrap_or_else(|e| e.into_inner());
    list.iter().find(|h| h.id == id)
        .or_else(|| list.iter().find(|h| h.id == DEFAULT_HARNESS))
        .cloned()
        .unwrap_or_else(|| Arc::new(Harness::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_profiles_parse() {
        let all = built_in();
        let ids: Vec<&str> = all.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["hermes", "openclaw", "claude-code", "generic"]);
        let hermes = &all[0];
        assert_eq!(hermes.command("stop"), "!stop");
        assert!(hermes.has_command("goal") && hermes.runtime_footer);
        assert_eq!(hermes.approvals.len(), 4);
        let openclaw = &all[1];
        assert!(openclaw.mention_commands && openclaw.steer.is_empty());
        // A profile that only says what differs gets the defaults for the rest.
        let minimal = parse("id = \"pi\"\nname = \"pi\"\nstop = \"abort\"").unwrap();
        assert_eq!(minimal.command("abort"), "/abort");
        assert!(minimal.typing_means_working && minimal.commands.is_empty());
    }
}
