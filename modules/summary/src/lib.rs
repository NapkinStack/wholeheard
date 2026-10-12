//! What `summary` reads and writes, with no provider in sight: the transcript of a session as
//! `contracts/transcript/v1` describes it, and its minutes as `contracts/minutes/v1` does.
//!
//! The minutes exist whole or not at all: an answer that is not minutes is refused before a
//! byte is written, and the file takes its name only once it is complete.

use std::{fs, io, path::Path};

/// The contract this module reads, as the transcript's first line names it.
pub const TRANSCRIPT: &str = "transcript/v1";
/// The contract this module provides, as the minutes' front matter names it.
pub const MINUTES: &str = "minutes/v1";

/// A stretch of speech, as the transcript holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub speaker: String,
    pub start: f64,
    pub text: String,
}

/// Why something a file or a provider gave was refused: one sentence, for the user.
#[derive(Debug, PartialEq, Eq)]
pub struct Refused(pub String);

fn refused<T>(why: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(why.into()))
}

/// The segments of a transcript, in the order of the meeting. A line still being written —
/// no line feed yet — is left out; a type or a field this version does not know is skipped.
pub fn read_transcript(text: &str) -> Result<Vec<Segment>, Refused> {
    let whole = text.rfind('\n').map_or("", |end| &text[..end]);
    let mut lines = whole.lines().filter(|line| !line.trim().is_empty());
    let header: serde_json::Value = lines.next().and_then(|line| serde_json::from_str(line).ok()).unwrap_or_default();
    if header["type"] != "header" || header["contract"] != TRANSCRIPT {
        return refused(format!(
            "this file is not a transcript this version reads: its first line does not name {TRANSCRIPT}"
        ));
    }
    let mut segments = Vec::new();
    for line in lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return refused("the transcript holds a line that is not JSON");
        };
        if value["type"] != "segment" {
            continue;
        }
        let (Some(speaker), Some(start), Some(end), Some(text)) = (
            value["speaker"].as_str(),
            value["start"].as_f64(),
            value["end"].as_f64(),
            value["text"].as_str(),
        ) else {
            return refused("the transcript holds a segment without its speaker, its times or its text");
        };
        if !matches!(speaker, "me" | "others") || end < start {
            return refused(format!(
                "the transcript holds a segment this version refuses: speaker {speaker}, {start} to {end}"
            ));
        }
        segments.push(Segment {
            speaker: speaker.into(),
            start,
            text: text.into(),
        });
    }
    if segments.is_empty() {
        return refused("the transcript holds no speech: there is nothing to write minutes of");
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(segments)
}

/// What a provider is asked: the rules, the context the user attached, the meeting.
pub fn request(segments: &[Segment], context: Option<&str>) -> String {
    let mut asked = String::from(
        "You write the minutes of a meeting from its transcript.\n\
         Write in the language the meeting is held in — the section titles too.\n\
         Write only what the transcript says: invent no name, no date, no decision.\n\
         When the meeting did not settle who does an action or by when, say so in the meeting's \
         language — \"not specified\", or that it was proposed and not accepted. Never guess.\n\
         A person the meeting talks about is not a participant: never give them an action.\n\
         In the transcript, \"me\" is the person who recorded and \"others\" is everyone else.\n\n\
         Answer with exactly three sections of second level and nothing else, in this order:\n\
         ## <highlights: a few sentences — what the meeting was for, what came out of it>\n\
         ## <actions: one bullet each — what — who — by when>\n\
         ## <key points: one bullet each — decisions, figures, dates, what stays open>\n\
         A section with nothing to say says that there is none.\n\n",
    );
    if let Some(context) = context.map(str::trim).filter(|context| !context.is_empty()) {
        asked.push_str(&format!("Context given by the user:\n{context}\n\n"));
    }
    asked.push_str("Transcript:\n");
    for segment in segments {
        let seconds = segment.start as u64;
        asked.push_str(&format!(
            "[{:02}:{:02}] {}: {}\n",
            seconds / 60,
            seconds % 60,
            segment.speaker,
            segment.text
        ));
    }
    asked
}

/// What made the minutes, as their front matter says it.
pub struct Made<'a> {
    pub at: &'a str,
    /// The host that answered, or `local` — never a key, never a path.
    pub provider: &'a str,
    pub model: &'a str,
    pub context: Option<&'a str>,
}

/// The minutes file for a provider's answer — or the reason the answer is not minutes.
pub fn minutes(answer: &str, made: &Made) -> Result<String, Refused> {
    let body = answer.trim();
    let sections: Vec<&str> = body.lines().filter(|line| line.starts_with("## ")).collect();
    if !body.starts_with("## ") || sections.len() != 3 {
        return refused(format!(
            "the provider's answer is not minutes: {} section(s) of second level where three are expected",
            sections.len()
        ));
    }
    if body
        .split("\n## ")
        .any(|section| section.lines().skip(1).all(|line| line.trim().is_empty()))
    {
        return refused("the provider's answer is not minutes: a section is empty");
    }
    // A JSON string is a YAML string: quoted, escaped, on one line.
    let quoted = |text: &str| serde_json::Value::from(text).to_string();
    let context = made
        .context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .map_or("null".into(), quoted);
    Ok(format!(
        "---\ncontract: {MINUTES}\nmade: {}\nprovider: {}\nmodel: {}\ncontext: {context}\n---\n\n{body}\n",
        made.at,
        quoted(made.provider),
        quoted(made.model),
    ))
}

/// Puts the minutes in the session's folder: under their name only once they are whole. The
/// minutes that were there are replaced; a failure before the last step leaves them as they were.
pub fn write_minutes(folder: &Path, minutes: &str) -> io::Result<()> {
    let draft = folder.join("minutes.md.part");
    fs::write(&draft, minutes)?;
    fs::rename(draft, folder.join("minutes.md"))
}
