//! What `summary` reads and writes, with no provider in sight: the transcript of a session as
//! `contracts/transcript/v1` describes it, and its minutes as `contracts/minutes/v3` does.
//!
//! The minutes exist whole or not at all: an answer that is not minutes is refused before a
//! byte is written, and the file takes its name only once it is complete.

use std::{fs, io, path::Path};

pub mod log;
pub mod provider;

/// The contract this module reads, as the transcript's first line names it.
pub const TRANSCRIPT: &str = "transcript/v1";
/// The contract this module provides, as the minutes' front matter names it.
pub const MINUTES: &str = "minutes/v3";

/// A stretch of speech, as the transcript holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub speaker: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Why something a file or a provider gave was refused: one sentence, for the user — and,
/// when a person acts on the cause differently, its code as `summarising/v1` lists it.
#[derive(Debug, PartialEq, Eq)]
pub struct Refused(pub String, pub Option<&'static str>);

fn refused<T>(code: &'static str, why: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(why.into(), Some(code)))
}

/// The segments of a transcript, in the order of the meeting. A line still being written —
/// no line feed yet — is left out; a type or a field this version does not know is skipped.
pub fn read_transcript(text: &str) -> Result<Vec<Segment>, Refused> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(end) = text.rfind('\n') else {
        return refused(
            "transcript.unusable",
            "the transcript holds no whole line yet: it is only being started, or it is empty",
        );
    };
    let whole = &text[..end];
    let mut lines = whole.lines();
    let header: serde_json::Value = lines.next().and_then(|line| serde_json::from_str(line).ok()).unwrap_or_default();
    if header["type"] != "header" || header["contract"] != TRANSCRIPT {
        return refused(
            "transcript.unusable",
            format!("this file is not a transcript this version reads: its first line does not name {TRANSCRIPT}"),
        );
    }
    let mut segments = Vec::new();
    let mut known = std::collections::HashSet::new();
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return refused("transcript.unusable", "the transcript holds a line that is not JSON");
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
            return refused(
                "transcript.unusable",
                "the transcript holds a segment without its speaker, its times or its text",
            );
        };
        if text.contains(['\n', '\r']) {
            return refused(
                "transcript.unusable",
                format!("the transcript holds a segment whose text is on more than one line, at {start}"),
            );
        }
        // What the transcript holds is never said back: a failure's sentence is logged (ADR-0007).
        if !matches!(speaker, "me" | "others") {
            return refused(
                "transcript.unusable",
                format!("the transcript holds a segment of a speaker other than me or others, at {start}"),
            );
        }
        if end < start || start < 0.0 {
            return refused(
                "transcript.unusable",
                format!("the transcript holds a segment this version refuses: {start} to {end}"),
            );
        }
        // A segment is known by its speaker and its start: said twice, it is read once.
        // Adding zero makes the two zeros one: -0.0 and 0.0 are the same start.
        if known.insert((speaker == "me", (start + 0.0).to_bits())) {
            segments.push(Segment {
                speaker: speaker.into(),
                start,
                end,
                text: text.into(),
            });
        }
    }
    if segments.is_empty() {
        return refused(
            "transcript.no_speech",
            "the transcript holds no speech: there is nothing to write minutes of",
        );
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(segments)
}

/// What a test of a provider sends: one sentence of the program's own, nothing of a meeting.
pub const TEST: &str = "Answer with the single word OK.";

