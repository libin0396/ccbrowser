// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 Bogdan Shapovalov and the Fury authors

//! Where the agent's own account of itself goes.
//!
//! WHY THIS EXISTS. The desktop application starts the agent with all three
//! standard streams on null (desktop/src-tauri/src/agent.rs) — it has to, or
//! Windows draws a console window over the application. Until this file, that
//! meant every line the agent ever wrote went nowhere on the machines that
//! matter: a person whose core would not install had a screenshot and nothing
//! else, and twice on 22.09.2026 that was the whole of the evidence.
//!
//! WHAT IT IS NOT. Not telemetry: nothing here leaves the machine, and there is
//! no code in this program that would send it. The file sits in the data
//! directory, which is owner-only and already holds the profile database —
//! something anyone who can read the log could read instead, and more of it.
//!
//! WHAT IT COSTS, and it is a real cost for this product: a persistent record
//! of when this machine ran profiles. It is capped and it is coarse — `info`,
//! which is "downloading the core", "the profile closed", not a trace of
//! browsing — and it can be deleted at any time; the agent makes a new one and
//! carries on. FURY_LOG=off turns it off entirely.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Two of these: the one being written and the one before it.
///
/// Four megabytes is thousands of lines at the level this logs, which covers
/// weeks of ordinary use and, more to the point, covers the several days
/// between somebody hitting a problem and somebody asking them for the file.
const MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Starts logging to stderr and, unless turned off, to a file.
///
/// Returns the file it will write to, so the caller can say where it is.
pub fn start() -> Option<PathBuf> {
    let filter = || {
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "fury_agent=info".into())
    };

    // A terminal still gets what it always got. The CLI is used by people who
    // are watching it run, and a command whose output moved into a file it does
    // not mention would be worse than one that never had a file.
    let to_stderr = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);

    let path = match std::env::var("FURY_LOG").as_deref() {
        Ok("off") => None,
        _ => open().ok(),
    };

    match path {
        Some(path) => {
            let for_writer = path.clone();
            let gate = Arc::new(Mutex::new(()));
            tracing_subscriber::registry()
                .with(filter())
                .with(to_stderr)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        // One buffer per event: serialize rotation and append
                        // together, without retaining an open Windows handle.
                        .with_writer(move || BoundedWriter {
                            path: for_writer.clone(),
                            gate: gate.clone(),
                            bytes: Vec::new(),
                            truncated: false,
                        }),
                )
                .init();
            Some(path)
        }
        None => {
            tracing_subscriber::registry()
                .with(filter())
                .with(to_stderr)
                .init();
            None
        }
    }
}

/// Makes the directory, rotates what is there, and hands back the path.
fn open() -> std::io::Result<PathBuf> {
    let path = crate::paths::log_file();
    let dir = path.parent().expect("log_file always has a parent");
    std::fs::create_dir_all(dir)?;
    // The data directory is owner-only and this is inside it; said again here
    // because a directory created by this function on a machine where the data
    // directory already existed would otherwise inherit whatever the default is.
    let _ = fury_platform::perms::owner_only_dir(dir);

    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        rotate(&path)?;
    }
    Ok(path)
}

/// Keep only the last run segment. Failure is propagated so callers never
/// append past the bound when rotation is impossible.
fn rotate(path: &std::path::Path) -> std::io::Result<()> {
    let previous = path.with_extension("log.1");
    match std::fs::remove_file(&previous) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::fs::rename(path, previous)
}

const MAX_EVENT_BYTES: usize = 64 * 1024;
const TRUNCATED: &[u8] = b"
[event truncated at 64 KiB]
";

struct BoundedWriter {
    path: PathBuf,
    gate: Arc<Mutex<()>>,
    bytes: Vec<u8>,
    truncated: bool,
}

