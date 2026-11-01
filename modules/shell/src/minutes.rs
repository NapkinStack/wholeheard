//! One meeting's minutes (`contracts/minutes/v1`, `v2` and `v3`), as the window opens them: who wrote them, where
//! and when, from the front matter; and the body as written — rendered from its CommonMark by the
//! reader their producer judged them with, not read into parts the window would guess at. Nothing
//! here writes. Minutes without their front matter or their three sections are said unreadable.

use std::{fs, ops::Range, path::Path};

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
    /// The meeting's name, as the front matter says it (`minutes/v2`, D48); one out of the
    /// contract's bounds is no name, and the minutes still read.
    pub title: Option<String>,
    /// What the call cost and in which currency, as the provider reported them (`minutes/v3`, D51):
    /// both or neither; out of the contract's bounds, neither, and the minutes still read.
    pub cost: Option<f64>,
    pub currency: Option<String>,
    /// The three sections as HTML: their titles, paragraphs and lists as written. What the
    /// body holds as HTML is shown as text, and a link as its words: a page shows the minutes, it
    /// is never driven by them. A time ending an item of `minutes/v3` is the one mark made here,
    /// of digits alone: `<time data-second="751">12:31</time>` (D75).
    pub body: String,
    /// The body as written, without the front matter, its three titles the page's and its items'
    /// times left out: what « Copier le compte rendu » puts on the clipboard, to paste where the
    /// user writes (D45, D75).
    pub markdown: String,
}

/// The minutes of a session's folder, their three sections named as the page names them.
pub fn minutes(folder: &Path, sections: [&str; 3]) -> Read<Minutes> {
    match fs::read_to_string(folder.join("minutes.md")) {
        Err(_) => Read::Absent,
        Ok(text) => parse_minutes(&text, sections).map_or(Read::Unreadable, Read::Present),
    }
}

/// The most a title holds, in characters (`minutes/v2`).
const TITLE_MOST: usize = 80;

/// A meeting's name, as the list and its page show it: the title of minutes that read.
pub fn title(folder: &Path) -> Option<String> {
    let text = fs::read_to_string(folder.join("minutes.md")).ok()?;
    let (fields, body) = front_matter(&text)?;
    (sections_in(body) == 3).then(|| fields.get("title").and_then(|written| value(written)).and_then(within))?
}

/// A title within the contract: on one line, not blank, at most 80 characters.
fn within(title: String) -> Option<String> {
    let one_line = !title.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'));
    (one_line && title.chars().any(char::is_alphanumeric) && title.chars().count() <= TITLE_MOST).then_some(title)
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

/// What the call cost, as `minutes/v3` bounds it: a JSON number at least zero and three capital
/// letters, both or neither. A cost in double quotes is a string, whatever it holds.
fn priced(cost: Option<&str>, currency: Option<&str>) -> Option<(f64, String)> {
    let cost = serde_json::from_str::<f64>(cost?.trim()).ok().filter(|cost| *cost >= 0.0)?;
    let currency = value(currency?).filter(|code| code.len() == 3 && code.bytes().all(|c| c.is_ascii_uppercase()))?;
    Some((cost.abs(), currency))
}

/// The second a time says, written as `minutes/v3` has it: `mm:ss`, minutes in two digits or more,
/// seconds from 00 to 59. One too long to count is the item's text.
fn second_of(said: &str) -> Option<u32> {
    let (minutes, seconds) = said.split_once(':')?;
    let digits = |part: &str| part.bytes().all(|c| c.is_ascii_digit());
    if minutes.len() < 2 || seconds.len() != 2 || !digits(minutes) || !digits(seconds) {
        return None;
    }
    let seconds: u32 = seconds.parse().ok().filter(|seconds| *seconds < 60)?;
    minutes.parse::<u32>().ok()?.checked_mul(60)?.checked_add(seconds)
}

/// The times of a body's Actions and Key points: where each ` [mm:ss]` lies, its space included,
/// and its second. Only an item's last characters, in the item's own words: a sub-item, a quote or
/// a code span holds none, and an item that is nothing else is its own text.
fn times(body: &str) -> Vec<(Range<usize>, u32)> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let mut found = Vec::new();
    let (mut titles, mut depth) = (0, 0);
    let (mut item, mut words, mut text_ended) = (None, None, None);
    for (event, range) in Parser::new(body).into_offset_iter() {
        if depth > 0 {
            words.get_or_insert(range.start);
        }
        match &event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2, ..
            }) => titles += 1,
            Event::Start(Tag::Item) => {
                if depth == 0 && titles >= 2 {
                    item = Some(range.clone());
                    words = None;
                }
                depth += 1;
            }
            Event::Start(Tag::BlockQuote(_)) => depth += 1,
            Event::End(TagEnd::BlockQuote(_)) => depth -= 1,
            Event::End(TagEnd::Item) => depth -= 1,
            _ => {}
        }
        text_ended = match event {
            Event::Text(_) => Some(range.end),
            Event::End(TagEnd::Paragraph | TagEnd::Item) => text_ended,
            _ => None,
        };
        if depth == 0 && matches!(event, Event::End(TagEnd::Item)) {
            let Some(whole) = item.take() else { continue };
            let written = body[whole.clone()].trim_end_matches(['\n', '\r']);
            let end = whole.start + written.len();
            let Some(said) = written.strip_suffix(']').filter(|_| text_ended == Some(end)) else {
                continue;
            };
            let Some(open) = said.rfind(" [") else { continue };
            let bracket = whole.start + open + 1;
            if words.is_some_and(|words| words < bracket)
                && let Some(second) = second_of(&said[open + 2..])
            {
                found.push((bracket - 1..end, second));
            }
        }
    }
    found
}

