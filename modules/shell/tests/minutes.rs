//! One meeting's minutes, read from folders made here.

use std::fs;

use wholeheard_shell::{meeting::Read, minutes};

const MINUTES: &str = "---
contract: minutes/v1
made: 2026-10-13T15:12:40+02:00
provider: local
model: mistral
context: null
---

## Points forts

La date de l'événement est fixée au jeudi 3 juin,
avec le 10 en seconde date.

## Actions

- Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril
- Pré-réserver les salles — non précisé — non précisé

## Points clés

- Réponse des intervenants attendue pour le 26 avril ; un plan B reste à trouver.
";

fn read(text: &str) -> Read<minutes::Minutes> {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("minutes.md"), text).unwrap();
    minutes::minutes(folder.path(), NAMES)
}

/// The sections as the page names them (D11's second board), whatever the model wrote.
const NAMES: [&str; 3] = ["Points saillants", "Actions", "Points clés"];

#[test]
fn the_minutes_are_shown_as_written_with_who_made_them() {
    let Read::Present(minutes) = read(MINUTES) else {
        panic!("not read")
    };
    assert_eq!(
        (minutes.provider.as_deref(), minutes.model.as_deref()),
        (Some("local"), Some("mistral"))
    );
    assert_eq!(minutes.made.as_deref(), Some("2026-10-13T15:12:40+02:00"));
    for shown in [
        "<h2>Points saillants</h2>",
        "<li>Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril</li>",
        "<li>Pré-réserver les salles — non précisé — non précisé</li>",
        "<h2>Points clés</h2>",
    ] {
        assert!(minutes.body.contains(shown), "{shown} not in {}", minutes.body);
    }
}

#[test]
fn whatever_the_producer_writes_is_shown_nothing_left_out() {
    let written = "---
contract: minutes/v1
made: 2026-10-13T15:12:40+02:00
provider: \"api.example.org\"
model: \"mod\\\"el\\nname\"
context: null
---

Faits marquants
---------------

- La date est fixée.
- Le budget aussi.

  ## Actions ##

* Appeler les intervenants

  Alexandre — 26 avril

Aucune autre action n'a été décidée.

## Points clés

Pour commencer :

1. Un premier point
2. Un second

Reste ouvert : la salle.
";
    let Read::Present(minutes) = read(written) else {
        panic!("not read")
    };
    assert_eq!(minutes.provider.as_deref(), Some("api.example.org"));
    assert_eq!(minutes.model.as_deref(), Some("mod\"el\nname"));
    for shown in [
        "<h2>Points saillants</h2>",
        "La date est fixée.",
        "<h2>Actions</h2>",
        "Appeler les intervenants",
        "Alexandre — 26 avril",
        "Aucune autre action n'a été décidée.",
        "Pour commencer :",
        "<li>Un second</li>",
        "Reste ouvert : la salle.",
    ] {
        assert!(minutes.body.contains(shown), "{shown} not in {}", minutes.body);
    }
}

#[test]
fn the_minutes_never_drive_the_page_that_shows_them() {
    let written = MINUTES.replace(
        "avec le 10 en seconde date.",
        "avec le 10 en seconde date. <script>alert(1)</script> [ici](javascript:alert(1)) ![une image](file:///etc/passwd)",
    );
    let Read::Present(minutes) = read(&written) else {
        panic!("not read")
    };
    assert!(!minutes.body.contains("<script"), "{}", minutes.body);
    assert!(minutes.body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "{}", minutes.body);
    assert!(!minutes.body.contains("<a ") && !minutes.body.contains("<img"), "{}", minutes.body);
    assert!(
        minutes.body.contains(" ici ") && minutes.body.contains("une image"),
        "{}",
        minutes.body
    );
    // A block of HTML, as a hand may write one, is text too.
    let block = MINUTES.replace("## Actions\n", "## Actions\n\n<div onclick=\"x()\">bloc</div>\n");
    let Read::Present(minutes) = read(&block) else { panic!("not read") };
    assert!(
        !minutes.body.contains("<div") && minutes.body.contains("&lt;div"),
        "{}",
        minutes.body
    );
}