impl BoundedWriter {
    fn commit(&mut self) -> std::io::Result<()> {
        if self.bytes.is_empty() {
            return Ok(());
        }
        let _guard = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        if self.truncated {
            self.bytes.extend_from_slice(TRUNCATED);
            self.truncated = false;
        }
        let len = self.path.metadata().map(|m| m.len()).unwrap_or(0);
        if len.saturating_add(self.bytes.len() as u64) > MAX_BYTES {
            rotate(&self.path)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // A logging failure must not stop profile operations, but also must
        // not allow unbounded append. Drop discards an unwritten event.
        options.open(&self.path)?.write_all(&self.bytes)?;
        self.bytes.clear();
        self.truncated = false;
        Ok(())
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let available = MAX_EVENT_BYTES.saturating_sub(self.bytes.len());
        let n = available.min(buf.len());
        self.bytes.extend_from_slice(&buf[..n]);
        self.truncated |= n < buf.len();
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.commit()
    }
}

impl Drop for BoundedWriter {
    fn drop(&mut self) {
        let _ = self.commit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_that_has_grown_past_the_cap_is_kept_as_the_previous_one() {
        let _guard = crate::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let home = std::env::temp_dir().join("fury-log-rotate-test");
        std::fs::remove_dir_all(&home).ok();
        // SAFETY: the mutex above is what makes this single-threaded.
        unsafe { std::env::set_var("FURY_HOME", &home) };

        // First run: the directory does not exist yet, which is the ordinary
        // case on a machine that has just installed the application.
        let path = open().expect("a log in a fresh data directory");
        assert_eq!(path, crate::paths::log_file());
        std::fs::write(&path, vec![b'x'; (MAX_BYTES + 1) as usize]).unwrap();

        let again = open().expect("a log beside a full one");
        let previous = path.with_extension("log.1");
        assert!(previous.exists(), "the full one is kept as .1");
        assert!(
            !again.exists() || again.metadata().unwrap().len() == 0,
            "and the next line starts a new file"
        );

        // A third run must not lose the rotated copy to a second rotation of an
        // empty file, and must not stack .1.1 files either.
        let _ = open().expect("a third run");
        assert!(previous.exists());
        assert!(!previous.with_extension("log.1").exists());

        unsafe { std::env::remove_var("FURY_HOME") };
        std::fs::remove_dir_all(&home).ok();
    }
    fn writer(path: &std::path::Path, gate: &Arc<Mutex<()>>) -> BoundedWriter {
        BoundedWriter {
            path: path.to_owned(),
            gate: gate.clone(),
            bytes: vec![],
            truncated: false,
        }
    }

    #[test]
    fn runtime_rotation_is_bounded_and_preserves_recent_events() {
        let home = std::env::temp_dir().join(format!("fury-runtime-log-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("agent.log");
        let gate = Arc::new(Mutex::new(()));
        for _ in 0..150 {
            let mut w = writer(&path, &gate);
            w.write_all(&vec![b'a'; MAX_EVENT_BYTES]).unwrap();
        }
        {
            writer(&path, &gate)
                .write_all(
                    b"latest-event
",
                )
                .unwrap();
        }
        assert!(path.metadata().unwrap().len() <= MAX_BYTES);
        assert!(path.with_extension("log.1").metadata().unwrap().len() <= MAX_BYTES);
        assert!(std::fs::read(&path).unwrap().ends_with(
            b"latest-event
"
        ));
        assert_eq!(std::fs::read_dir(&home).unwrap().count(), 2);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn oversized_events_and_rotation_failure_do_not_grow_the_log() {
        let home = std::env::temp_dir().join(format!("fury-bounded-log-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("agent.log");
        let gate = Arc::new(Mutex::new(()));
        {
            writer(&path, &gate)
                .write_all(&vec![b'x'; MAX_EVENT_BYTES * 3])
                .unwrap();
        }
        assert_eq!(
            path.metadata().unwrap().len() as usize,
            MAX_EVENT_BYTES + TRUNCATED.len()
        );
        assert!(std::fs::read(&path).unwrap().ends_with(TRUNCATED));
        std::fs::write(&path, vec![b'x'; MAX_BYTES as usize]).unwrap();
        // A directory occupying the rotation target cannot be removed as a file.
        std::fs::create_dir(path.with_extension("log.1")).unwrap();
        {
            writer(&path, &gate).write_all(b"must not append").unwrap();
        }
        assert_eq!(path.metadata().unwrap().len(), MAX_BYTES);
        std::fs::remove_dir_all(home).unwrap();
    }
}
