//! A line of a transcript corrected by whoever listens to it (D62, ADR-0014): once the transcript
//! is complete, one segment's `text` rewritten and its `edited` set to when — the file replaced
//! whole, written beside then renamed, every other line as it was, in its order.

use std::{fs, io, path::Path};

use serde_json::Value;

/// Why a correction is not made.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// No `complete` line: the transcript is still made, or was stopped.
    NotComplete,
    /// No segment of that track starts there.
    NoSuchLine,
    /// Nothing left once tidied.
    Empty,
    /// The file could not be read or written.
    File(String),
}

/// The text as it is kept: one line — a break pasted becomes a space —, its ends' spaces dropped;
/// none when nothing is left.
pub fn tidy(text: &str) -> Option<String> {
    let one = text.replace("\r\n", " ").replace(['\n', '\r'], " ");
    let kept = one.trim();
    (!kept.is_empty()).then(|| kept.to_string())
}

/// The speaker a track's segments are written under (`transcript/v1`).
fn speaker(track: &str) -> &'static str {
    if track == "local" { "me" } else { "others" }
}

/// Corrects the segment heard on `track` from `start` in the transcript `file`: its `text` set to
/// `text` tidied, its `edited` to `when`. A segment is known by its speaker and its start, to the
/// hundredth (`transcript/v1`).
pub fn correct(file: &Path, track: &str, start: f64, text: &str, when: &str) -> Result<(), Refused> {
    let text = tidy(text).ok_or(Refused::Empty)?;
    let read = fs::read_to_string(file).map_err(|error| Refused::File(error.to_string()))?;
    let lines: Vec<&str> = read.split_inclusive('\n').collect();
    let parsed = |line: &str| serde_json::from_str::<Value>(line.trim_end()).ok();
    let kind = |value: &Value| value.get("type").and_then(Value::as_str).map(str::to_owned);
    if !lines
        .iter()
        .filter_map(|line| parsed(line))
        .any(|value| kind(&value).as_deref() == Some("complete"))
    {
        return Err(Refused::NotComplete);
    }
    let wanted = speaker(track);
    let found = lines.iter().position(|line| {
        parsed(line).is_some_and(|value| {
            kind(&value).as_deref() == Some("segment")
                && value.get("speaker").and_then(Value::as_str) == Some(wanted)
                && value
                    .get("start")
                    .and_then(Value::as_f64)
                    .is_some_and(|at| (at - start).abs() < 0.005)
        })
    });
    let index = found.ok_or(Refused::NoSuchLine)?;
    let mut segment = parsed(lines[index]).ok_or(Refused::NoSuchLine)?;
    segment["text"] = Value::String(text);
    segment["edited"] = Value::String(when.to_string());
    let rewritten = format!("{segment}\n");
    let whole: String = lines
        .iter()
        .enumerate()
        .map(|(at, line)| if at == index { rewritten.as_str() } else { line })
        .collect();
    replace(file, &whole).map_err(|error| Refused::File(error.to_string()))
}

/// Writes `content` beside `file`, then renames it over: read at any moment, the file is whole.
fn replace(file: &Path, content: &str) -> io::Result<()> {
    let beside = file.with_extension("jsonl.part");
    let written = fs::write(&beside, content).and_then(|()| fs::rename(&beside, file));
    if written.is_err() {
        let _ = fs::remove_file(&beside);
    }
    written
}
