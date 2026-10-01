//! Opt-in slow-frame logging for performance reports. Run with
//! `FLUXCHAT_TIMING=1` and every `update()` slower than `SLOW_UPDATE` and
//! every `view()` slower than `SLOW_VIEW` is logged (target
//! `fluxchat::timing`) with the message that caused it. Off by default: then
//! it's one cached bool check per call.

use std::cell::RefCell;
use std::fmt::Write;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use iced::{Element, Task};

use crate::message::Message;
use crate::state::FluxChat;

const SLOW_UPDATE: Duration = Duration::from_millis(4);
const SLOW_VIEW: Duration = Duration::from_millis(8);

static ENABLED: LazyLock<bool> = LazyLock::new(|| std::env::var_os("FLUXCHAT_TIMING").is_some());

thread_local! {
    /// The last message handled, for attributing a slow `view()`.
    static LAST_MESSAGE: RefCell<String> = const { RefCell::new(String::new()) };
}

pub(crate) fn update(state: &mut FluxChat, message: Message) -> Task<Message> {
    if !*ENABLED {
        return state.update(message);
    }
    let name = variant_name(&message);
    let start = Instant::now();
    let task = state.update(message);
    let took = start.elapsed();
    if took >= SLOW_UPDATE {
        tracing::info!(target: "fluxchat::timing", "update {name}: {:.1} ms", took.as_secs_f64() * 1000.0);
    }
    LAST_MESSAGE.with(|last| *last.borrow_mut() = name);
    task
}

pub(crate) fn view(state: &FluxChat) -> Element<'_, Message> {
    if !*ENABLED {
        return state.view();
    }
    let start = Instant::now();
    let element = state.view();
    let took = start.elapsed();
    if took >= SLOW_VIEW {
        LAST_MESSAGE.with(|last| {
            tracing::info!(target: "fluxchat::timing", "view after {}: {:.1} ms", last.borrow(), took.as_secs_f64() * 1000.0);
        });
    }
    element
}

/// `Message::SyncEvent(SyncEvent::MessagesUpdated(..))` → `SyncEvent::MessagesUpdated`,
/// without formatting the payload: the writer stops `Debug` at the first
/// field (a `MessagesUpdated` can carry thousands of rows).
fn variant_name(message: &Message) -> String {
    struct Head {
        out: String,
        depth: usize,
    }
    impl Write for Head {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            for c in s.chars() {
                match c {
                    '(' if self.depth == 0 => {
                        self.depth = 1;
                        self.out.push_str("::");
                    }
                    c if c.is_alphanumeric() || c == '_' => self.out.push(c),
                    _ => return Err(std::fmt::Error),
                }
            }
            Ok(())
        }
    }
    let mut head = Head { out: String::new(), depth: 0 };
    let _ = write!(head, "{message:?}");
    head.out.trim_end_matches("::").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_name_skips_the_payload() {
        assert_eq!(variant_name(&Message::Tick), "Tick");
        assert_eq!(variant_name(&Message::ImageResolved("mxc://a/b".into(), vec![1, 2, 3])), "ImageResolved");
        assert_eq!(
            variant_name(&Message::SyncEvent(crate::message::SyncEvent::SyncLag(5))),
            "SyncEvent::SyncLag",
        );
    }
}
