//! What the shell does, written for whoever reads after it (ADR-0007): one JSON line per step
//! in the user's state folder, a file a day. Each making — a recording, a transcript, minutes,
//! a test — has an id of its own ([`making`]): the shell logs under it and passes it to the
//! program in `WHOLEHEARD_TRACE`, so that both logs read together.
//! Never what was said: no text of the meeting, no context, no key, no path to a meeting
//! beyond its folder's name.
//!
//! A log that cannot be written is a log missing, never a meeting lost: the shell goes on.

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
pub fn folder() -> Option<PathBuf> {
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

/// A new making: its id — unique on this machine, nothing of the meeting in it — and the span
/// its lines are logged in, so that they say `span.trace` as the program's own lines do.
pub fn making() -> (String, tracing::Span) {
    static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id = format!("{:x}-{:x}-{count}", now.as_millis(), std::process::id());
    let span = tracing::span!(Level::ERROR, "making", program = "shell", trace = %id);
    (id, span)
}

/// A program's sentence as the log may hold it: the folder of sessions it may name taken out —
/// a version of a program that says a path must not put whose meetings, and where, in a log.
/// Only the folder itself is taken out, never the start of another name: `/srv/Meet` leaves
/// `/srv/Meetings`, `/srv/Meet.bak` and `/data/srv/Meet` as they are.
pub fn without(said: &str, sessions: Option<&Path>) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let sessions = sessions.map(Path::to_string_lossy).unwrap_or_default();
    let sessions = sessions.trim_end_matches(separator);
    if sessions.is_empty() {
        return said.to_string();
    }
    let name = |c: char| c.is_alphanumeric() || "._-".contains(c);
    let (mut kept, mut rest) = (String::new(), said);
    while let Some(at) = rest.find(sessions) {
        let (before, after) = (&rest[..at], &rest[at + sessions.len()..]);
        kept.push_str(before);
        // Inside another path — `/data/srv/Meet` —, or going on with a name — `ings`, `.bak` —, it
        // is another folder; a sentence's last `.` is no name going on.
        let inside = kept.ends_with(name);
        let goes_on = after.trim_start_matches(['.', '_', '-']).starts_with(char::is_alphanumeric);
        kept.push_str(if inside || goes_on { sessions } else { "<sessions>" });
        rest = after;
    }
    kept.push_str(rest);
    kept
}

/// Logs a failure of the window's page, as it reports its own: its kind and the page's file it
/// was thrown in — names of the page's code —, and where. Its sentence is never taken, which may
/// hold a meeting's words; what is no such name is left out, empty.
pub fn page_failed(kind: &str, file: &str, line: u32, column: u32) {
    let named =
        |name: &str, marks: &str| (1..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || marks.contains(c));
    let file = file.rsplit('/').next().unwrap_or_default();
    let (kind, file) = (if named(kind, "") { kind } else { "" }, if named(file, "._-") { file } else { "" });
    tracing::error!(kind, file, line, column, "shell.page_failed");
}

/// `info` unless `WHOLEHEARD_LOG` names a level, `off` included — or, for whoever knows them, a filter.
/// A level reaches the shell's own lines only — its library's and its program's, both named
/// `wholeheard…` —: a library's are never asked for by it.
fn filter() -> EnvFilter {
    let own = "wholeheard";
    let asked = std::env::var("WHOLEHEARD_LOG").unwrap_or_default();
    let directives = match asked.trim() {
        level @ ("off" | "error" | "warn" | "info" | "debug" | "trace") => format!("off,{own}={level}"),
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
    // Turned off, the log leaves nothing: no file, nothing removed.
    let off = std::env::var("WHOLEHEARD_LOG").is_ok_and(|level| level.trim() == "off");
    let appender = folder().filter(|_| !off).and_then(|folder| {
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
            // A log that cannot be written says nothing on the error stream: what is said there
            // is the program's one sentence (its contract).
            .log_internal_errors(false)
            .try_init();
    }
    tracing::span!(Level::ERROR, "run", program, version = env!("CARGO_PKG_VERSION"), trace = %trace()).entered()
}

/// What is sent to whoever looks into a meeting's failure: every program's lines of that
/// meeting's makings — the ids the shell logged with its folder's name —, in the order they
/// were written, from the days kept. Nothing of another meeting; nothing of the meeting itself,
/// which no line holds.
pub fn diagnostic(folder: &Path, session: &str) -> String {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return String::new();
    };
    let lines: Vec<(serde_json::Value, String)> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let parts: Vec<&str> = name.split('.').collect();
            matches!(parts[..], [program, date, "jsonl"] if is_program(program) && is_date(date))
        })
        // The folder's own files, as pruned: a link is not followed.
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .flat_map(|text| text.lines().map(str::to_string).collect::<Vec<_>>())
        .filter_map(|line| Some((serde_json::from_str(&line).ok()?, line)))
        .collect();
    let trace = |line: &serde_json::Value| line["span"]["trace"].as_str().map(str::to_string);
    let ids: std::collections::HashSet<String> = lines
        .iter()
        .filter(|(line, _)| line["session"].as_str() == Some(session))
        .filter_map(|(line, _)| trace(line))
        .collect();
    let mut kept: Vec<(String, String)> = lines
        .into_iter()
        .filter(|(line, _)| trace(line).is_some_and(|id| ids.contains(&id)))
        .map(|(line, text)| (line["timestamp"].as_str().unwrap_or_default().to_string(), text))
        .collect();
    kept.sort_by(|a, b| a.0.cmp(&b.0));
    kept.into_iter().map(|(_, text)| text).collect::<Vec<_>>().join("\n")
}
