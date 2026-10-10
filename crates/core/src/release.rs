//! Desktop self-discovery of updates: fetch the newest GitHub Release,
//! compare its tag to the version this binary was compiled with, and hold
//! the result for the status-bar chip. Checks are rate-limited to once a
//! day (the timestamp lives in the settings table, so failures count too —
//! an offline box retries tomorrow, not per launch). The port of Android's
//! `Update.kt`: same endpoint, same rate-limit, same tag comparison.
//!
//! No self-update installer: the chip opens the release page and the user
//! downloads the artifact themselves. That's honest for unsigned macOS
//! builds, and the release page is also where the caveats live.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::db::AppDb;
use crate::error::AppError;

/// Where releases live: the repo page the chip opens, and the API it checks.
pub const RELEASES_PAGE: &str = "https://github.com/phrac/fluxchat/releases";
const DEFAULT_LATEST_API: &str = "https://api.github.com/repos/phrac/fluxchat/releases/latest";

/// The API to check. Defaults to GitHub Releases; `FLUXCHAT_RELEASES_URL`
/// points it at a stand-in — used to prove the update path while the repo
/// is private (the real endpoint 404s unauthenticated there), and available
/// for the same reason in tests. The chip still opens the real release page.
fn latest_api_url() -> String {
    std::env::var("FLUXCHAT_RELEASES_URL").unwrap_or_else(|_| DEFAULT_LATEST_API.to_string())
}

/// The settings row holding the epoch-ms of the last attempt (Android:
/// `update_last_check`).
const LAST_CHECK_KEY: &str = "update_last_check";
const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;

/// The version this binary was built as (`crates/app/Cargo.toml`).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dot-separated versions compared part by part (numeric when both sides
/// are numeric, else lexicographic); a longer version wins ties, so
/// 0.3 > 0.2.9 and 0.2.2.1 > 0.2.2. Mirrors `Update.isNewer` on Android.
pub fn is_newer(current: &str, latest: &str) -> bool {
    let a: Vec<&str> = current.split('.').collect();
    let b: Vec<&str> = latest.split('.').collect();
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied();
        let y = b.get(i).copied();
        let Some(y) = y else { return false }; // current has more parts: current is newer
        let Some(x) = x else { return true }; // latest has more parts
        let c = if x.chars().all(|c| c.is_ascii_digit()) && y.chars().all(|c| c.is_ascii_digit()) {
            x.parse::<u64>().ok().cmp(&y.parse::<u64>().ok())
        } else {
            x.cmp(y)
        };
        if c != std::cmp::Ordering::Equal {
            return c == std::cmp::Ordering::Less;
        }
    }
    false // equal
}

/// Release tag (minus the `v`) newer than `APP_VERSION`, if `latest_tag`
/// is one. The single place the tag prefix is stripped.
pub fn available_update(latest_tag: &str) -> Option<String> {
    let name = latest_tag.trim().trim_start_matches('v');
    is_newer(APP_VERSION, name).then(|| name.to_string())
}

/// Whether the rate-limit window lets us hit the API. The last-check
/// stamp is written *before* the attempt, so failures rate-limit too.
pub fn due_now(db: &AppDb, force: bool) -> bool {
    if force {
        return true;
    }
    let last = db
        .get_setting(LAST_CHECK_KEY)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    now_ms().saturating_sub(last) >= CHECK_INTERVAL_MS
}

/// Stamp an attempt (call even when the fetch fails).
pub fn stamp_attempt(db: &AppDb) {
    let _ = db.set_setting(LAST_CHECK_KEY, &now_ms().to_string());
}

/// GET the newest release's tag. The repo is private today, so this 404s
/// until it goes public — a silent path, not an error to surface.
pub async fn fetch_latest_tag() -> Result<String, AppError> {
    let resp = reqwest::Client::new()
        .get(latest_api_url())
        .header("Accept", "application/vnd.github+json")
        .header(reqwest::header::USER_AGENT, concat!("fluxchat/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| AppError::Other(format!("releases/latest: {e}")))?;
    if resp.status() != reqwest::StatusCode::OK {
        return Err(AppError::Other(format!("releases/latest: HTTP {}", resp.status())));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Other(format!("releases/latest: bad json: {e}")))?;
    body.get("tag_name")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::Other("releases/latest: no tag_name".into()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> (AppDb, std::path::PathBuf) {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "fluxchat-release-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (AppDb::open(&dir.join("t.db")).unwrap(), dir)
    }

    #[test]
    fn is_newer_matches_android_semantics() {
        assert!(is_newer("0.2.1", "0.2.2"));
        assert!(is_newer("0.2.9", "0.3")); // numeric parts, not lexicographic
        assert!(is_newer("0.2.2", "0.2.2.1")); // more parts wins ties
        assert!(!is_newer("0.2.2.1", "0.2.2"));
        assert!(!is_newer("0.2.2", "0.2.2"));
        assert!(!is_newer("0.2.2", "0.2.1"));
        assert!(!is_newer("1.0", "1.0"));
        assert!(is_newer("1.9.9", "2.0.0"));
    }

    #[test]
    fn available_update_compares_against_the_compiled_version() {
        // Same version as this build: never newer than itself.
        assert_eq!(available_update(&format!("v{APP_VERSION}")), None);
        assert_eq!(available_update(APP_VERSION), None);
        assert_eq!(available_update("v0.0.0"), None);
        // A version ahead of anything a released crate has ever carried.
        assert_eq!(available_update("v999999.0"), Some("999999.0".into()));
    }

    #[test]
    fn rate_limit_gates_attempts() {
        let (db, dir) = temp_db("rate");
        assert!(due_now(&db, false)); // never checked
        stamp_attempt(&db);
        assert!(!due_now(&db, false)); // just checked
        assert!(due_now(&db, true)); // force bypasses the window
        // A stamp from two days ago is due again.
        db.set_setting(LAST_CHECK_KEY, &(now_ms() - CHECK_INTERVAL_MS - 1000).to_string())
            .unwrap();
        assert!(due_now(&db, false));
        // Junk in the row behaves like "never checked".
        db.set_setting(LAST_CHECK_KEY, "not-a-number").unwrap();
        assert!(due_now(&db, false));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
