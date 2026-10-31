//! D67: a command of the page that fails says so, or is named here with why it says nothing.

use std::collections::BTreeMap;

/// The handlers that stay silent: what the page asked — the command, or the name of what it asked —,
/// how many times, and why.
const SILENT: &[(&str, usize, &str)] = &[
    ("page_failed", 1, "the log itself: nowhere left to say it"),
    ("makings", 2, "returns nothing that can fail: the handler is for a window closing"),
    (
        "header",
        1,
        "returns nothing that can fail, and is asked again in a second and a half",
    ),
    (
        "recording_now",
        1,
        "returns nothing that can fail, and is asked again at the next tick",
    ),
    (
        "listed",
        1,
        "the small window's rest, asked again at the next tick; a folder unread is said in what it returns",
    ),
    (
        "model_fetch",
        2,
        "returns nothing that can fail: how the fetch goes is said where it is shown",
    ),
    (
        "ollama_pull_offered",
        1,
        "returns nothing that can fail: how the fetch goes is said where it is shown",
    ),
    (
        "asked",
        1,
        "model_offered_fetch or ollama_pull_offered: both return nothing that can fail",
    ),
    ("speech_shown", 2, "Apple's model is an offer, simply absent when it cannot be read"),
    (
        "speech_installed",
        1,
        "refused only while macOS is already fetching it: that fetch is said",
    ),
    (
        "context_shown",
        2,
        "a meeting gone: its page goes back to the list at the next drawing",
    ),
    ("context_saved", 1, "the small window's: tried again at the next tick"),
    ("confidential_set", 1, "the small window's: tried again at the next tick"),
    ("agreement_shown", 1, "a meeting gone: the page goes back to the list"),
    ("opened", 1, "a meeting gone: the page goes back to the list"),
];

/// Where the bracket opened at `open` closes.
fn closing(text: &[u8], open: usize) -> usize {
    let (up, down) = (text[open], if text[open] == b'(' { b')' } else { b'}' });
    let mut depth = 0;
    for (at, &byte) in text.iter().enumerate().skip(open) {
        if byte == up {
            depth += 1;
        } else if byte == down {
            depth -= 1;
            if depth == 0 {
                return at;
            }
        }
    }
    panic!("a bracket at byte {open} never closes");
}

/// The statement ending at `end`: back to the `;`, or the bracket opened, outside its own brackets.
fn statement(text: &str, end: usize) -> &str {
    let mut depth = 0;
    for (at, byte) in text.as_bytes()[..end].iter().enumerate().rev() {
        match byte {
            b')' | b'}' => depth += 1,
            b'(' | b'{' if depth > 0 => depth -= 1,
            b'(' | b'{' => return &text[at + 1..end],
            b';' if depth == 0 => return &text[at + 1..end],
            _ => {}
        }
    }
    &text[..end]
}

/// The last command `text` asks for.
fn command(text: &str) -> Option<String> {
    let start = text.rfind("invoke(")? + "invoke(".len();
    let rest = &text[start..];
    let end = rest.find([',', ')'])?;
    Some(rest[..end].trim().trim_matches('"').to_string())
}

/// A handler says it when it draws a sentence of the page's words or the one the command told, or
/// logs it. Its own parameter is no reading: only what follows the arrow.
fn says(handler: &str) -> bool {
    let body = handler.split_once("=>").map_or(handler, |(_, body)| body);
    body.contains("words.") || body.contains("why") || body.contains("pageFailed(")
}

/// What the page asks whose failure says nothing, and how many times.
fn silent(page: &str) -> BTreeMap<String, usize> {
    let bytes = page.as_bytes();
    let mut found = BTreeMap::new();
    for (at, _) in page.match_indices(".catch(") {
        let open = at + ".catch".len();
        let handler = &page[open..=closing(bytes, open)];
        // A function by its name, a media element's own: not a command's answer.
        if !handler.starts_with("((") && !handler.contains("=>") {
            continue;
        }
        let asked = statement(page, at);
        let name = command(asked).unwrap_or_else(|| {
            asked
                .trim()
                .trim_start_matches("await ")
                .split(['.', ' '])
                .next()
                .unwrap_or_default()
                .to_string()
        });
        if !says(handler) {
            *found.entry(name).or_insert(0) += 1;
        }
    }
    for (at, _) in page.match_indices("} catch") {
        // The body of its `try`, read back from the bracket that closes it.
        let mut depth = 0;
        let start = (0..=at)
            .rev()
            .find(|&back| {
                match bytes[back] {
                    b'}' => depth += 1,
                    b'{' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .expect("a try's bracket");
        // No command in it: the parse of an address, the clipboard.
        let Some(name) = command(&page[start..at]) else { continue };
        // After `catch {`, or `catch (why) {`.
        let open = at + 1 + page[at + 1..].find('{').expect("a catch's bracket");
        if !says(&page[open..=closing(bytes, open)]) {
            *found.entry(name).or_insert(0) += 1;
        }
    }
    found
}

#[test]
fn a_command_that_fails_says_so_or_is_named_here_with_why() {
    let found = silent(include_str!("../ui/window.js"));
    let named: BTreeMap<String, usize> = SILENT.iter().map(|(name, count, _)| (name.to_string(), *count)).collect();
    assert_eq!(
        found, named,
        "a command's failure that says nothing: say it, or name it in SILENT with why"
    );
}

#[test]
fn the_reading_tells_what_says_from_what_does_not() {
    let page = r#"
        const a = await invoke("quiet", { x }).catch(() => []);
        const b = await invoke("told").catch((why) => { said.textContent = fill(words.no, why); });
        try { await invoke("hushed", { y: {} }); } catch { }
        try { await invoke("spoken"); } catch (why) { said.textContent = words.no; }
        try { new URL(text); } catch {}
        audio.play().catch(failed);
        await invoke("ignored").catch((why) => {});
        await invoke("drawn").catch((why) => { said.textContent = why; });
    "#;
    let expected: BTreeMap<String, usize> = [("hushed".to_string(), 1), ("ignored".to_string(), 1), ("quiet".to_string(), 1)].into();
    assert_eq!(silent(page), expected);
}