/// What a provider is asked: the rules, the context the user attached, the meeting.
pub fn request(segments: &[Segment], context: Option<&str>) -> String {
    let mut asked = String::from(
        "You write the minutes of a meeting from its transcript.\n\
         Write everything in the language the meeting is held in, the three titles included: \
         never use an English word of these instructions as a title.\n\
         Write only what the transcript says: invent no name, no date, no decision.\n\
         When the meeting did not settle who does a task or by when, say so in the meeting's \
         language — \"not specified\", or that it was proposed and not accepted. Never guess.\n\
         A person the meeting talks about is not a participant: never give them an action.\n\
         In the transcript, \"me\" is what the recording machine's microphone heard — the \
         person who recorded and whoever sat beside them — and \"others\" is everyone heard \
         through the machine. These two are the transcript's words, not the meeting's: never \
         write the words \"me\" or \"others\" in the minutes — write, in the meeting's language, \
         the person who recorded and the other participants, or the names the meeting or the \
         context gives.\n\n\
         The transcript was written by a speech recogniser, and holds its errors: a word heard \
         as another that sounds alike, a name or a figure misheard, a sentence written twice or \
         several times in a row, a sentence of \"others\" written again under \"me\" as an echo, \
         a reply missing. Read the meeting through them: where what was meant is certain from \
         the rest of the meeting, write what was meant; where a name, a figure, a date or who \
         does a task is uncertain, write it as heard and say it is uncertain. A sentence written \
         twice counts once. The user's context, when there is one, names who was there and the \
         words of the trade: read the names, the words and who speaks on which side through \
         it. A task said as \"I\" names nobody, and each side may hold several people: give it \
         an owner without doubt only when the meeting names them; otherwise write the likely \
         owner with \"probably\", or none.\n\n\
         Open your answer on one line that names the meeting: a title of first level — a line \
         starting with one number sign and a space, in plain text —, a few words in the \
         meeting's language saying what it was about, at most 80 characters, never a date, without a slash.\n\
         Then answer with exactly three sections and nothing between or after them. Each \
         opens on a title of second level — a line starting with two number signs and a space, \
         in plain text — in this order:\n\
         1. What the meeting was for and what came out of it, in a few sentences.\n\
         2. Every task the meeting agreed on or asked for, one bullet each, even when nobody \
         took it and no date was set: what is to be done — who does it — by when. Go through \
         the whole transcript before writing this section: a task said once counts, and so \
         does one said as a need — something that must be done, prepared, sent again or kept \
         in reserve. A task belongs here, not in the third section.\n\
         3. What was decided, the figures, the dates, what stays open, one bullet each.\n\
         A section with nothing to say says that there is none.\n\
         End each bullet of the second and third sections with one time, as [mm:ss] copied from \
         the transcript, of the passage it came from: never a list, nothing after it, and never \
         make one up.\n\n",
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
    /// When: written to the second, with its offset.
    pub at: chrono::DateTime<chrono::FixedOffset>,
    /// The host that answered, or `local` — never a key, never a path.
    pub provider: &'a str,
    pub model: &'a str,
    pub context: Option<&'a str>,
    /// What the call cost, when the provider reported it (`reported`).
    pub cost: Option<Cost>,
    /// The transcript the times of the answer are checked against.
    pub segments: &'a [Segment],
}

/// A price as a provider reported it, its number's text untouched: made by [`reported`] alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cost {
    amount: String,
    currency: &'static str,
}

/// The one host whose cost is read, and the currency its documentation names: "a credit system
/// where the base currency is US dollars" (openrouter.ai/docs/faq, read 2026-11-01).
const PRICED: (&str, &str) = ("openrouter.ai", "USD");

/// The cost in the `usage` of an answer from `host`, as reported (`minutes/v3`, D51): `Err` says
/// why one that was there is not written, for the log. Never estimated.
pub fn reported(host: &str, usage: &serde_json::Value) -> Result<Option<Cost>, &'static str> {
    let cost = &usage["cost"];
    // The user's own key at the model's provider: what was charged here is not what the call cost.
    let upstream = &usage["cost_details"]["upstream_inference_cost"];
    let own_key = usage["is_byok"] == true || !(upstream.is_null() || upstream.as_f64() == Some(0.0));
    if cost.is_null() || host == "local" || own_key {
        return Ok(None);
    }
    if host != PRICED.0 {
        return Err("a provider whose currency no documentation read here names");
    }
    match cost
        .as_number()
        .filter(|number| number.as_f64().is_some_and(|amount| amount >= 0.0))
    {
        Some(number) => Ok(Some(Cost {
            amount: number.to_string(),
            currency: PRICED.1,
        })),
        None => Err("not a number at least zero"),
    }
}

/// Minutes this module accepted: the only thing [`write_minutes`] writes.
#[derive(Debug, PartialEq, Eq)]
pub struct Minutes(String, usize);

