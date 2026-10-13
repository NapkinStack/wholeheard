//! The transcript of a session as `contracts/transcript/v1` describes it, written by a step
//! that may be stopped at any moment and run again: a file that only grows, true up to its
//! last line. Nothing here knows an engine: one hears a span of sound, this writes it, once.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

use serde_json::{Value, json};

/// The contract this module provides, as the transcript's first line names it.
pub const CONTRACT: &str = "transcript/v1";

/// Who spoke: whoever recorded, or everyone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Speaker {
    Me,
    Others,
}

impl Speaker {
    pub fn name(self) -> &'static str {
        match self {
            Speaker::Me => "me",
            Speaker::Others => "others",
        }
    }
}

/// A stretch of speech an engine heard: seconds from the session's start, and what was said.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// What made a transcript: its first line. Two makings with different answers here are two
/// transcripts, never one continued.
#[derive(Debug, Clone, PartialEq)]
pub struct Making {
    pub engine: String,
    pub model: String,
    /// The language asked for, or `None` when the engine detects it.
    pub language: Option<String>,
    /// The text the engine received as context, or `None`.
    pub context: Option<String>,
}

impl Making {
    fn header(&self) -> Value {
        json!({ "type": "header", "contract": CONTRACT, "engine": self.engine, "model": self.model,
                "language": self.language, "context": self.context })
    }
}

/// Seconds, to the hundredth: what the file holds, and what two runs must agree on.
fn hundredths(seconds: f64) -> i64 {
    (seconds * 100.0).round() as i64
}

/// A session's transcript, open for adding.
pub struct Transcript {
    file: fs::File,
    covered: [i64; 2],
    known: HashSet<(Speaker, i64)>,
    complete: bool,
}

impl Transcript {
    /// Opens the transcript of a session's folder, or starts it. One that another making
    /// started is refused: a better transcript is another file, never this one continued.
    pub fn open(folder: &Path, making: &Making) -> io::Result<Self> {
        let path = folder.join("transcript.jsonl");
        let refused = |why: &str| Err(io::Error::new(io::ErrorKind::InvalidData, why.to_string()));
        let written = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        // What follows the last line feed was being written when the step stopped: it never
        // was a line, and a line added after it would be glued to it.
        let whole = written.iter().rposition(|&byte| byte == b'\n').map_or(0, |end| end + 1);
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(&path)?;
        file.set_len(whole as u64)?;
        let mut transcript = Self {
            file,
            covered: [0, 0],
            known: HashSet::new(),
            complete: false,
        };
        let mut lines = std::str::from_utf8(&written[..whole]).unwrap_or("").lines();
        match lines.next().map(serde_json::from_str::<Value>) {
            None => {
                io::Seek::seek(&mut transcript.file, io::SeekFrom::End(0))?;
                transcript.add(&[making.header()])?;
                return Ok(transcript);
            }
            Some(Ok(header)) if header == making.header() => {}
            Some(Ok(header)) if header["type"] == "header" && header["contract"] == CONTRACT => {
                return refused(
                    "this transcript was made by another engine, model, language or context: make it again, it is not continued",
                );
            }
            Some(_) => return refused("the file is not a transcript this version writes"),
        }
        for line in lines.filter_map(|line| serde_json::from_str::<Value>(line).ok()) {
            let speaker = match line["speaker"].as_str() {
                Some("me") => Speaker::Me,
                Some("others") => Speaker::Others,
                _ => {
                    transcript.complete |= line["type"] == "complete";
                    continue;
                }
            };
            match (line["type"].as_str(), line["start"].as_f64(), line["until"].as_f64()) {
                (Some("segment"), Some(start), _) => drop(transcript.known.insert((speaker, hundredths(start)))),
                (Some("covered"), _, Some(until)) => {
                    transcript.covered[speaker as usize] = transcript.covered[speaker as usize].max(hundredths(until));
                }
                _ => {}
            }
        }
        io::Seek::seek(&mut transcript.file, io::SeekFrom::End(0))?;
        Ok(transcript)
    }

    /// Up to where a speaker's sound was heard, in seconds: where the step takes up again.
    pub fn covered(&self, speaker: Speaker) -> f64 {
        self.covered[speaker as usize] as f64 / 100.0
    }

    /// Whether the whole session was heard.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Adds what an engine heard of one speaker, up to `until`. A segment already there — the
    /// same speaker, the same start — is not added again: hearing a span twice changes nothing.
    pub fn heard(&mut self, speaker: Speaker, segments: &[Segment], until: f64) -> io::Result<()> {
        let mut lines = Vec::new();
        for segment in segments {
            let (start, end) = (hundredths(segment.start.max(0.0)), hundredths(segment.end));
            let text = segment.text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() || start < self.covered[speaker as usize] || !self.known.insert((speaker, start)) {
                continue;
            }
            lines.push(json!({ "type": "segment", "speaker": speaker.name(), "start": start as f64 / 100.0,
                               "end": end.max(start) as f64 / 100.0, "text": text }));
        }
        if hundredths(until) > self.covered[speaker as usize] {
            self.covered[speaker as usize] = hundredths(until);
            lines.push(json!({ "type": "covered", "speaker": speaker.name(), "until": self.covered(speaker) }));
        }
        self.add(&lines)
    }

    /// Says the whole session was heard, once: `duration` seconds of sound.
    pub fn complete(&mut self, duration: f64) -> io::Result<()> {
        if std::mem::replace(&mut self.complete, true) {
            return Ok(());
        }
        self.add(&[json!({ "type": "complete", "duration": hundredths(duration) as f64 / 100.0 })])
    }

    /// Whole lines, in one write: read at any moment, the file ends on a line or on one being written.
    fn add(&mut self, lines: &[Value]) -> io::Result<()> {
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        self.file.write_all(text.as_bytes())?;
        self.file.flush()
    }
}
