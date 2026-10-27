//! One meeting, as the window opens it: its transcript (`contracts/transcript/v1`), read from the
//! session's folder. Nothing here writes.
//!
//! What a file holds that its contract does not allow is never shown as something it is not:
//! a transcript line that is not one is skipped and counted.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;

/// What was said, in the meeting's order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transcript {
    /// What transcribed it: the engine and its model, as the header says.
    pub engine: Option<String>,
    pub model: Option<String>,
    /// The context the engine received, as the header says (D59).
    pub context: Option<String>,
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
        context: text_of(&header, "context"),
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

/// How much each side spoke in each of `parts` equal parts of a meeting `seconds` long: for the
/// track `local`, then `remote`, the share of the part covered by speech, from 0 to 1. What a list
/// draws small, the meeting's page large.
pub fn shape(transcript: &Transcript, seconds: f64, parts: usize) -> [Vec<f32>; 2] {
    let mut shares = [vec![0.0_f32; parts], vec![0.0_f32; parts]];
    if seconds <= 0.0 || parts == 0 {
        return shares;
    }
    let width = seconds / parts as f64;
    for segment in &transcript.segments {
        let side = usize::from(segment.track != "local");
        let first = (segment.start / width).floor().max(0.0) as usize;
        for (part, share) in shares[side].iter_mut().enumerate().skip(first) {
            let (from, to) = (part as f64 * width, (part + 1) as f64 * width);
            if from >= segment.end {
                break;
            }
            let covered = segment.end.min(to) - segment.start.max(from);
            if covered > 0.0 {
                *share = (*share + (covered / width) as f32).min(1.0);
            }
        }
    }
    shares
}

/// A track of a meeting, to be played: its name — `local` or `remote` —, its file, and where
/// its first frame is on the session's clock, in seconds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Track {
    pub name: String,
    pub path: PathBuf,
    pub starts: f64,
}

/// The tracks a meeting's folder holds, as `session-audio` describes them, each where its first
/// frame is: version 1's from the session's start; version 2's where `clock.jsonl`'s first point
/// of the track puts its frame 0 — a track that opened on a silence starts later. A file that is
/// not there, or is a link, is no track: freed audio leaves nothing to play.
pub fn tracks(folder: &Path) -> Vec<Track> {
    let Some(session) = fs::read_to_string(folder.join("session.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return Vec::new();
    };
    let named: Vec<(&str, &str)> = match session["contract"].as_str() {
        Some("session-audio/v1") => vec![("local", "me.wav"), ("remote", "others.wav")],
        Some("session-audio/v2") => [("local", "local.wav"), ("remote", "remote.wav")]
            .into_iter()
            .filter(|(name, _)| session["tracks"].get(name).is_some())
            .collect(),
        _ => return Vec::new(),
    };
    let clock = fs::read_to_string(folder.join("clock.jsonl")).unwrap_or_default();
    let points: Vec<serde_json::Value> = clock.lines().filter_map(|line| serde_json::from_str(line).ok()).collect();
    named
        .into_iter()
        .filter_map(|(name, file)| {
            let path = std::path::absolute(folder.join(file)).ok()?;
            fs::symlink_metadata(&path).ok().filter(fs::Metadata::is_file)?;
            let rate = rate(&path)?;
            let starts = points
                .iter()
                .find(|point| point["track"] == name && point["sample"].is_u64() && point["at"].is_number())
                .map_or(0.0, |point| {
                    point["at"].as_f64().unwrap_or(0.0) - point["sample"].as_u64().unwrap_or(0) as f64 / rate
                });
            Some(Track {
                name: name.into(),
                path,
                starts,
            })
        })
        .collect()
}

/// A WAV file's sample rate, from its own `fmt ` chunk — never assumed.
fn rate(path: &Path) -> Option<f64> {
    use std::io::Read;
    let mut head = [0u8; 4096];
    let read = fs::File::open(path).ok()?.read(&mut head).ok()?;
    let head = &head[..read];
    if head.get(0..4)? != b"RIFF" || head.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut at = 12;
    while at + 8 <= head.len() {
        let size = u32::from_le_bytes(head[at + 4..at + 8].try_into().ok()?) as usize;
        if &head[at..at + 4] == b"fmt " {
            let rate = u32::from_le_bytes(head.get(at + 12..at + 16)?.try_into().ok()?);
            return (rate > 0).then_some(f64::from(rate));
        }
        at += 8 + size + size % 2;
    }
    None
}