impl Minutes {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// How many times the answer held that were not written.
    pub fn removed(&self) -> usize {
        self.1
    }
}

/// Why an answer is not three sections of second level and nothing else — `None` when it is.
/// Read by a CommonMark reader, as whoever opens the file will read it: a title is a title
/// however it is written, and wherever it hides.
fn not_minutes(body: &str) -> Option<String> {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    // Letters to Unicode, and nothing to a reader: the fillers a text is padded with.
    let shows = |c: char| c.is_alphanumeric() && !matches!(c, '\u{3164}' | '\u{115f}' | '\u{1160}' | '\u{ffa0}');
    let says = |text: &str| text.chars().any(shows);
    let mut opened = false;
    let (mut inside, mut title, mut sub_title) = (0, None, false);
    let mut said: Vec<bool> = Vec::new();
    for event in Parser::new(body) {
        // Whatever comes before the first section — a sentence, a sub-title, a list, a
        // picture, a mark — is not minutes; what has a reason of its own keeps it.
        let opens = matches!(
            event,
            Event::Start(
                Tag::Heading {
                    level: HeadingLevel::H1 | HeadingLevel::H2,
                    ..
                } | Tag::CodeBlock(_)
                    | Tag::HtmlBlock
            ) | Event::Html(_)
        );
        if !std::mem::replace(&mut opened, true) && !opens {
            return Some("it does not open on its first section".into());
        }
        match event {
            Event::Start(Tag::BlockQuote(_) | Tag::List(_) | Tag::Item) => inside += 1,
            Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item) => inside -= 1,
            Event::Start(Tag::CodeBlock(_)) => return Some("it holds a block of code".into()),
            Event::Start(Tag::HtmlBlock) | Event::Html(_) | Event::InlineHtml(_) => return Some("it holds HTML".into()),
            Event::Start(Tag::Heading { .. }) if inside > 0 => return Some("it holds a title inside a quotation or a list".into()),
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1, ..
            }) => return Some("it holds a title above its sections".into()),
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2, ..
            }) => {
                said.push(false);
                title = Some(false);
            }
            Event::Start(Tag::Heading { .. }) => sub_title = true,
            Event::End(TagEnd::Heading(_)) => {
                if title.take() == Some(false) {
                    return Some("a section has no title".into());
                }
                sub_title = false;
            }
            Event::Text(text) | Event::Code(text) if says(&text) => match (&mut title, said.last_mut()) {
                (Some(named), _) => *named = true,
                (None, _) if sub_title => {}
                (None, Some(section)) => *section = true,
                (None, None) => return Some("it does not open on its first section".into()),
            },
            _ => {}
        }
    }
    if said.len() != 3 {
        return Some(format!("{} section(s) of second level where three are expected", said.len()));
    }
    said.contains(&false).then(|| "a section is empty".into())
}

/// An answer without the one title of first level it opens on (#353, D48): never judged as a
/// section; said apart, the meeting's name when `minutes/v3` lets it be one. Any other title
/// stays, and is judged.
fn without_title(body: &str) -> (Option<String>, &str) {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};

    match Parser::new(body).into_offset_iter().next() {
        Some((
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1, ..
            }),
            range,
        )) if body[range.clone()].chars().any(char::is_alphanumeric) => (title(&body[range.clone()]), body[range.end..].trim_start()),
        _ => (None, body),
    }
}

/// The most a title holds, in characters (`minutes/v3`).
const TITLE_MOST: usize = 80;

/// The words of a title of first level, its marks left out, when `minutes/v3` lets them name a
/// meeting: on one line, at most 80 characters, no path. Otherwise none: the minutes are written
/// without it, never refused for it. A key is the program's to refuse, in the whole answer.
fn title(written: &str) -> Option<String> {
    use pulldown_cmark::{Event, Parser};

    let mut words = String::new();
    for event in Parser::new(written) {
        match event {
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => words.push_str(&text),
            Event::SoftBreak | Event::HardBreak => return None,
            _ => {}
        }
    }
    let words = words.trim();
    // Whether words name a path no rule can tell: a slash of any kind leaves the date to name it,
    // « Q3/Q4 » too — a meeting loses a name, never shows a path (`minutes/v3`).
    let path = words.contains(['/', '\\']);
    let one_line = !words.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'));
    let says = words
        .chars()
        .any(|c| c.is_alphanumeric() && !matches!(c, '\u{3164}' | '\u{115f}' | '\u{1160}' | '\u{ffa0}'));
    (one_line && !path && says && words.chars().count() <= TITLE_MOST).then(|| words.to_string())
}

