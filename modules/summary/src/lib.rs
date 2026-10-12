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
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(end) = text.rfind('\n') else {
        return refused("the transcript holds no whole line yet: it is only being started, or it is empty");
    };
    let whole = &text[..end];
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
        if text.contains(['\n', '\r']) {
            return refused(format!(
                "the transcript holds a segment whose text is on more than one line, at {start}"
            ));
        }
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

/// Minutes this module accepted: the only thing [`write_minutes`] writes.
#[derive(Debug, PartialEq, Eq)]
pub struct Minutes(String);

impl Minutes {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The level of a heading written with number signs, as Markdown reads the line; 0 otherwise.
fn heading(line: &str) -> usize {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let marks = line.trim_start_matches(' ');
    let level = marks.len() - marks.trim_start_matches('#').len();
    let after = &marks[level..];
    if indent <= 3 && (1..=6).contains(&level) && (after.is_empty() || after.starts_with([' ', '\t'])) {
        level
    } else {
        0
    }
}

/// Why an answer is not three sections of second level and nothing else — `None` when it is.
fn not_minutes(body: &str) -> Option<String> {
    let lines: Vec<&str> = body.lines().collect();
    if lines
        .iter()
        .any(|line| line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~"))
    {
        return Some("it holds a block of code".into());
    }
    if heading(lines.first().copied().unwrap_or("")) != 2 {
        return Some("it does not open on its first section".into());
    }
    // A line of dashes or equals signs under a line of text is a title too.
    let underlined = lines.windows(2).any(|pair| {
        let mark = pair[1].trim();
        !pair[0].trim().is_empty()
            && !mark.is_empty()
            && (mark.chars().all(|c| c == '=') || (mark.len() >= 2 && mark.chars().all(|c| c == '-')))
    });
    if underlined || lines.iter().any(|line| heading(line) == 1) {
        return Some("it holds a title above its sections".into());
    }
    let titles: Vec<&str> = lines.iter().copied().filter(|line| heading(line) == 2).collect();
    if titles.len() != 3 {
        return Some(format!("{} section(s) of second level where three are expected", titles.len()));
    }
    if titles.iter().any(|title| title.trim().trim_matches('#').trim().is_empty()) {
        return Some("a section has no title".into());
    }
    let mut said = [false; 3];
    let mut section = 0;
    for line in &lines[1..] {
        match heading(line) {
            2 => section += 1,
            0 if !line.trim().is_empty() => said[section] = true,
            _ => {}
        }
    }
    said.contains(&false).then(|| "a section is empty".into())
}

/// A text as one YAML string: quoted, on one line, with every character a YAML reader
/// refuses or reads otherwise written as its escape.
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{feff}' | '\u{fffe}' | '\u{ffff}') => {
                out.push_str(&format!("\\u{:04X}", c as u32))
            }
            c => out.push(c),
        }
    }
    out + "\""
}

/// `2026-10-13T15:12:40+02:00`, or with `Z`: nothing else is written as the time.
fn is_a_time(at: &str) -> bool {
    let shape = |text: &str, shape: &str| {
        text.len() == shape.len()
            && text
                .bytes()
                .zip(shape.bytes())
                .all(|(c, s)| if s == b'0' { c.is_ascii_digit() } else { c == s })
    };
    match at.len() {
        20 => shape(at, "0000-00-00T00:00:00Z"),
        25 => shape(&at[..19], "0000-00-00T00:00:00") && matches!(&at[19..20], "+" | "-") && shape(&at[20..], "00:00"),
        _ => false,
    }
}

/// The minutes file for a provider's answer — or the reason the answer is not minutes.
pub fn minutes(answer: &str, made: &Made) -> Result<Minutes, Refused> {
    let body = answer.trim();
    if let Some(why) = not_minutes(body) {
        return refused(format!("the provider's answer is not minutes: {why}"));
    }
    if !is_a_time(made.at) {
        return refused("the minutes cannot be dated: the time given is not a time");
    }
    let context = made
        .context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .map_or("null".into(), quoted);
    Ok(Minutes(format!(
        "---\ncontract: {MINUTES}\nmade: {}\nprovider: {}\nmodel: {}\ncontext: {context}\n---\n\n{body}\n",
        made.at,
        quoted(made.provider),
        quoted(made.model),
    )))
}

/// Puts the minutes in the session's folder: under their name only once they are whole. The
/// minutes that were there are replaced; a failure before the last step leaves them as they were.
pub fn write_minutes(folder: &Path, minutes: &Minutes) -> io::Result<()> {
    let draft = folder.join("minutes.md.part");
    fs::write(&draft, minutes.as_str())?;
    fs::rename(draft, folder.join("minutes.md"))
}