#[test]
fn minutes_that_are_not_of_this_version_are_said_unreadable_and_none_are_absent() {
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(minutes::minutes(folder.path(), NAMES), Read::Absent);
    for wrong in [
        MINUTES.replacen("---\ncontract", "contract", 1),
        MINUTES.replace("minutes/v1", "minutes/v4"),
        MINUTES.replace("## Points clés", "Points clés"),
        format!("{MINUTES}\n## Encore\n"),
    ] {
        assert_eq!(read(&wrong), Read::Unreadable, "{wrong}");
    }
    // A sub-title is no section: three sections and a title below second level read.
    assert!(matches!(
        read(&MINUTES.replace("## Actions\n", "## Actions\n\n### Un détail\n")),
        Read::Present(_)
    ));
    // A value written `null`, quoted or not, is no value.
    let Read::Present(minutes) = read(&MINUTES.replace("model: mistral", "model: \"null\"")) else {
        panic!("not read")
    };
    assert_eq!(minutes.model, None);
}

#[test]
fn the_three_sections_are_named_by_the_page_whatever_titles_the_model_wrote() {
    // A small local model numbers them (#261); another dresses them up.
    for titles in [["1.", "2.", "3."], ["**Résumé** _bref_", "Actions à mener", "`Points`"]] {
        let written = MINUTES
            .replace("## Points forts", &format!("## {}", titles[0]))
            .replace("## Actions", &format!("## {}", titles[1]))
            .replace("## Points clés", &format!("## {}", titles[2]));
        let Read::Present(minutes) = read(&written) else {
            panic!("not read: {written}")
        };
        let named: Vec<&str> = minutes.body.match_indices("<h2>").map(|(at, _)| &minutes.body[at..]).collect();
        assert_eq!(named.len(), 3, "{}", minutes.body);
        for (shown, name) in named.iter().zip(NAMES) {
            assert!(shown.starts_with(&format!("<h2>{name}</h2>")), "{name}: {}", minutes.body);
        }
        // What the sections hold is shown as written.
        assert!(minutes.body.contains("Pré-réserver les salles"), "{}", minutes.body);
    }
}

#[test]
fn the_minutes_copied_are_their_markdown_without_the_front_matter_titled_as_the_page_names_them() {
    // D45: what a click puts on the clipboard — the body as written, its three titles the page's.
    let Read::Present(minutes) = read(
        &MINUTES
            .replace("## Points forts", "## 1.")
            .replace("## Points clés", "## 3. Ce qui compte"),
    ) else {
        panic!("not read")
    };
    assert_eq!(
        minutes.markdown,
        "## Points saillants\n\nLa date de l'événement est fixée au jeudi 3 juin,\navec le 10 en seconde date.\n\n## Actions\n\n\
         - Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril\n- Pré-réserver les salles — non précisé — non précisé\n\n\
         ## Points clés\n\n- Réponse des intervenants attendue pour le 26 avril ; un plan B reste à trouver.\n"
    );
    assert!(!minutes.markdown.contains("contract:") && !minutes.markdown.contains("---"));
    // What it holds as HTML stays its text: it is pasted as the model wrote it.
    let Read::Present(raw) = read(&MINUTES.replace("avec le 10", "avec <b>le 10</b>")) else {
        panic!("not read")
    };
    assert!(raw.markdown.contains("avec <b>le 10</b> en seconde date."));
}

fn titled(title: &str) -> String {
    MINUTES
        .replace("minutes/v1", "minutes/v2")
        .replace("context: null\n", &format!("context: null\ntitle: {title}\n"))
}

#[test]
fn a_title_is_read_from_minutes_v2_and_minutes_v1_have_none() {
    let Read::Present(two) = read(&titled("\"Date de l'événement : \\\"jeudi\\\"\"")) else {
        panic!("v2 not read")
    };
    assert_eq!(two.title.as_deref(), Some("Date de l'événement : \"jeudi\""));
    let Read::Present(untitled) = read(&MINUTES.replace("minutes/v1", "minutes/v2")) else {
        panic!("v2 without its optional title not read")
    };
    assert_eq!(untitled.title, None);
    let Read::Present(one) = read(MINUTES) else { panic!("v1 not read") };
    assert_eq!(one.title, None);
    // A key this version does not know is left aside, as the contract says.
    let Read::Present(more) = read(&titled("\"Un nom\"").replace("provider:", "later: 1\nprovider:")) else {
        panic!("a key not known made them unreadable")
    };
    assert_eq!(more.title.as_deref(), Some("Un nom"));
}