/// The seconds of a time as `minutes/v3` writes one: minutes in two digits or more, seconds from
/// 00 to 59. Anything else, or a time no number can hold, is text.
fn seconds(time: &str) -> Option<u64> {
    let (minutes, seconds) = time.split_once(':')?;
    let digits = |text: &str| text.bytes().all(|b| b.is_ascii_digit());
    if minutes.len() < 2 || seconds.len() != 2 || !digits(minutes) || !digits(seconds) {
        return None;
    }
    let seconds: u64 = seconds.parse().ok().filter(|seconds| *seconds < 60)?;
    minutes.parse::<u64>().ok()?.checked_mul(60)?.checked_add(seconds)
}

/// The times in these square brackets: `[12:31]`, or a list however its times are joined —
/// `[12:31, 14:02]`, `[12:31 et 14:02]`. One time among other words is text.
fn times_in(inside: &str) -> Vec<u64> {
    if let Some(time) = seconds(inside) {
        return vec![time];
    }
    let times: Vec<u64> = inside
        .split(|c: char| !c.is_alphanumeric() && c != ':')
        .filter_map(seconds)
        .collect();
    if times.len() > 1 { times } else { Vec::new() }
}

/// Where each pair of square brackets holding times sits in `text`, with those times.
fn times_of(text: &str) -> Vec<(std::ops::Range<usize>, Vec<u64>)> {
    let mut found = Vec::new();
    for (open, _) in text.match_indices('[') {
        if let Some(close) = text[open..].find(']').map(|close| open + close)
            && !text[open + 1..close].contains('[')
        {
            let times = times_in(&text[open + 1..close]);
            if !times.is_empty() {
                found.push((open..close + 1, times));
            }
        }
    }
    found
}

/// What `minutes/v3` lets an item of Actions or Key points write of its times: one, in a segment,
/// ending the item. More than one, anywhere in it, is a list, and none stays; one elsewhere than
/// at its end is its text. `text` is the item from its mark to its own end: the answer is what to
/// replace in it, each with what, and how many times went. The last replacement reaching the end
/// of the text means its end, the marks a time left there included.
fn item_times(text: &str, segments: &[Segment]) -> (Vec<(std::ops::Range<usize>, String)>, usize) {
    let found = times_of(text);
    let count = found.iter().map(|(_, times)| times.len()).sum::<usize>();
    let ends = |from: usize| {
        let rest = text[from..].trim_end();
        rest.is_empty() || rest == "."
    };
    // Spaces and the marks a list is written with go with what they stood before, at the end.
    let before_end = |from: usize| {
        text[..from]
            .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';'))
            .len()
    };
    let before = |from: usize| text[..from].trim_end().len();
    let end = text.trim_end().len();
    match &found[..] {
        [] => (Vec::new(), 0),
        [(range, times)] if count == 1 => {
            if !ends(range.end) {
                return (Vec::new(), 0);
            }
            let inside = segments
                .iter()
                .any(|segment| segment.start as u64 <= times[0] && times[0] <= segment.end as u64);
            if inside {
                (vec![(before(range.start)..end, format!(" {}", &text[range.clone()]))], 0)
            } else {
                (vec![(before_end(range.start)..end, String::new())], 1)
            }
        }
        _ => {
            // Text between two times is the line's and stays: only spaces and a list's commas go.
            let list_mark = |c: char| c.is_whitespace() || matches!(c, ',' | ';');
            let mut spans: Vec<std::ops::Range<usize>> = Vec::new();
            for (range, _) in &found {
                match spans.last_mut() {
                    Some(last) if text[last.end..range.start].split_whitespace().count() <= 1 => {
                        last.end = range.end;
                    }
                    _ => spans.push(range.clone()),
                }
            }
            let edits = spans
                .into_iter()
                .map(|span| {
                    if ends(span.end) {
                        (before_end(span.start)..end, String::new())
                    } else {
                        (before(span.start)..span.end, String::new())
                    }
                })
                .collect();
            (edits, count)
        }
    }
}

