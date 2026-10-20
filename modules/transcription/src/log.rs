//! What the program does, written for whoever reads after it (ADR-0007): one JSON line per
//! step in the user's state folder, a file a day kept fourteen days, under the id of the making
//! the shell passed. Never what was said: no text of the meeting, no context, no key, no path
//! to a meeting beyond its folder's name.
//!
//! A log that cannot be written is a log missing, never a transcript lost: the program goes on.

use std::path::PathBuf;

use tracing::span::EnteredSpan;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Where the logs go: `WHOLEHEARD_LOG_DIR` when given, else the system's place for them.
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
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
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

/// Starts the log of `program`; what is logged until the value returned is dropped belongs
/// to this run, its version and its making's id.
pub fn start(program: &'static str) -> EnteredSpan {
    let appender = folder().and_then(|folder| {
        std::fs::create_dir_all(&folder).ok()?;
        RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(program)
            .filename_suffix("jsonl")
            .max_log_files(14)
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
    tracing::info_span!("run", program, version = env!("CARGO_PKG_VERSION"), trace = %trace()).entered()
}
