//! The transcript of a session as `contracts/transcript/v1` describes it, written by a step
//! that may be stopped at any moment and run again: a file that only grows, true up to its
//! last line. Nothing here knows an engine: one hears a span of sound, this writes it, once.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
};

use serde_json::{Value, json};

/// The contract this module provides, as the transcript's first line names it.
pub const CONTRACT: &str = "transcript/v1";
/// Two starts of one speaker closer than this are one segment heard twice, not two.
const SAME: i64 = 5;

/// Who spoke: whoever recorded, or everyone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

fn refused<T>(kind: io::ErrorKind, why: &str) -> io::Result<T> {
    Err(io::Error::new(kind, why.to_string()))
}

/// Seconds as the file holds them, in hundredths — or a refusal: a time is a number, not negative.
fn hundredths(seconds: f64) -> io::Result<i64> {
    let rounded = (seconds * 100.0).round();
    if seconds.is_finite() && (0.0..1e11).contains(&rounded) {
        return Ok(rounded as i64);
    }
    refused(
        io::ErrorKind::InvalidInput,
        "a time of the transcript is not a number of seconds from the session's start",
    )
}

/// A time an engine gave within a second before the session's start is its start; any other
/// is left as it is, to be refused if it is no time.
fn not_before(seconds: f64) -> f64 {
    if (-1.0..0.0).contains(&seconds) { 0.0 } else { seconds }
}

/// Whether a text says something: letters, digits, signs — not only what nobody sees.
fn says(text: &str) -> bool {
    text.chars()
        .any(|c| !c.is_whitespace() && !c.is_control() && !matches!(c, '\u{200b}'..='\u{200d}' | '\u{2060}' | '\u{feff}'))
}

/// A session's transcript, open for adding. A speaker is heard forward: what is added
/// starts after what is there.
pub struct Transcript {
    file: fs::File,
    covered: [i64; 2],
    last: [Option<i64>; 2],
    complete: bool,
}

impl Transcript {
    /// Starts the transcript of a session's folder; one that is already there is not touched.
    pub fn start(folder: &Path, making: &Making) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(folder.join("transcript.jsonl"))?;
        let mut transcript = Self {
            file,
            covered: [0, 0],
            last: [None, None],
            complete: false,
        };
        transcript.add(&[making.header()])?;
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

    /// Adds what an engine heard of one speaker, up to `until`, in the order it was said. A
    /// segment that starts before the last one written — or with it, to within a few
    /// hundredths — is one already heard, and is not added again.
    pub fn heard(&mut self, speaker: Speaker, segments: &[Segment], until: f64) -> io::Result<()> {
        if self.complete {
            return refused(
                io::ErrorKind::InvalidInput,
                "the transcript is complete: nothing is added after its last line",
            );
        }
        let who = speaker as usize;
        let (until, mut last, mut lines) = (hundredths(until)?, self.last[who], Vec::new());
        // In the order they were said, whatever order they came in; and all of them within the span.
        let mut heard = Vec::new();
        for segment in segments {
            let (start, end) = (hundredths(not_before(segment.start))?, hundredths(not_before(segment.end))?);
            if start > until {
                return refused(
                    io::ErrorKind::InvalidInput,
                    "a segment starts after the end of the span it was heard in",
                );
            }
            heard.push((start, end.max(start), segment.text.split_whitespace().collect::<Vec<_>>().join(" ")));
        }
        heard.sort_by_key(|(start, ..)| *start);
        for (start, end, text) in heard {
            if !says(&text) || start < self.covered[who] || last.is_some_and(|last| start < last + SAME) {
                continue;
            }
            last = Some(start);
            lines.push(json!({ "type": "segment", "speaker": speaker.name(), "start": start as f64 / 100.0,
                               "end": end as f64 / 100.0, "text": text }));
        }
        if until > self.covered[who] {
            lines.push(json!({ "type": "covered", "speaker": speaker.name(), "until": until as f64 / 100.0 }));
        }
        // What is known changes once the lines are in the file, not before.
        self.add(&lines)?;
        self.last[who] = last;
        self.covered[who] = self.covered[who].max(until);
        Ok(())
    }

    /// Says the whole session was heard, once: `duration` seconds of sound.
    pub fn complete(&mut self, duration: f64) -> io::Result<()> {
        let duration = hundredths(duration)?;
        if !self.complete {
            self.add(&[json!({ "type": "complete", "duration": duration as f64 / 100.0 })])?;
            self.complete = true;
        }
        Ok(())
    }

    /// Whole lines, in one write: read at any moment, the file ends on a line or on one being written.
    fn add(&mut self, lines: &[Value]) -> io::Result<()> {
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        self.file.write_all(text.as_bytes())?;
        self.file.flush()
    }
}
