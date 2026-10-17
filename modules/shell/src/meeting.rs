//! One meeting, as the window opens it: its transcript (`contracts/transcript/v1`) and its
//! minutes (`contracts/minutes/v1`), read from the session's folder. Nothing here writes.
//!
//! What a file holds that its contract does not allow is never shown as something it is not:
//! a transcript line that is not one is skipped and counted; minutes without their front matter
//! or their three sections are said unreadable.

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

/// The minutes, as their contract lays them out.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Minutes {
    pub made: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The three sections' titles, in the meeting's language, in their order.
    pub titles: [String; 3],
    pub highlights: String,
    pub actions: Vec<Action>,
    pub key_points: Vec<String>,
}

/// One action: what, who, by when — as the minutes wrote them; a part the item did not give is
/// empty.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Action {
    pub what: String,
    pub who: String,
    pub when: String,
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
    let mut read = Transcript {
        engine: None,
        model: None,
        complete: false,
        segments: Vec::new(),
        unread: 0,
    };
    let mut header = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            read.unread += 1;
            continue;
        };
        let field = |name: &str| value.get(name).and_then(serde_json::Value::as_str).map(str::to_string);
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("header") if field("contract").as_deref() == Some("transcript/v1") => {
                header = true;
                read.engine = field("engine");
                read.model = field("model");
            }
            Some("segment") => {
                let number = |name: &str| value.get(name).and_then(serde_json::Value::as_f64);
                let (Some(start), Some(end), Some(text)) = (number("start"), number("end"), field("text")) else {
                    read.unread += 1;
                    continue;
                };
                // A segment written before tracks were is on its speaker's (the contract).
                let track =
                    field("track").or_else(|| field("speaker").map(|speaker| if speaker == "me" { "local" } else { "remote" }.to_string()));
                let Some(track) = track.filter(|track| track == "local" || track == "remote") else {
                    read.unread += 1;
                    continue;
                };
                read.segments.push(Segment {
                    start,
                    end,
                    track,
                    voice: field("voice"),
                    text,
                });
            }
            Some("covered") => {}
            Some("complete") => read.complete = true,
            _ => read.unread += 1,
        }
    }
    if !header {
        return Read::Unreadable;
    }
    read.segments
        .sort_by(|a, b| a.start.total_cmp(&b.start).then(a.track.cmp(&b.track)));
    Read::Present(read)
}

/// The minutes of a session's folder.
pub fn minutes(folder: &Path) -> Read<Minutes> {
    match fs::read_to_string(folder.join("minutes.md")) {
        Err(_) => Read::Absent,
        Ok(text) => parse_minutes(&text).map_or(Read::Unreadable, Read::Present),
    }
}

fn parse_minutes(text: &str) -> Option<Minutes> {
    let rest = text.strip_prefix("---\n")?;
    let (front, body) = rest.split_once("\n---\n")?;
    let mut fields = std::collections::HashMap::new();
    for line in front.lines() {
        let (key, value) = line.split_once(':')?;
        fields.insert(key.trim(), value.trim().to_string());
    }
    if fields.get("contract").map(String::as_str) != Some("minutes/v1") {
        return None;
    }
    // The sections, found by their place: each opens on a title of second level.
    let mut sections: Vec<(String, Vec<&str>)> = Vec::new();
    for line in body.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            sections.push((title.trim().to_string(), Vec::new()));
        } else if let Some((_, lines)) = sections.last_mut() {
            lines.push(line);
        }
    }
    let [highlights, actions, points] = <[(String, Vec<&str>); 3]>::try_from(sections).ok()?;
    let items = |lines: &[&str]| -> Vec<String> {
        lines
            .iter()
            .filter_map(|line| line.trim_start().strip_prefix("- ").map(|item| item.trim().to_string()))
            .collect()
    };
    let value = |key: &str| fields.get(key).filter(|value| *value != "null" && !value.is_empty()).cloned();
    Some(Minutes {
        made: value("made"),
        provider: value("provider"),
        model: value("model"),
        highlights: highlights
            .1
            .iter()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        actions: items(&actions.1)
            .into_iter()
            .map(|item| {
                let mut parts = item.splitn(3, " — ").map(str::trim);
                let mut next = || parts.next().unwrap_or_default().to_string();
                Action {
                    what: next(),
                    who: next(),
                    when: next(),
                }
            })
            .collect(),
        key_points: items(&points.1),
        titles: [highlights.0, actions.0, points.0],
    })
}