/// The contract's bounds, at their edge: 80 characters, not bytes.
#[test]
fn a_title_out_of_the_contract_is_no_title_and_the_minutes_still_read() {
    let longest = "é".repeat(80);
    let Read::Present(kept) = read(&titled(&format!("\"{longest}\""))) else {
        panic!("not read")
    };
    assert_eq!(kept.title.as_deref(), Some(longest.as_str()));
    for wrong in [
        format!("\"{}\"", "é".repeat(81)),
        "\"deux\\nlignes\"".to_string(),
        "\"un\\u2028saut\"".to_string(),
        "\"un\\u2029saut\"".to_string(),
        "\"une\\ttabulation\"".to_string(),
        "\"\\u200b\"".to_string(),
        "\"guillemet non fermé".to_string(),
        "\"   \"".to_string(),
        "null".to_string(),
        "\"\"".to_string(),
    ] {
        let Read::Present(minutes) = read(&titled(&wrong)) else {
            panic!("{wrong} made them unreadable")
        };
        assert_eq!(minutes.title, None, "{wrong}");
    }
}

#[test]
fn a_session_listed_says_its_minutes_title() {
    let sessions = tempfile::tempdir().unwrap();
    let write = |name: &str, minutes: Option<&str>| {
        let folder = sessions.path().join(name);
        fs::create_dir(&folder).unwrap();
        fs::write(
            folder.join("session.json"),
            r#"{"contract":"session-audio/v2","started":"2026-10-13T15:00:00+02:00"}"#,
        )
        .unwrap();
        if let Some(minutes) = minutes {
            fs::write(folder.join("minutes.md"), minutes).unwrap();
        }
    };
    write("titled", Some(&titled("\"Un nom\"")));
    write("old", Some(MINUTES));
    write("none", None);
    write("broken", Some(&titled("\"Un nom\"").replace("## Actions", "Actions")));
    let long = format!("\"{}\"", "é".repeat(81));
    write("long", Some(&titled(&long)));
    let listed = wholeheard_shell::sessions::list(sessions.path(), None);
    let title = |name: &str| listed.iter().find(|session| session.folder.ends_with(name)).unwrap().title.clone();
    assert_eq!(title("titled").as_deref(), Some("Un nom"));
    for untitled in ["old", "none", "broken", "long"] {
        assert_eq!(title(untitled), None, "{untitled}");
    }
}

/// Minutes of `minutes/v3`: what they are written with after `context`, and their three sections' items.
fn third(front: &str, actions: &str, points: &str) -> String {
    format!(
        "---\ncontract: minutes/v3\nmade: 2026-10-13T15:12:40+02:00\nprovider: \"openrouter.ai\"\nmodel: \"m\"\ncontext: null\n{front}---\n\n## 1.\n\nLe point. [01:00]\n\n## 2.\n\n{actions}\n\n## 3.\n\n{points}\n"
    )
}

fn present(text: &str) -> minutes::Minutes {
    match read(text) {
        Read::Present(minutes) => minutes,
        other => panic!("not read: {other:?}\n{text}"),
    }
}

#[test]
fn minutes_v3_read_as_v2_do_and_say_what_they_cost() {
    let priced = present(&third("title: \"Un nom\"\ncost: 0.012\ncurrency: USD\n", "- a — b — c", "- d"));
    assert_eq!(priced.title.as_deref(), Some("Un nom"));
    assert_eq!((priced.cost, priced.currency.as_deref()), (Some(0.012), Some("USD")));
    for (cost, read_as) in [("1.2e-6", 1.2e-6), ("0", 0.0), ("-0", 0.0), ("3", 3.0)] {
        let shown = present(&third(&format!("cost: {cost}\ncurrency: EUR\n"), "- a", "- d"));
        assert_eq!(shown.cost, Some(read_as), "{cost}");
        assert!(!shown.cost.unwrap().is_sign_negative(), "{cost}");
    }
    // Quoted, a currency is the same word; a value in double quotes is read as JSON.
    assert_eq!(
        present(&third("cost: 1\ncurrency: \"EUR\"\n", "- a", "- d")).currency.as_deref(),
        Some("EUR")
    );
    // Neither, the minutes as they were: no key, no price.
    let free = present(&third("", "- a", "- d"));
    assert_eq!((free.cost, free.currency), (None, None));
}

