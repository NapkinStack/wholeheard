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
    /// The context they were written with, as the front matter says (D59).
    pub context: Option<String>,
    /// The three sections as HTML: their titles, paragraphs and lists as written. What the
    /// body holds as HTML is shown as text, and a link as its words: a page shows the minutes, it
    /// is never driven by them.
    pub body: String,
    /// The body as written, without the front matter, its three titles the page's: what « Copier
    /// le compte rendu » puts on the clipboard, to paste where the user writes (D45).
    pub markdown: String,
}

/// The minutes of a session's folder, their three sections named as the page names them.
pub fn minutes(folder: &Path, sections: [&str; 3]) -> Read<Minutes> {
    match fs::read_to_string(folder.join("minutes.md")) {
        Err(_) => Read::Absent,
        Ok(text) => parse_minutes(&text, sections).map_or(Read::Unreadable, Read::Present),
    }
}

/// A front matter value as `wholeheard-summary` writes it: `null`, a plain value, or a string in
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

fn parse_minutes(text: &str, sections: [&str; 3]) -> Option<Minutes> {
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
    // Each section titled as the page names it: the order is the contract's, the words the
    // model's own — a small one numbers them (#261).
    let mut in_title = false;
    let shown = Parser::new(body).filter_map(move |event| match event {
        Event::Start(Tag::Heading {
            level: HeadingLevel::H2, ..
        }) => {
            in_title = true;
            Some(event)
        }
        Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
            in_title = false;
            Some(event)
        }
        _ if in_title => None,
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Link { .. } | Tag::Image { .. }) | Event::End(TagEnd::Link | TagEnd::Image) => None,
        other => Some(other),
    });
    let shown = shown.flat_map({
        let mut named = 0;
        move |event| {
            let title = matches!(
                event,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                })
            );
            let name = title.then(|| {
                named += 1;
                Event::Text(sections[named - 1].into())
            });
            std::iter::once(event).chain(name)
        }
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, shown);
    // Each title's line said as the page names it; every other byte as the model wrote it.
    let mut markdown = String::new();
    let mut kept = 0;
    let titled = Parser::new(body).into_offset_iter().filter_map(|(event, range)| {
        matches!(
            event,
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            })
        )
        .then_some(range)
    });
    for (name, range) in sections.iter().zip(titled) {
        markdown.push_str(&body[kept..range.start]);
        markdown.push_str(&format!("## {name}\n"));
        kept = range.end;
    }
    markdown.push_str(&body[kept..]);
    let markdown = format!("{}\n", markdown.trim());
    Some(Minutes {
        made: field("made"),
        provider: field("provider"),
        model: field("model"),
        context: field("context"),
        body: html,
        markdown,
    })
}