/// The body with only the times `minutes/v3` lets it write, and how many went. Read by the
/// CommonMark reader: only the items under the second and third titles hold one, each its own.
fn with_times_checked(body: &str, segments: &[Segment]) -> (String, usize) {
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

    let (mut section, mut open, mut edits, mut removed) = (0, Vec::<(usize, Option<usize>)>::new(), Vec::new(), 0);
    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2, ..
            }) => section += 1,
            Event::Start(Tag::Item) => open.push((range.start, None)),
            // An item's own text stops where the list inside it starts.
            Event::Start(Tag::List(_)) => {
                if let Some((_, own @ None)) = open.last_mut() {
                    *own = Some(range.start);
                }
            }
            Event::End(TagEnd::Item) => {
                let Some((start, own)) = open.pop() else { continue };
                let own = &body[start..own.unwrap_or(range.end)];
                if section >= 2 {
                    let (replaced, went) = item_times(own, segments);
                    removed += went;
                    for (range, written) in replaced {
                        // At its text's end: the spaces that follow on its line go too.
                        let mut end = start + range.end;
                        if range.end == own.trim_end().len() {
                            end += body[end..].len() - body[end..].trim_start_matches(|c: char| c.is_whitespace() && c != '\n').len();
                        }
                        edits.push((start + range.start..end, written));
                    }
                }
            }
            _ => {}
        }
    }
    edits.sort_by_key(|(range, _): &(std::ops::Range<usize>, String)| std::cmp::Reverse(range.start));
    let mut checked = body.to_string();
    for (range, written) in edits {
        checked.replace_range(range, &written);
    }
    (checked, removed)
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

/// The minutes file for a provider's answer — or the reason the answer is not minutes.
pub fn minutes(answer: &str, made: &Made) -> Result<Minutes, Refused> {
    // One way to end a line, before anything reads it: readers do not agree on a lone
    // carriage return, and the file is written with the line ends it was judged with.
    let unified = answer.replace("\r\n", "\n").replace('\r', "\n");
    let (title, body) = without_title(unified.trim().trim_start_matches('\u{feff}').trim());
    // Judged as it will be written: a section that held only a time it may not keep is empty.
    let (body, removed) = with_times_checked(body, made.segments);
    if let Some(why) = not_minutes(&body) {
        return refused("answer.not_minutes", format!("the provider's answer is not minutes: {why}"));
    }
    let context = made
        .context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .map_or("null".into(), quoted);
    let named = title.map_or(String::new(), |title| format!("title: {}\n", quoted(&title)));
    let priced = made.cost.as_ref().map_or(String::new(), |cost| {
        format!("cost: {}\ncurrency: {}\n", cost.amount, cost.currency)
    });
    Ok(Minutes(
        format!(
            "---\ncontract: {MINUTES}\nmade: {}\nprovider: {}\nmodel: {}\ncontext: {context}\n{named}{priced}---\n\n{body}\n",
            made.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            quoted(made.provider),
            quoted(made.model),
        ),
        removed,
    ))
}

/// Puts the minutes in the session's folder: under their name only once they are whole. The
/// minutes that were there are replaced; a failure before the last step leaves them as they were.
pub fn write_minutes(folder: &Path, minutes: &Minutes) -> io::Result<()> {
    use std::io::Write;
    let draft = folder.join("minutes.md.part");
    // What sits at the draft's name — one left by a crash, or a link — goes first: the draft is
    // made new, never written through a link into another file.
    match fs::remove_file(&draft) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    // Whatever fails, the draft goes: written in part or not renamed, it is no minutes.
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&draft)
        .and_then(|mut file| file.write_all(minutes.as_str().as_bytes()))
        .and_then(|()| fs::rename(&draft, folder.join("minutes.md")))
        .inspect_err(|_| {
            let _ = fs::remove_file(&draft);
        })
}
