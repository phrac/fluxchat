//! Agent budgets: a spend limit per agent per day / week / month.
//!
//! Spend comes from run history (`RunRecord::cost_usd`, the agent protocol's
//! `cost_usd`). That's a running total for the agent's *session*, so a run's
//! spend is the increase over the previous record of the same session (same
//! agent and run key — Hermes keys sessions on the thread); a total that
//! went down means a new session, counted whole.
//!
//! At `WARN_AT` of the limit you get a desktop notification and a bar over
//! the composer; over it, new tasks to that agent wait for "Send anyway"
//! (once per period). Commands (`!stop`, `/usage`…) always go through.

use std::collections::{HashMap, HashSet};

use iced::widget::{container, row, text, Space};
use iced::{Alignment, Element, Fill, Task, Theme};
use serde::{Deserialize, Serialize};

use crate::message::Message;
use crate::state::FluxChat;
use crate::ui::{self, Tone};

/// Share of the limit that warns.
pub(crate) const WARN_AT: f64 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Period {
    Day,
    Week,
    #[default]
    Month,
}

impl Period {
    pub const ALL: [Period; 3] = [Period::Day, Period::Week, Period::Month];

    pub fn label(self) -> &'static str {
        match self {
            Period::Day => "Day",
            Period::Week => "Week",
            Period::Month => "Month",
        }
    }

    /// "today", "this week", "this month".
    pub fn this(self) -> &'static str {
        match self {
            Period::Day => "today",
            Period::Week => "this week",
            Period::Month => "this month",
        }
    }

    /// Start of the period containing `now_ms`, local time (ms). Weeks start
    /// on Monday.
    pub fn start(self, now_ms: u64) -> u64 {
        use chrono::{Datelike, Local, NaiveTime, TimeZone};
        let Some(now) = Local.timestamp_millis_opt(now_ms as i64).single() else { return now_ms };
        let day = now.date_naive();
        let first = match self {
            Period::Day => day,
            Period::Week => day - chrono::Days::new(day.weekday().num_days_from_monday() as u64),
            Period::Month => day.with_day(1).unwrap_or(day),
        };
        Local.from_local_datetime(&first.and_time(NaiveTime::MIN))
            .earliest()
            .map(|t| t.timestamp_millis().max(0) as u64)
            .unwrap_or(now_ms)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub usd: f64,
    pub period: Period,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Fine,
    Near,
    Over,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Status {
    pub spent: f64,
    pub budget: Budget,
    pub since: u64,
    pub level: Level,
}

/// Budget state that lasts the session.
#[derive(Debug, Default)]
pub struct BudgetState {
    /// (agent, period start, level) already warned about.
    warned: HashSet<(String, u64, Level)>,
    /// (agent, period start) you chose to keep sending to over the limit.
    acked: HashSet<(String, u64)>,
    /// A view whose send is waiting on "Send anyway".
    pub(crate) hold: Option<String>,
    /// Limits being typed in Settings, by agent.
    pub(crate) inputs: HashMap<String, String>,
}

pub fn format_usd(c: f64) -> String {
    format!("${c:.2}")
}

/// "$0.18" / "0.18" / "$1,204.50" → dollars.
pub fn parse_usd(s: &str) -> Option<f64> {
    let t = s.trim().trim_start_matches('$').replace(',', "");
    t.parse::<f64>().ok().filter(|c| c.is_finite() && *c >= 0.0)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl FluxChat {
    /// What `agent` spent in runs that started at or after `since` (ms).
    pub(crate) fn spent(&self, agent: &str, since: u64) -> f64 {
        let mut recs: Vec<_> = self.run_history.iter()
            .filter(|r| r.agent == agent && r.cost_usd.is_some())
            .collect();
        recs.sort_by_key(|r| r.started);
        let mut last: HashMap<&str, f64> = HashMap::new();
        let mut total = 0.0;
        for r in recs {
            let cost = r.cost_usd.unwrap_or(0.0);
            let prev = last.insert(r.key.as_str(), cost);
            let run = match prev {
                Some(p) if cost >= p => cost - p,
                _ => cost,
            };
            if r.started >= since {
                total += run;
            }
        }
        total
    }

    pub(crate) fn budget_status(&self, agent: &str) -> Option<Status> {
        let budget = *self.prefs.agent_budgets.get(agent)?;
        if budget.usd <= 0.0 {
            return None;
        }
        let since = budget.period.start(now_ms());
        let spent = self.spent(agent, since);
        let level = if spent >= budget.usd {
            Level::Over
        } else if spent >= budget.usd * WARN_AT {
            Level::Near
        } else {
            Level::Fine
        };
        Some(Status { spent, budget, since, level })
    }

    /// After `agent`'s spend changed: a notification the first time this
    /// period it nears or passes its limit.
    pub(crate) fn check_budget(&mut self, agent: &str) {
        let Some(st) = self.budget_status(agent) else { return };
        if st.level == Level::Fine || !self.budgets.warned.insert((agent.to_string(), st.since, st.level)) {
            return;
        }
        if !self.notifications_enabled {
            return;
        }
        let name = self.agent_display_name(agent);
        let what = if st.level == Level::Over { "is over its budget" } else { "is nearing its budget" };
        crate::update::show_notification(
            format!("{name} {what}"),
            format!("{} of {} {}", format_usd(st.spent), format_usd(st.budget.usd), st.budget.period.this()),
            self.notify_sound,
            None,
        );
    }

    /// An agent's name from any room's members (Settings has no room).
    fn agent_display_name(&self, agent: &str) -> String {
        self.members_by_room.values().flatten()
            .find(|m| m.user_id == agent)
            .map(|m| m.display_name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| agent.to_string())
    }

    /// Whether sending `text` in view `key` must wait for "Send anyway": a
    /// task (not a command) to an agent over its limit, not waved through
    /// this period.
    pub(crate) fn budget_blocks(&self, key: &str, text: &str) -> bool {
        let t = text.trim_start();
        if t.starts_with('/') || t.starts_with('!') {
            return false;
        }
        let Some(agent) = self.agent_in(key) else { return false };
        self.budget_status(agent).is_some_and(|st| {
            st.level == Level::Over && !self.budgets.acked.contains(&(agent.to_string(), st.since))
        })
    }

    pub(crate) fn handle_budget_message(&mut self, message: Message) -> Option<Task<Message>> {
        match message {
            Message::BudgetSendAnyway(key) => {
                if let Some(agent) = self.agent_in(&key).map(str::to_string) {
                    if let Some(st) = self.budget_status(&agent) {
                        self.budgets.acked.insert((agent, st.since));
                    }
                }
                self.budgets.hold = None;
                let send = if self.selected_room_id.as_deref() == Some(key.as_str()) {
                    Message::SendMessage
                } else {
                    Message::PaneSend(key)
                };
                Some(self.update(send))
            }
            Message::BudgetCancel => {
                self.budgets.hold = None;
                Some(Task::none())
            }
            Message::BudgetInput(agent, value) => {
                let mut p = self.prefs.clone();
                if value.trim().is_empty() {
                    p.agent_budgets.remove(&agent);
                } else if let Some(usd) = parse_usd(&value).filter(|u| *u > 0.0) {
                    let period = p.agent_budgets.get(&agent).map(|b| b.period).unwrap_or_default();
                    p.agent_budgets.insert(agent.clone(), Budget { usd, period });
                }
                self.budgets.inputs.insert(agent, value);
                Some(self.update(Message::SetPrefs(Box::new(p))))
            }
            Message::BudgetPeriod(agent, period) => {
                let mut p = self.prefs.clone();
                if let Some(b) = p.agent_budgets.get_mut(&agent) {
                    b.period = period;
                }
                Some(self.update(Message::SetPrefs(Box::new(p))))
            }
            _ => None,
        }
    }

    /// Over the composer when the view's agent is near or over its limit;
    /// with Send anyway / Cancel when a send is waiting on it.
    pub(crate) fn view_budget_bar(&self, key: &str) -> Option<Element<'_, Message>> {
        let agent = self.agent_in(key)?;
        let st = self.budget_status(agent).filter(|s| s.level != Level::Fine)?;
        let colors = crate::theme::colors();
        let over = st.level == Level::Over;
        let color = if over { colors.red } else { colors.yellow };
        let name = self.agent_name(key, agent);
        let held = self.budgets.hold.as_deref() == Some(key);
        let what = if held {
            format!("{name} is over its budget ({} of {} {}). Send anyway?",
                format_usd(st.spent), format_usd(st.budget.usd), st.budget.period.this())
        } else if over {
            format!("{name} is over its budget: {} of {} {}", format_usd(st.spent), format_usd(st.budget.usd), st.budget.period.this())
        } else {
            format!("{name} has used {} of its {} budget {}", format_usd(st.spent), format_usd(st.budget.usd), st.budget.period.this())
        };
        let mut r = row![
            ui::icon(crate::icons::SVG_ALERT, 14.0, color),
            text(what).font(self.sidebar_font()).size(12).color(colors.text_normal)
                .wrapping(iced::widget::text::Wrapping::None),
            Space::new().width(Fill),
        ].spacing(8).align_y(Alignment::Center);
        if held {
            r = r.push(ui::btn("Send anyway", Tone::Danger, Some(Message::BudgetSendAnyway(key.to_string()))))
                .push(ui::btn("Cancel", Tone::Ghost, Some(Message::BudgetCancel)));
        } else {
            r = r.push(ui::btn("Change limit", Tone::Ghost, Some(Message::OpenModal(crate::state::Modal::Settings { tab: "preferences".into() }))));
        }
        Some(container(r)
            .padding([4, 12])
            .width(Fill)
            .clip(true)
            .style(move |_: &Theme| container::Style {
                background: Some(crate::theme::with_alpha(color, 0.10).into()),
                ..Default::default()
            })
            .into())
    }

    /// Settings → Preferences → AI agents: an agent's limit and period.
    pub(crate) fn view_budget_setting<'a>(&self, agent: &str) -> Element<'a, Message> {
        let colors = crate::theme::colors();
        let budget = self.prefs.agent_budgets.get(agent).copied();
        let typed = self.budgets.inputs.get(agent).cloned()
            .unwrap_or_else(|| budget.map(|b| format!("{:.2}", b.usd)).unwrap_or_default());
        let user = agent.to_string();
        let input = ui::input("No limit", &typed, move |v| Message::BudgetInput(user.clone(), v)).width(iced::Length::Fixed(110.0));
        let period = budget.map(|b| b.period).unwrap_or_default();
        let periods = ui::segmented(Period::ALL.iter()
            .map(|p| (p.label().to_string(), budget.map(|_| Message::BudgetPeriod(agent.to_string(), *p)), *p == period))
            .collect());
        let hint = match self.budget_status(agent) {
            Some(st) => format!("Spent {} of {} {}. Warns at {}%; past the limit, new tasks ask first.",
                format_usd(st.spent), format_usd(st.budget.usd), st.budget.period.this(), (WARN_AT * 100.0) as u32),
            None => "Spend limit in US dollars, from the cost the agent reports (agent protocol)".to_string(),
        };
        iced::widget::column![
            row![
                text("Budget $").font(crate::view::MONO).size(13).color(colors.text_normal),
                input,
                text("per").font(crate::view::MONO).size(13).color(colors.text_dim),
                container(periods).width(iced::Length::Fixed(210.0)),
            ].spacing(8).align_y(Alignment::Center),
            ui::hint(hint),
        ]
        .spacing(4)
        .into()
    }

    /// Run key → the room's agent menu line for the budget, if one is set.
    pub(crate) fn budget_menu_label(&self, key: &str) -> Option<String> {
        let agent = self.agent_in(key)?;
        let st = self.budget_status(agent)?;
        Some(format!("Budget: {} of {} {}", format_usd(st.spent), format_usd(st.budget.usd), st.budget.period.this()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::RunRecord;

    fn rec(key: &str, started: u64, cost: f64) -> RunRecord {
        RunRecord {
            key: key.into(), room: "!r:x".into(), agent: "@bot:x".into(), title: "t".into(),
            state: "done".into(), started, ended: Some(started + 1), steps: 0, model: None,
            cost: Some(format_usd(cost)), cost_usd: Some(cost),
        }
    }

    #[test]
    fn spend_adds_up_session_increases() {
        let mut app = crate::panes::flow_tests::app();
        app.run_history = vec![
            rec("thread::!r:x::$a", 1_000, 0.50),
            rec("thread::!r:x::$b", 2_000, 0.20),
            // Same session as $a: 0.50 → 0.80 is 0.30 more.
            rec("thread::!r:x::$a", 3_000, 0.80),
            // A new session (total went down): counted whole.
            rec("thread::!r:x::$a", 4_000, 0.10),
        ];
        assert!((app.spent("@bot:x", 0) - 1.10).abs() < 1e-9);
        // Only runs in the period, but still as increases.
        assert!((app.spent("@bot:x", 2_500) - 0.40).abs() < 1e-9);
        assert_eq!(app.spent("@other:x", 0), 0.0);
    }

    #[tokio::test]
    async fn over_the_limit_tasks_wait_but_commands_dont() {
        let mut app = crate::panes::flow_tests::app();
        app.prefs.agents = vec!["@bot:x".into()];
        app.messages_by_room.insert("!r:x".into(), vec![fluxchat_core::MessageInfo {
            event_id: "$1".into(), sender: "@bot:x".into(), body: "hi".into(), ..Default::default()
        }]);
        let now = now_ms();
        app.run_history = vec![rec("!r:x", now - 1_000, 4.0)];
        app.prefs.agent_budgets.insert("@bot:x".into(), Budget { usd: 5.0, period: Period::Day });
        assert_eq!(app.budget_status("@bot:x").map(|s| s.level), Some(Level::Near));
        assert!(!app.budget_blocks("!r:x", "do the thing"));
        app.run_history[0].cost_usd = Some(6.0);
        assert_eq!(app.budget_status("@bot:x").map(|s| s.level), Some(Level::Over));
        assert!(app.budget_blocks("!r:x", "do the thing"));
        assert!(!app.budget_blocks("!r:x", "!stop"));
        assert!(!app.budget_blocks("!r:x", "/usage"));
        // Waved through: the rest of the period sends as usual.
        let st = app.budget_status("@bot:x").unwrap();
        app.budgets.acked.insert(("@bot:x".into(), st.since));
        assert!(!app.budget_blocks("!r:x", "do the thing"));
    }

    #[test]
    fn periods_start_at_local_midnight() {
        use chrono::{Datelike, Local, TimeZone, Timelike};
        let now = now_ms();
        for p in Period::ALL {
            let start = Local.timestamp_millis_opt(p.start(now) as i64).single().unwrap();
            assert_eq!((start.hour(), start.minute()), (0, 0));
            assert!(p.start(now) <= now);
            match p {
                Period::Week => assert_eq!(start.weekday(), chrono::Weekday::Mon),
                Period::Month => assert_eq!(start.day(), 1),
                Period::Day => assert!(now - p.start(now) < 25 * 3600 * 1000),
            }
        }
        assert_eq!(parse_usd("$1,204.50"), Some(1204.5));
        assert_eq!(parse_usd("-1"), None);
    }
}