/// The front matter's keys, of minutes of a version read here, and the body after it.
fn front_matter(text: &str) -> Option<(std::collections::HashMap<&str, &str>, &str)> {
    let rest = text.strip_prefix("---\n")?;
    let (front, body) = rest.split_once("\n---\n")?;
    let mut fields = std::collections::HashMap::new();
    for line in front.lines() {
        let (key, written) = line.split_once(':')?;
        fields.insert(key.trim(), written);
    }
    let contract = fields.get("contract").and_then(|written| value(written));
    matches!(contract.as_deref(), Some("minutes/v1" | "minutes/v2" | "minutes/v3")).then_some((fields, body))
}

/// The sections, each on a title of second level however it is written: what the producer judged
/// before writing, read by the same reader.
fn sections_in(body: &str) -> usize {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

    Parser::new(body)
        .filter(|event| {
            matches!(
                event,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                })
            )
        })
        .count()
}

fn parse_minutes(text: &str, sections: [&str; 3]) -> Option<Minutes> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let (fields, body) = front_matter(text)?;
    let field = |key: &str| fields.get(key).and_then(|written| value(written));
    if sections_in(body) != 3 {
        return None;
    }
    // What v3 adds: to the others a key they do not know, and their brackets' text.
    let third = field("contract").as_deref() == Some("minutes/v3");
    let times = if third { times(body) } else { Vec::new() };
    let price = if third {
        priced(fields.get("cost").copied(), fields.get("currency").copied())
    } else {
        None
    };
    // Each section titled as the page names it: the order is the contract's, the words the
    // model's own — a small one numbers them (#261).
    let mut in_title = false;
    let time_at = |range: &Range<usize>| times.iter().find(|(at, _)| at.start < range.start && range.end <= at.end);
    let shown = Parser::new(body).into_offset_iter().filter_map(move |(event, range)| match event {
        Event::Start(Tag::Heading {
            level: HeadingLevel::H2, ..
        }) => {
            in_title = true;
            Some(event)
        }
        // A time's words, in as many pieces as the reader cut them: the mark in place of the first.
        Event::Text(_) if time_at(&range).is_some() => time_at(&range).and_then(|(at, second)| {
            (range.start == at.start + 1).then(|| {
                let said = &body[at.start + 2..at.end - 1];
                Event::InlineHtml(format!("<time data-second=\"{second}\">{said}</time>").into())
            })
        }),
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
    // Each title's line said as the page names it, each time left out; every other byte as the
    // model wrote it.
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
    let titled = sections.iter().zip(titled).map(|(name, range)| (range, format!("## {name}\n")));
    let left_out = times.iter().map(|(at, _)| (at.clone(), String::new()));
    let mut changes: Vec<_> = titled.chain(left_out).collect();
    changes.sort_by_key(|(range, _)| range.start);
    for (range, said) in changes {
        markdown.push_str(&body[kept..range.start]);
        markdown.push_str(&said);
        kept = range.end;
    }
    markdown.push_str(&body[kept..]);
    let markdown = format!("{}\n", markdown.trim());
    Some(Minutes {
        made: field("made"),
        provider: field("provider"),
        model: field("model"),
        context: field("context"),
        title: field("title").and_then(within),
        cost: price.as_ref().map(|(cost, _)| *cost),
        currency: price.map(|(_, currency)| currency),
        body: html,
        markdown,
    })
}
