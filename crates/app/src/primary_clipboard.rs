//! PRIMARY selection mirror for X11/Wayland.
//!
//! Iced's clipboard API only touches the CLIPBOARD selection (Ctrl+C / Ctrl+V).
//! Terminals and traditional X apps paste from PRIMARY on middle-click /
//! Shift+Insert. We mirror any finished drag-selection into PRIMARY here so
//! that flow works.

#[cfg(target_os = "linux")]
pub fn set(text: String) {
    use std::sync::{Mutex, OnceLock};

    use arboard::{Clipboard, LinuxClipboardKind, SetExtLinux};

    static CLIPBOARD: OnceLock<Mutex<Option<Clipboard>>> = OnceLock::new();
    let cell = CLIPBOARD.get_or_init(|| Mutex::new(Clipboard::new().ok()));

    let Ok(mut guard) = cell.lock() else { return };
    let Some(cb) = guard.as_mut() else { return };
    if let Err(e) = cb.set().clipboard(LinuxClipboardKind::Primary).text(text) {
        tracing::warn!("failed to write PRIMARY selection: {e}");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set(_text: String) {}
