//! What the program does, written for whoever reads after it (ADR-0007): one JSON line per
//! step in the user's state folder, a file a day, under the id of the making the shell passed.
//! Never what was said: no text of the meeting, no context, no key, no path to a meeting
//! beyond its folder's name.
//!
//! A log that cannot be written is a log missing, never minutes lost: the program goes on.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use tracing::{Level, span::EnteredSpan};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// How long a day's file is kept.
const KEPT: Duration = Duration::from_secs(14 * 24 * 3600);
/// What the folder may weigh, every program's files together.
const WEIGHT: u64 = 50 * 1024 * 1024;

/// Where the logs go: `WHOLEHEARD_LOG_DIR` when given, else the system's place for them. A
/// path that is not absolute is not taken: it would depend on where the program was started.
fn folder() -> Option<PathBuf> {
    let given = |name| std::env::var_os(name).map(PathBuf::from).filter(|path| path.is_absolute());
    if let Some(folder) = given("WHOLEHEARD_LOG_DIR") {
        return Some(folder);
    }
    if cfg!(target_os = "macos") {
        given("HOME").map(|home| home.join("Library/Logs/Wholeheard"))
    } else if cfg!(windows) {
        given("LOCALAPPDATA").map(|local| local.join("Wholeheard").join("logs"))
    } else {
        given("XDG_STATE_HOME")
            .or_else(|| given("HOME").map(|home| home.join(".local/state")))
            .map(|state| state.join("wholeheard").join("logs"))
    }
}

/// The making's id the shell passed in `WHOLEHEARD_TRACE`, or one of this run's own.
fn trace() -> String {
    match std::env::var("WHOLEHEARD_TRACE") {
        Ok(id) if (1..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') => id,
        _ => {
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
            format!("{:x}-{:x}", now.as_millis(), std::process::id())
        }
    }
}

/// `info` unless `WHOLEHEARD_LOG` names a level — or, for whoever knows them, a filter.
/// A level reaches this program's own lines only: a library's are never asked for by it.
fn filter() -> EnvFilter {
    let own = env!("CARGO_CRATE_NAME");
    let asked = std::env::var("WHOLEHEARD_LOG").unwrap_or_default();
    let directives = match asked.trim() {
        level @ ("error" | "warn" | "info" | "debug" | "trace") => format!("off,{own}={level}"),
        filter if filter.contains('=') => filter.to_string(),
        _ => format!("off,{own}=info"),
    };
    EnvFilter::try_new(directives).unwrap_or_else(|_| EnvFilter::new(format!("off,{own}=info")))
}

/// A program's name as its files start: lower-case letters, and dashes between.
fn is_program(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

/// A day as the files name it: `YYYY-MM-DD`.
fn is_date(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes
            .iter()
            .enumerate()
            .all(|(at, b)| if at == 4 || at == 7 { *b == b'-' } else { b.is_ascii_digit() })
}

/// Removes the day files — every program's, `<program>.<date>.jsonl` — written more than
/// fourteen days ago, then the oldest until the folder weighs less than fifty megabytes.
/// Nothing else in the folder is touched.
pub fn prune(folder: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut days: Vec<(SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let parts: Vec<&str> = name.split('.').collect();
            matches!(parts[..], [program, date, "jsonl"] if is_program(program) && is_date(date))
        })
        .filter_map(|entry| {
            let found = entry.metadata().ok().filter(std::fs::Metadata::is_file)?;
            Some((found.modified().ok()?, found.len(), entry.path()))
        })
        .collect();
    days.sort();
    let mut weight: u64 = days.iter().map(|(_, size, _)| size).sum();
    for (written, size, path) in days {
        let old = now.duration_since(written).is_ok_and(|age| age > KEPT);
        if (old || weight > WEIGHT) && std::fs::remove_file(&path).is_ok() {
            weight -= size;
        }
    }
}

/// Starts the log of `program`; what is logged until the value returned is dropped belongs
/// to this run, its version and its making's id — at every level, a failure's line included.
pub fn start(program: &'static str) -> EnteredSpan {
    let appender = folder().and_then(|folder| {
        std::fs::create_dir_all(&folder).ok()?;
        prune(&folder, SystemTime::now());
        RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(program)
            .filename_suffix("jsonl")
            .build(folder)
            .ok()
    });
    if let Some(appender) = appender {
        let _ = tracing_subscriber::fmt()
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(false)
            .with_target(false)
            .with_ansi(false)
            .with_env_filter(filter())
            .with_writer(appender)
            .try_init();
    }
    tracing::span!(Level::ERROR, "run", program, version = env!("CARGO_PKG_VERSION"), trace = %trace()).entered()
}
