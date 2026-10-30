//! A meeting's context (D59): what it is about, who takes part — `context.txt` of
//! `contracts/session-audio/v2` (and v1), "a few lines of plain text, UTF-8", written by the user
//! in the window and read by the transcript and the minutes made after. Written whole, never
//! before the meeting's folder holds its `session.json`; never while a making could read it half.

use std::{fs, io, path::Path};

use serde::Serialize;

use crate::{meeting::Read, meeting::Transcript, minutes::Minutes, words::Failure};

/// The most the field takes, in characters as the page counts them: 400 of a Latin or Cyrillic
/// script are under the 224 tokens Whisper keeps and a service like Groq takes (#466, #489).
pub const MOST: usize = 400;

/// Why the field is closed on a meeting's page.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Closed {
    /// Its transcript or its minutes are being made: they read it as they start.
    Making,
    /// A transcript is left unfinished: continued with another context, it would be refused.
    Unfinished,
}

impl Closed {
    /// How a command refused for it says so.
    pub fn failure(self) -> Failure {
        match self {
            Closed::Making => Failure::Busy,
            Closed::Unfinished => Failure::NotComplete,
        }
    }
}

/// The context as the user wrote it; none when there is none, or it cannot be read as text.
pub fn read(folder: &Path) -> Option<String> {
    fs::read_to_string(folder.join("context.txt")).ok()
}

/// The context as the steps read it: trimmed, none when nothing is left.
pub fn heard(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}

/// Writes the meeting's context whole — a file beside it, then renamed over it —, or removes it
/// when nothing is left of it. Refused before the folder holds `session.json` (`NotFound`), and
/// over `MOST` characters (`InvalidInput`): what was there then stays.
pub fn write(folder: &Path, text: &str) -> io::Result<()> {
    if !folder.join("session.json").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the meeting's folder holds no session.json yet",
        ));
    }
    if text.chars().count() > MOST {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("a context holds {MOST} characters at most"),
        ));
    }
    let file = folder.join("context.txt");
    if heard(Some(text)).is_none() {
        return match fs::remove_file(&file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let beside = folder.join(".context.txt.part");
    let written = fs::write(&beside, text).and_then(|()| fs::rename(&beside, &file));
    if written.is_err() {
        let _ = fs::remove_file(&beside);
    }
    written
}

/// Whether the field is closed: a making of the meeting's runs (`making`), or its transcript is
/// left unfinished.
pub fn closed(making: bool, transcript: &Read<Transcript>) -> Option<Closed> {
    match transcript {
        _ if making => Some(Closed::Making),
        Read::Present(transcript) if !transcript.complete => Some(Closed::Unfinished),
        _ => None,
    }
}

/// Whether the transcript or the minutes there were made with another context than the one
/// written now: then the page offers to make them again with it.
pub fn changed(now: Option<&str>, transcript: &Read<Transcript>, minutes: &Read<Minutes>) -> bool {
    let now = heard(now);
    let other = |made: &Option<String>| heard(made.as_deref()) != now;
    matches!(transcript, Read::Present(transcript) if other(&transcript.context))
        || matches!(minutes, Read::Present(minutes) if other(&minutes.context))
}
