//! One meeting, as the window opens it: its transcript (`contracts/transcript/v1`), read from the
//! session's folder. Nothing here writes.
//!
//! What a file holds that its contract does not allow is never shown as something it is not:
//! a transcript line that is not one is skipped and counted.

use std::{fs, path::Path};

use serde::Serialize;

/// What was said, in the meeting's order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transcript {
    /// What transcribed it: the engine and its model, as the header says.
    pub engine: Option<String>,
    pub model: Option<String>,
    /// The `complete` line is there: the step finished.
    pub complete: bool,
    pub segments: Vec<Segment>,
    /// Lines that were not lines of the contract, left out.
    pub unread: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Segment {
    /// Seconds from the session's start.
    pub start: f64,
    pub end: f64,
    /// `local` or `remote`: the track it was heard on.
    pub track: String,
    /// The voice within the track, when one was told.
    pub voice: Option<String>,
    pub text: String,
}

/// A file of the session, read: not there, there and read, or there and not of this version.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Read<T> {
    Absent,
    Present(T),
    Unreadable,
}

/// The transcript of a session's folder.
pub fn transcript(folder: &Path) -> Read<Transcript> {
    let Ok(text) = fs::read_to_string(folder.join("transcript.jsonl")) else {
        return Read::Absent;
    };
    // Each line ends with a line feed: a last one without is still being written, and waits.
    let whole = text.rfind('\n').map_or("", |end| &text[..end]);
    let mut lines = whole.lines().filter(|line| !line.trim().is_empty());
    let value = |line: &str| serde_json::from_str::<serde_json::Value>(line).ok();
    let text_of = |value: &serde_json::Value, name: &str| value.get(name).and_then(serde_json::Value::as_str).map(str::to_string);
    // The header is the first line, once.
    let Some(header) = lines.next().and_then(value) else {
        return Read::Unreadable;
    };
    if text_of(&header, "type").as_deref() != Some("header") || text_of(&header, "contract").as_deref() != Some("transcript/v1") {
        return Read::Unreadable;
    }
    let mut read = Transcript {
        engine: text_of(&header, "engine"),
        model: text_of(&header, "model"),
        complete: false,
        segments: Vec::new(),
        unread: 0,
    };
    for line in lines {
        let Some(value) = value(line) else {
            read.unread += 1;
            continue;
        };
        match text_of(&value, "type").as_deref() {
            Some("segment") => match segment(&value) {
                Some(segment) => read.segments.push(segment),
                None => read.unread += 1,
            },
            Some("covered") => {}
            Some("complete") => read.complete = true,
            _ => read.unread += 1,
        }
    }
    read.segments
        .sort_by(|a, b| a.start.total_cmp(&b.start).then(a.track.cmp(&b.track)));
    Read::Present(read)
}

/// A segment line, as the contract allows it: a speaker, its track — `local` for `me`, `remote`
/// for `others`, the speaker's when the line was written before tracks were —, a start no later
/// than its end, a text.
fn segment(value: &serde_json::Value) -> Option<Segment> {
    let text = |name: &str| value.get(name).and_then(serde_json::Value::as_str).map(str::to_string);
    let number = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
    let heard_on = match text("speaker")?.as_str() {
        "me" => "local",
        "others" => "remote",
        _ => return None,
    };
    let track = text("track").unwrap_or_else(|| heard_on.to_string());
    let (start, end, said) = (number("start")?, number("end")?, text("text")?);
    (track == heard_on && start <= end).then(|| Segment {
        start,
        end,
        track,
        voice: text("voice"),
        text: said,
    })
}
