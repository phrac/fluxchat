//! The app's log, kept in a file as well as printed to the terminal.
//!
//! Launched from the Finder / a desktop launcher there is no terminal, so
//! everything logged went nowhere — including the one line that says *why* a
//! message couldn't be decrypted (the SDK's "Failed to decrypt a non-pre-key
//! message with all available sessions", with the error per Olm session).
//! Chasing "Hermes's messages go undecryptable every few days" came down to
//! not having that line.
//!
//! One file, `logs/fluxchat.log` under the data directory, owner-only. When
//! it reaches [`MAX_BYTES`] it becomes `fluxchat.log.1` (replacing the last
//! one) and a new file starts, so the two together never hold much more than
//! twice that. It names user, room and device IDs — never message text at the
//! levels logged — and is emptied on sign-out.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A log file is rotated once it is this big.
pub const MAX_BYTES: u64 = 2 * 1024 * 1024;

const FILE_NAME: &str = "fluxchat.log";

struct Open {
    path: PathBuf,
    file: File,
    written: u64,
    max: u64,
}

static LOG: Mutex<Option<Open>> = Mutex::new(None);

/// Where the log files live (`<data dir>/fluxchat/logs`).
pub fn dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("fluxchat").join("logs"))
}

/// The current log file.
pub fn path() -> Option<PathBuf> {
    Some(dir()?.join(FILE_NAME))
}

fn previous(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

/// Open `path` for appending, moving it aside first when it has reached
/// `max` bytes. Returns the file and how much it already holds.
fn open_rotating(path: &Path, max: u64) -> io::Result<(File, u64)> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() >= max) {
        // Replaces the older one: two files at most.
        std::fs::rename(path, previous(path))?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    let written = file.metadata().map(|m| m.len()).unwrap_or(0);
    Ok((file, written))
}

/// Start logging to the file. `None` (and the terminal only) when it can't be
/// opened — a read-only home must not stop the app from starting.
pub fn init() -> Option<FileWriter> {
    init_at(path()?, MAX_BYTES)
}

fn init_at(path: PathBuf, max: u64) -> Option<FileWriter> {
    let (file, written) = open_rotating(&path, max).ok()?;
    *LOG.lock().unwrap_or_else(|e| e.into_inner()) = Some(Open { path, file, written, max });
    Some(FileWriter)
}

/// Empty the log (sign-out: it names the account's rooms and devices).
pub fn clear() {
    let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(open) = log.as_mut() {
        // Opened for appending, so writing carries on from the new end.
        let _ = open.file.set_len(0);
        open.written = 0;
        let _ = std::fs::remove_file(previous(&open.path));
    }
}

/// `tracing_subscriber`'s writer for the file.
#[derive(Clone, Copy)]
pub struct FileWriter;

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut log = LOG.lock().unwrap_or_else(|e| e.into_inner());
        let Some(open) = log.as_mut() else { return Ok(buf.len()) };
        if open.written >= open.max {
            // Full: start the next file. If that fails, keep writing here —
            // an oversized log beats a silent one.
            if let Ok((file, written)) = open_rotating(&open.path, open.max) {
                open.file = file;
                open.written = written;
            }
        }
        // A failed write is dropped: logging must never take the app down,
        // and tracing would only report it by logging.
        if open.file.write_all(buf).is_ok() {
            open.written += buf.len() as u64;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(open) = LOG.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = open.file.flush();
        }
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for FileWriter {
    type Writer = FileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The log is one process-wide static: these tests take turns.
    static TURN: Mutex<()> = Mutex::new(());

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fluxchat-logfile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("logs").join(FILE_NAME)
    }

    #[test]
    fn a_full_log_starts_a_new_file_and_keeps_one_old_one() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let path = scratch("rotate");
        let mut w = init_at(path.clone(), 64).expect("a log file in the temp dir");
        w.write_all(&[b'a'; 40]).unwrap();
        w.write_all(&[b'b'; 40]).unwrap();
        // 80 bytes: over the limit, so the next line opens a new file.
        w.write_all(b"next\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"next\n");
        assert_eq!(std::fs::metadata(previous(&path)).unwrap().len(), 80);
        // A second rotation replaces the old file rather than adding a third.
        w.write_all(&[b'c'; 70]).unwrap();
        w.write_all(b"again\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"again\n");
        assert_eq!(std::fs::metadata(previous(&path)).unwrap().len(), 75);
        let files = std::fs::read_dir(path.parent().unwrap()).unwrap().count();
        assert_eq!(files, 2);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn a_log_left_full_by_the_last_run_is_moved_aside_at_start() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let path = scratch("start");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, [b'x'; 100]).unwrap();
        let mut w = init_at(path.clone(), 64).unwrap();
        w.write_all(b"hello\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello\n");
        assert_eq!(std::fs::metadata(previous(&path)).unwrap().len(), 100);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn signing_out_empties_the_log_and_logging_carries_on() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let path = scratch("clear");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(previous(&path), b"the account before").unwrap();
        let mut w = init_at(path.clone(), 1024).unwrap();
        w.write_all(b"@someone:example.org did something\n").unwrap();
        clear();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        assert!(!previous(&path).exists());
        w.write_all(b"after\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"after\n");
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn the_log_is_readable_by_its_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let path = scratch("mode");
        let _w = init_at(path.clone(), 1024).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
    }
}
