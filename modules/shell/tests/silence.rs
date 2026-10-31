//! D67: a command of the page that fails says so, or is named here with why it says nothing.

use std::collections::BTreeMap;

/// The handlers that stay silent: what the page asked — the command, or the name of what it asked —,
/// how many times, and why.
const SILENT: &[(&str, usize, &str)] = &[
    ("page_failed", 1, "the log itself: nowhere left to say it"),
    ("makings", 2, "returns no `Result`: it cannot fail"),
    ("header", 1, "returns no `Result`: it cannot fail"),
    ("recording_now", 1, "returns no `Result`: it cannot fail"),
    (
        "model_fetch",
        2,
        "returns no `Result`: how the fetch goes is said where it is shown",
    ),
    (
        "ollama_pull_offered",
        1,
        "returns no `Result`: how the fetch goes is said where it is shown",
    ),
    ("asked", 1, "model_offered_fetch or ollama_pull_offered: neither returns a `Result`"),
    (
        "registry_refresh",
        1,
        "`.then(() => true, () => false)`: the answer is said by the lines after it",
    ),
    (
        "speech_installed",
        1,
        "`.then(() => true, () => false)`: the answer is said by the lines after it; the assistant's logs it",
    ),
    (
        "listed",
        1,
        "the small window's rest: fails only if its task dies, logged by `told`; asked again at the next tick",
    ),
    (
        "speech_shown",
        2,
        "an offer: returns `Ok` on what it reads, and is simply absent otherwise",
    ),
    (
        "context_shown",
        2,
        "a meeting gone, logged by `told`: its flag and its text stay as they were",
    ),
    (
        "opened",
        1,
        "a meeting gone, logged by `told`: most often sent to the trash here, and the list shows it",
    ),
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

/// The statement ending at `end`: back to the `;`, a block closed, or the bracket opened, outside its own brackets.
fn statement(text: &str, end: usize) -> &str {
    let mut depth = 0;
    for (at, byte) in text.as_bytes()[..end].iter().enumerate().rev() {
        match byte {
            // A block closed before this statement: it ended there.
            b'}' if depth == 0 => return &text[at + 1..end],
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

/// `page` without its comments, its length unchanged. A string holding `//` — an address — keeps it.
fn without_comments(page: &str) -> String {
    let mut bytes = page.as_bytes().to_vec();
    let mut at = 0;
    let mut quote = None;
    while at + 1 < bytes.len() {
        match (quote, bytes[at], bytes[at + 1]) {
            (Some(open), byte, _) => {
                if byte == b'\\' {
                    at += 1;
                } else if byte == open {
                    quote = None;
                }
            }
            (None, b'"' | b'\'' | b'`', _) => quote = Some(bytes[at]),
            (None, b'/', b'/') => {
                while at < bytes.len() && bytes[at] != b'\n' {
                    bytes[at] = b' ';
                    at += 1;
                }
            }
            (None, b'/', b'*') => {
                while at + 1 < bytes.len() && !(bytes[at] == b'*' && bytes[at + 1] == b'/') {
                    bytes[at] = b' ';
                    at += 1;
                }
                bytes[at] = b' ';
                bytes[at + 1] = b' ';
            }
            _ => {}
        }
        at += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A handler says it when it uses a sentence — its words, or the one the command told — or logs it.
/// Holding one is not using it: `const said = words.none`, `void why`.
fn says(handler: &str) -> bool {
    let body = handler.split_once("=>").map_or(handler, |(_, body)| body);
    body.contains("pageFailed(")
        || body
            .split([';', '{', '}'])
            .map(str::trim)
            .any(|step| !step.starts_with("const ") && !step.starts_with("void ") && (step.contains("words.") || step.contains("why")))
}

/// The second argument of a `.then(`, when it has one.
fn second_argument(arguments: &str) -> Option<&str> {
    let mut depth = 0;
    for (at, byte) in arguments.bytes().enumerate() {
        match byte {
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => depth -= 1,
            b',' if depth == 1 => return Some(arguments[at + 1..arguments.len() - 1].trim()),
            _ => {}
        }
    }
    None
}

/// What the page asks whose failure says nothing, and how many times.
fn silent(page: &str) -> BTreeMap<String, usize> {
    let page = without_comments(page);
    let page = page.as_str();
    let bytes = page.as_bytes();
    let mut found = BTreeMap::new();
    let mut count = |name: String, handler: &str| {
        if !says(handler) {
            *found.entry(name).or_insert(0) += 1;
        }
    };
    for (at, _) in page.match_indices(".catch(") {
        let open = at + ".catch".len();
        let handler = &page[open..=closing(bytes, open)];
        let asked = statement(page, at);
        match command(asked) {
            Some(name) => count(name, handler),
            // A function by its name, a media element's own: not a command's answer.
            None if handler.contains("=>") => count(
                asked
                    .trim()
                    .trim_start_matches("await ")
                    .split(['.', ' '])
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                handler,
            ),
            None => {}
        }
    }
    for (at, _) in page.match_indices(".then(") {
        let open = at + ".then".len();
        let arguments = &page[open..=closing(bytes, open)];
        if let (Some(handler), Some(name)) = (second_argument(arguments), command(statement(page, at))) {
            count(name, handler);
        }
    }
    let catches = page.match_indices("catch").filter(|(at, _)| page[..*at].trim_end().ends_with('}'));
    for (at, _) in catches {
        // The body of its `try`, read back from the bracket that closes it.
        let close = page[..at].trim_end().len() - 1;
        let mut depth = 0;
        let start = (0..=close)
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
        let Some(name) = command(&page[start..close]) else { continue };
        // After `catch {`, or `catch (why) {`.
        let open = at + page[at..].find('{').expect("a catch's bracket");
        count(name, &page[open..=closing(bytes, open)]);
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
        invoke("then1").then(ok, () => {});
        invoke("then2").then(null, () => []);
        invoke("thenSaid").then(ok, (why) => { said.textContent = why; });
        invoke("thenOnly").then(ok);
        invoke("function").catch(function () {});
        invoke("named").catch(noop);
        try { await invoke("tight"); }catch {}
        try { await invoke("newline"); }
        catch {}
        try { await invoke("voided"); } catch (why) { void why; }
        invoke("commented").catch(() => { /* why not */ });
        invoke("held").catch(() => { const q = words.none; });
    "#;
    let expected: BTreeMap<String, usize> = [
        "commented",
        "function",
        "held",
        "hushed",
        "ignored",
        "named",
        "newline",
        "quiet",
        "then1",
        "then2",
        "tight",
        "voided",
    ]
    .map(|name| (name.to_string(), 1))
    .into();
    assert_eq!(silent(page), expected);
}
