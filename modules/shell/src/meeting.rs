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

/// The minutes of a session's folder.
pub fn minutes(folder: &Path) -> Read<Minutes> {
    match fs::read_to_string(folder.join("minutes.md")) {
        Err(_) => Read::Absent,
        Ok(text) => parse_minutes(&text).map_or(Read::Unreadable, Read::Present),
    }
}

/// A front matter value as `summit-summary` writes it: `null`, a plain value, or a string in
/// double quotes whose escapes — `\"`, `\\`, `\n`, `\t`, `\uXXXX` — are JSON's too.
fn value(written: &str) -> Option<String> {
    let written = written.trim();
    let read = if written.starts_with('"') {
        serde_json::from_str::<String>(written).ok()?
    } else {
        written.to_string()
    };
    (read != "null" && !read.is_empty()).then_some(read)
}

/// One section: its title, and what it holds — its paragraphs, and the items of its lists of first
/// level, each item's own text without the lists inside it.
#[derive(Default)]
struct Section {
    title: String,
    paragraphs: Vec<String>,
    items: Vec<String>,
}

fn parse_minutes(text: &str) -> Option<Minutes> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let rest = text.strip_prefix("---\n")?;
    let (front, body) = rest.split_once("\n---\n")?;
    let mut fields = std::collections::HashMap::new();
    for line in front.lines() {
        let (key, written) = line.split_once(':')?;
        fields.insert(key.trim(), written);
    }
    if fields.get("contract").and_then(|written| value(written)).as_deref() != Some("minutes/v1") {
        return None;
    }
    // The body read as `summit-summary` judged it: by a CommonMark reader, a section opening on
    // each title of second level, however it is written.
    let mut sections: Vec<Section> = Vec::new();
    let (mut in_title, mut lists, mut item, mut paragraph) = (false, 0usize, None::<String>, None::<String>);
    for event in Parser::new(body) {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2, ..
            }) => {
                sections.push(Section::default());
                in_title = true;
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => in_title = false,
            Event::Start(Tag::List(_)) => lists += 1,
            Event::End(TagEnd::List(_)) => lists -= 1,
            Event::Start(Tag::Item) if lists == 1 => item = Some(String::new()),
            Event::End(TagEnd::Item) if lists == 1 => {
                let done = item.take().unwrap_or_default();
                if let Some(section) = sections.last_mut() {
                    section.items.push(done.trim().to_string());
                }
            }
            Event::Start(Tag::Paragraph) if lists == 0 => paragraph = Some(String::new()),
            Event::End(TagEnd::Paragraph) if lists == 0 => {
                let done = paragraph.take().unwrap_or_default();
                if let Some(section) = sections.last_mut() {
                    section.paragraphs.push(done.trim().to_string());
                }
            }
            Event::Text(said) | Event::Code(said) => {
                let into = if in_title {
                    sections.last_mut().map(|section| &mut section.title)
                } else if lists == 1 {
                    item.as_mut()
                } else if lists == 0 {
                    paragraph.as_mut()
                } else {
                    None
                };
                if let Some(into) = into {
                    into.push_str(&said);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                for open in [item.as_mut().filter(|_| lists == 1), paragraph.as_mut().filter(|_| lists == 0)]
                    .into_iter()
                    .flatten()
                {
                    open.push(' ');
                }
            }
            _ => {}
        }
    }
    let [highlights, actions, points] = <[Section; 3]>::try_from(sections).ok()?;
    let key_points = if points.items.is_empty() { points.paragraphs } else { points.items };
    Some(Minutes {
        made: fields.get("made").and_then(|written| value(written)),
        provider: fields.get("provider").and_then(|written| value(written)),
        model: fields.get("model").and_then(|written| value(written)),
        highlights: highlights.paragraphs.join(" "),
        actions: actions
            .items
            .iter()
            .map(|item| {
                // What, who, by when: the last two parts are who and when, whatever the first holds.
                let mut parts: Vec<&str> = item.rsplitn(3, " — ").map(str::trim).collect();
                parts.reverse();
                match parts[..] {
                    [what, who, when] => Action {
                        what: what.into(),
                        who: who.into(),
                        when: when.into(),
                    },
                    [what, who] => Action {
                        what: what.into(),
                        who: who.into(),
                        when: String::new(),
                    },
                    _ => Action {
                        what: item.clone(),
                        who: String::new(),
                        when: String::new(),
                    },
                }
            })
            .collect(),
        key_points,
        titles: [
            highlights.title.trim().to_string(),
            actions.title.trim().to_string(),
            points.title.trim().to_string(),
        ],
    })
}
