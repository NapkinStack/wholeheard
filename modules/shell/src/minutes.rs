//! One meeting's minutes (`contracts/minutes/v1`), as the window opens them: who wrote them, where
//! and when, from the front matter; and the body as written — rendered from its CommonMark by the
//! reader their producer judged them with, not read into parts the window would guess at. Nothing
//! here writes. Minutes without their front matter or their three sections are said unreadable.

use std::{fs, path::Path};

use serde::Serialize;

use crate::meeting::Read;

/// The minutes: who made them, and their body as a page shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Minutes {
    pub made: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The three sections as HTML: their titles, paragraphs and lists as written. What the
    /// body holds as HTML is shown as text, and a link as its words: a page shows the minutes, it
    /// is never driven by them.
    pub body: String,
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

fn parse_minutes(text: &str) -> Option<Minutes> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let rest = text.strip_prefix("---\n")?;
    let (front, body) = rest.split_once("\n---\n")?;
    let mut fields = std::collections::HashMap::new();
    for line in front.lines() {
        let (key, written) = line.split_once(':')?;
        fields.insert(key.trim(), written);
    }
    let field = |key: &str| fields.get(key).and_then(|written| value(written));
    if field("contract").as_deref() != Some("minutes/v1") {
        return None;
    }
    // Three sections, each on a title of second level however it is written: what the producer
    // judged before writing, read by the same reader.
    let titles = Parser::new(body)
        .filter(|event| {
            matches!(
                event,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                })
            )
        })
        .count();
    if titles != 3 {
        return None;
    }
    let shown = Parser::new(body).filter_map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Link { .. } | Tag::Image { .. }) | Event::End(TagEnd::Link | TagEnd::Image) => None,
        other => Some(other),
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, shown);
    Some(Minutes {
        made: field("made"),
        provider: field("provider"),
        model: field("model"),
        body: html,
    })
}