/// The contract's bounds: anything else is neither, and the minutes still read.
#[test]
fn a_cost_out_of_the_contract_is_no_cost_and_the_minutes_still_read() {
    for front in [
        "cost: 0.012\n",
        "currency: USD\n",
        "cost: -0.5\ncurrency: USD\n",
        "cost: \"0.012\"\ncurrency: USD\n",
        "cost: null\ncurrency: USD\n",
        "cost: \ncurrency: USD\n",
        "cost: .5\ncurrency: USD\n",
        "cost: +1\ncurrency: USD\n",
        "cost: 01\ncurrency: USD\n",
        "cost: 1,5\ncurrency: USD\n",
        "cost: NaN\ncurrency: USD\n",
        "cost: Infinity\ncurrency: USD\n",
        "cost: 1e999\ncurrency: USD\n",
        "cost: 12 USD\ncurrency: USD\n",
        "cost: 1\ncurrency: usd\n",
        "cost: 1\ncurrency: US\n",
        "cost: 1\ncurrency: USDT\n",
        "cost: 1\ncurrency: U5D\n",
        "cost: 1\ncurrency: ÉUR\n",
        "cost: 1\ncurrency: null\n",
        "cost: 1\ncurrency: \n",
    ] {
        let minutes = present(&third(front, "- a", "- d"));
        assert_eq!((minutes.cost, minutes.currency.as_deref()), (None, None), "{front}");
    }
}

/// Keys of v3 in an older version are keys it does not know, and its items' brackets are its text.
#[test]
fn minutes_v1_and_v2_know_no_cost_and_no_time() {
    let written =
        |version: &str| third("cost: 0.012\ncurrency: USD\n", "- a — b — c [12:31]", "- d [00:05]").replace("minutes/v3", version);
    for version in ["minutes/v1", "minutes/v2"] {
        let minutes = present(&written(version));
        assert_eq!((minutes.cost, minutes.currency.as_deref()), (None, None), "{version}");
        assert!(minutes.body.contains("<li>a — b — c [12:31]</li>"), "{}", minutes.body);
        assert!(minutes.body.contains("<li>d [00:05]</li>"), "{}", minutes.body);
        assert!(!minutes.body.contains("<time"), "{}", minutes.body);
        assert!(minutes.markdown.contains("- a — b — c [12:31]\n"), "{}", minutes.markdown);
    }
}

#[test]
fn a_time_ending_an_item_is_said_apart_and_left_out_of_what_is_copied() {
    let minutes = present(&third(
        "",
        "- Contacter — Alexandre — 26 avril [12:31]\n- Réserver — non précisé — non précisé",
        "- Un plan B reste à trouver. [00:00]\n- Sans temps.\n- Le long [120:59]",
    ));
    for shown in [
        "<li>Contacter — Alexandre — 26 avril <time data-second=\"751\">12:31</time></li>",
        "<li>Réserver — non précisé — non précisé</li>",
        "<li>Un plan B reste à trouver. <time data-second=\"0\">00:00</time></li>",
        "<li>Sans temps.</li>",
        "<li>Le long <time data-second=\"7259\">120:59</time></li>",
    ] {
        assert!(minutes.body.contains(shown), "{shown} not in {}", minutes.body);
    }
    assert_eq!(
        minutes.markdown,
        "## Points saillants\n\nLe point. [01:00]\n\n## Actions\n\n- Contacter — Alexandre — 26 avril\n- Réserver — non précisé — non précisé\n\n\
         ## Points clés\n\n- Un plan B reste à trouver.\n- Sans temps.\n- Le long\n"
    );
    // Written around other marks, or over several lines, or an item of paragraphs, or numbered.
    let minutes = present(&third(
        "",
        "1. **Fort** [00:01]\n2. sur\n   deux lignes [00:02]",
        "- un\n\n  deux [00:03]\n\n- trois [00:04]",
    ));
    for shown in [
        "<strong>Fort</strong> <time data-second=\"1\">00:01</time>",
        "deux lignes <time data-second=\"2\">00:02</time>",
        "deux <time data-second=\"3\">00:03</time></p>",
        "trois <time data-second=\"4\">00:04</time></p>",
    ] {
        assert!(minutes.body.contains(shown), "{shown} not in {}", minutes.body);
    }
    assert!(!minutes.markdown.contains("[00:0"), "{}", minutes.markdown);
    assert!(
        minutes.markdown.contains("1. **Fort**\n2. sur\n   deux lignes\n"),
        "{}",
        minutes.markdown
    );
}

