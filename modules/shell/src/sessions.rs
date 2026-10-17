//! The sessions in the user's folder, as the window lists them: each one's day, length, what it
//! holds and what it weighs. Read from `session.json` (`contracts/session-audio`, both forms)
//! and the files beside it; nothing here writes, moves or opens a file for writing.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

/// Where a session is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Stopped: its files are whole.
    Done,
    /// Being recorded now, by this Summit.
    Recording,
    /// Started and never stopped, and nobody records it now: killed, or the machine went off.
    Interrupted,
}

/// One session, as listed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Listed {
    pub folder: PathBuf,
    /// When it started, as `session.json` says it: an instant with its offset.
    pub started: String,
    /// The same, in seconds since 1970; none when it cannot be read as an instant.
    pub instant: Option<i64>,
    /// How long it lasted, from its start to its stop; none while it has no stop.
    pub seconds: Option<f64>,
    pub state: State,
    pub transcript: bool,
    pub minutes: bool,
    /// What its sound weighs on disk, the tracks and what was made from them, in bytes.
    pub audio: u64,
    /// What everything else weighs: the transcript, the minutes, the session's own files.
    pub text: u64,
}

/// The sessions of a folder, newest first. `recording` is the folder being recorded now, if
/// any. A folder that is no session — no `session.json`, or one that is not of a form this
/// version reads — is left out: it is never listed as something it is not.
pub fn list(sessions: &Path, recording: Option<&Path>) -> Vec<Listed> {
    let Ok(entries) = fs::read_dir(sessions) else {
        return Vec::new();
    };
    let mut listed: Vec<Listed> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter_map(|folder| read(&folder, recording))
        .collect();
    listed.sort_by(|a, b| b.instant.cmp(&a.instant).then(b.folder.cmp(&a.folder)));
    listed
}

/// The folder of the session a page names, in the sessions' folder: by its name alone — never a
/// path, never a way out of the folder by its name — and only when it holds a session. A link the
/// user put in the folder is followed, as the list follows it.
pub fn folder(sessions: &Path, name: &str) -> Option<PathBuf> {
    let mut parts = Path::new(name).components();
    let (Some(std::path::Component::Normal(part)), None) = (parts.next(), parts.next()) else {
        return None;
    };
    let folder = sessions.join(part);
    folder.join("session.json").is_file().then_some(folder)
}

fn read(folder: &Path, recording: Option<&Path>) -> Option<Listed> {
    let text = fs::read_to_string(folder.join("session.json")).ok()?;
    let session: serde_json::Value = serde_json::from_str(&text).ok()?;
    let contract = session.get("contract")?.as_str()?;
    if !matches!(contract, "session-audio/v1" | "session-audio/v2") {
        return None;
    }
    let started = session.get("started")?.as_str()?.to_string();
    let stopped = session.get("stopped").and_then(serde_json::Value::as_str);
    let seconds = stopped.and_then(|stopped| Some((instant(stopped)? - instant(&started)?) as f64));
    let state = match (stopped, recording) {
        (Some(_), _) => State::Done,
        (None, Some(now)) if now == folder => State::Recording,
        (None, _) => State::Interrupted,
    };
    let (mut audio, mut text) = (0, 0);
    weigh(folder, &mut audio, &mut text);
    Some(Listed {
        folder: folder.to_path_buf(),
        instant: instant(&started),
        started,
        seconds,
        state,
        transcript: folder.join("transcript.jsonl").is_file(),
        minutes: folder.join("minutes.md").is_file(),
        audio,
        text,
    })
}

/// The sound of the sessions stopped more than `days` days before `now` (seconds since 1970): every
/// `.wav` in each such session's folder, what was made from the tracks too. A session being
/// recorded, left without its end, or that cannot be dated keeps its sound.
pub fn expired_audio(sessions: &Path, days: u32, now: i64) -> Vec<PathBuf> {
    let reach = i64::from(days) * 86_400;
    // A session with a length is one that stopped.
    list(sessions, None)
        .into_iter()
        .filter(|session| {
            matches!((session.instant, session.seconds), (Some(started), Some(seconds)) if started + seconds as i64 + reach <= now)
        })
        .flat_map(|session| {
            let mut sound = Vec::new();
            sounds(&session.folder, &mut sound);
            sound
        })
        .collect()
}

/// Every `.wav` under a folder.
fn sounds(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => sounds(&path, found),
            Ok(kind) if kind.is_file() && path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("wav")) => {
                found.push(path)
            }
            _ => {}
        }
    }
}

/// Adds up a folder's files: sound — a `.wav`, wherever it is — and the rest.
fn weigh(folder: &Path, audio: &mut u64, text: &mut u64) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for entry in entries.filter_map(Result::ok) {
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            weigh(&path, audio, text);
        } else if kind.is_file() {
            let size = entry.metadata().map_or(0, |data| data.len());
            if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("wav")) {
                *audio += size;
            } else {
                *text += size;
            }
        }
    }
}

/// An instant as the contract writes it — `2026-10-13T14:05:12+02:00`, with `Z`, or another
/// offset form of ISO 8601 — in seconds since 1970; none when it is not one.
pub fn instant(text: &str) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if text.get(4..5)? != "-" || text.get(10..11)? != "T" {
        return None;
    }
    // Fractions of a second are left: a session's length is said in minutes.
    let rest = text.get(19..)?;
    let rest = match rest.strip_prefix('.') {
        Some(fraction) => fraction.trim_start_matches(|c: char| c.is_ascii_digit()),
        None => rest,
    };
    // The offset, in each form ISO 8601 allows — `+02:00`, `+0200`, `+02` — and nothing after.
    let offset = if rest == "Z" {
        0
    } else {
        let sign = match rest.get(0..1)? {
            "+" => 1,
            "-" => -1,
            _ => return None,
        };
        let digits = |text: &str| text.len() == 2 && text.bytes().all(|b| b.is_ascii_digit());
        let (hours, minutes) = match rest.len() {
            3 => (rest.get(1..3)?, "00"),
            5 => (rest.get(1..3)?, rest.get(3..5)?),
            6 if rest.get(3..4)? == ":" => (rest.get(1..3)?, rest.get(4..6)?),
            _ => return None,
        };
        if !digits(hours) || !digits(minutes) {
            return None;
        }
        sign * (hours.parse::<i64>().ok()? * 3600 + minutes.parse::<i64>().ok()? * 60)
    };
    // Days from 1970-01-01 to the date (Howard Hinnant's days_from_civil).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}