/// « One time, at the end, or none »: every other bracket is the item's text, read and copied as written.
#[test]
fn anything_else_in_square_brackets_is_the_item_s_text() {
    for item in [
        "a [12:75]",
        "a [12:60]",
        "a [12:059]",
        "a [+1:30]",
        "a [12:+5]",
        "a [1:30]",
        "a [12:5]",
        "a [12:305]",
        "a [:30]",
        "a [12:]",
        "a [12:31:05]",
        "a [ab:cd]",
        "a [12:31].",
        "a [12:31] b",
        "a [12:31]  ",
        "a[12:31]",
        "[12:31]",
        "a `[12:31]`",
        "a *[12:31]*",
        "a \\[12:31]",
        "a [12:31](http://example.org)",
        "a [٠١:٣٠]",
        "a [99999999999999999999:00]",
    ] {
        let minutes = present(&third("", &format!("- {item}"), &format!("- {item}")));
        assert!(!minutes.body.contains("<time"), "{item}: {}", minutes.body);
        assert_eq!(minutes.markdown.matches(item.trim_end()).count(), 2, "{item}: {}", minutes.markdown);
    }
    // An item that is nothing else is its own text, wherever it stands in its list.
    let alone = present(&third("", "- a\n- [12:31]", "- d"));
    assert!(alone.body.contains("<li>[12:31]</li>"), "{}", alone.body);
    // An item within an item keeps its brackets; the time is its parent's own, ending the parent.
    let parent = present(&third("", "- a\n  - b [00:01]\n\n  c [00:02]", "- d"));
    assert_eq!(parent.body.matches("<time ").count(), 1, "{}", parent.body);
    assert!(parent.body.contains("b [00:01]"), "{}", parent.body);
    assert!(parent.body.contains("c <time data-second=\"2\">00:02</time>"), "{}", parent.body);
    assert!(
        parent.markdown.contains("b [00:01]") && !parent.markdown.contains("[00:02]"),
        "{}",
        parent.markdown
    );
    // Two: the last characters alone are the time, the other the item's text.
    let two = present(&third("", "- a [12:31] [14:02]", "- d"));
    assert!(
        two.body.contains("<li>a [12:31] <time data-second=\"842\">14:02</time></li>"),
        "{}",
        two.body
    );
    assert!(two.markdown.contains("- a [12:31]\n"), "{}", two.markdown);
    // Only Actions and Key points hold times, and only their own items: not Highlights, not an
    // item within an item, nor one quoted.
    let elsewhere = present(&third(
        "",
        "- a\n  - b [00:01]\n\n> - c [00:02]",
        "- d [00:03]\n  ```\n  [00:04]\n  ```",
    ))
    .body
    .replace("</code>", "");
    assert!(!elsewhere.contains("<time"), "{elsewhere}");
    let highlights = present(&third("", "- a", "- d").replace("Le point. [01:00]", "- Le point. [01:00]"));
    assert!(highlights.body.contains("<li>Le point. [01:00]</li>"), "{}", highlights.body);
    assert!(highlights.markdown.contains("- Le point. [01:00]"), "{}", highlights.markdown);
}

/// The time is the shell's own mark, made of digits: nothing a model writes becomes one.
#[test]
fn a_mark_written_in_the_minutes_stays_text() {
    let minutes = present(&third(
        "",
        "- <time data-second=\"1\" onclick=\"x()\">12:31</time> — b — c",
        "- d <img src=x onerror=y> [00:05]",
    ));
    assert!(!minutes.body.contains("<time data-second=\"1\""), "{}", minutes.body);
    assert!(!minutes.body.contains("<img"), "{}", minutes.body);
    assert_eq!(minutes.body.matches("<time ").count(), 1, "{}", minutes.body);
    assert!(minutes.body.contains("<time data-second=\"5\">00:05</time>"), "{}", minutes.body);
}

#[test]
fn a_session_listed_says_the_title_of_minutes_v3() {
    let sessions = tempfile::tempdir().unwrap();
    let folder = sessions.path().join("third");
    fs::create_dir(&folder).unwrap();
    fs::write(
        folder.join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-13T15:00:00+02:00"}"#,
    )
    .unwrap();
    fs::write(folder.join("minutes.md"), third("title: \"Un nom\"\n", "- a", "- d")).unwrap();
    let listed = wholeheard_shell::sessions::list(sessions.path(), None);
    assert_eq!(listed[0].title.as_deref(), Some("Un nom"));
}
