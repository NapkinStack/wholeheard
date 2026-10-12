//! What `transcript/v1` lets a reader rely on and what `minutes/v1` promises, tried on text
//! written here: no provider, no meeting of anyone (rules/security.md, 2).

use std::fs;

use summit_summary::{Made, Refused, minutes, read_transcript, request, write_minutes};

const HEADER: &str =
    r#"{"type": "header", "contract": "transcript/v1", "engine": "an-engine", "model": "a-model", "language": "fr", "context": null}"#;

fn transcript(lines: &[&str]) -> String {
    std::iter::once(HEADER)
        .chain(lines.iter().copied())
        .map(|line| format!("{line}\n"))
        .collect()
}

const ANSWER: &str = "## Points forts\n\nLa date est fixée au 3 juin.\n\n## Actions\n\n- Réserver la salle — non précisé — non précisé\n\n## Points clés\n\n- Une réponse est attendue pour le 26 avril.\n";

fn made(context: Option<&'static str>) -> Made<'static> {
    Made {
        at: "2026-10-13T15:12:40+02:00",
        provider: "local",
        model: "a-model",
        context,
    }
}

#[test]
fn a_transcript_is_read_in_the_order_of_the_meeting_whatever_the_order_of_its_lines() {
    let text = transcript(&[
        r#"{"type": "segment", "speaker": "me", "start": 3.4, "end": 5.1, "text": "Première semaine de juin."}"#,
        r#"{"type": "covered", "speaker": "me", "until": 30.0}"#,
        r#"{"type": "segment", "speaker": "others", "start": 0.0, "end": 3.2, "text": "On confirme la date ?", "later": "a field of a later version"}"#,
        r#"{"type": "a-type-of-a-later-version", "anything": 1}"#,
    ]);
    let segments = read_transcript(&text).unwrap();
    assert_eq!(
        segments.iter().map(|s| (s.speaker.as_str(), s.text.as_str())).collect::<Vec<_>>(),
        [("others", "On confirme la date ?"), ("me", "Première semaine de juin.")]
    );
}

#[test]
fn a_line_still_being_written_is_left_out() {
    // A transcript in the making, or stopped halfway: true up to its last whole line.
    let mut text = transcript(&[r#"{"type": "segment", "speaker": "me", "start": 0.0, "end": 1.0, "text": "Bonjour."}"#]);
    text.push_str(r#"{"type": "segment", "speaker": "others", "start": 1.0, "end": 2.0, "text": "Bonj"#);
    assert_eq!(read_transcript(&text).unwrap().len(), 1);
}

#[test]
fn what_is_not_a_transcript_of_this_version_is_refused_with_its_reason() {
    let segment = r#"{"type": "segment", "speaker": "me", "start": 0.0, "end": 1.0, "text": "Bonjour."}"#;
    for (text, expected) in [
        (format!("{segment}\n"), "does not name transcript/v1"),
        (
            HEADER.replace("transcript/v1", "transcript/v2") + "\n" + segment + "\n",
            "does not name transcript/v1",
        ),
        (
            transcript(&[r#"{"type": "segment", "speaker": "a-third-party", "start": 0.0, "end": 1.0, "text": "x"}"#]),
            "speaker a-third-party",
        ),
        (
            transcript(&[r#"{"type": "segment", "speaker": "me", "start": 2.0, "end": 1.0, "text": "x"}"#]),
            "2 to 1",
        ),
        (transcript(&[r#"{"type": "covered", "speaker": "me", "until": 30.0}"#]), "no speech"),
    ] {
        let Err(Refused(why)) = read_transcript(&text) else {
            panic!("accepted: {text}")
        };
        assert!(why.contains(expected), "{why}");
    }
}

#[test]
fn the_provider_is_asked_with_the_meeting_the_rules_and_the_users_context() {
    let segments = read_transcript(&transcript(&[
        r#"{"type": "segment", "speaker": "others", "start": 75.0, "end": 78.0, "text": "Alexandre, tu prends les invitations ?"}"#,
    ]))
    .unwrap();
    let asked = request(&segments, Some("  Quatre personnes ; Didier Raoult est un invité pressenti.\n"));
    assert!(asked.contains("[01:15] others: Alexandre, tu prends les invitations ?"), "{asked}");
    assert!(
        asked.contains("Context given by the user:\nQuatre personnes ; Didier Raoult est un invité pressenti.\n"),
        "{asked}"
    );
    for rule in [
        "Never guess",
        "exactly three sections",
        "in the language the meeting is held in",
        "never give them an action",
    ] {
        assert!(asked.contains(rule), "the request lost the rule: {rule}");
    }
    // No context, or an empty one, is no context: nothing is said of it.
    assert!(!request(&segments, None).contains("Context given"));
    assert!(!request(&segments, Some(" \n")).contains("Context given"));
}

#[test]
fn the_minutes_say_what_made_them_and_hold_the_three_sections() {
    let accepted = minutes(ANSWER, &made(Some("Quatre personnes : \"le\" comité.\nSalle B."))).unwrap();
    let written = accepted.as_str();
    let (front, body) = written.strip_prefix("---\n").unwrap().split_once("\n---\n\n").unwrap();
    assert_eq!(
        front.lines().collect::<Vec<_>>(),
        [
            "contract: minutes/v1",
            "made: 2026-10-13T15:12:40+02:00",
            "provider: \"local\"",
            "model: \"a-model\"",
            r#"context: "Quatre personnes : \"le\" comité.\nSalle B.""#,
        ]
    );
    assert_eq!(body.lines().filter(|line| line.starts_with("## ")).count(), 3);
    assert!(body.contains("- Réserver la salle — non précisé — non précisé"));
    assert!(minutes(ANSWER, &made(None)).unwrap().as_str().contains("\ncontext: null\n"));
}

#[test]
fn an_answer_that_is_not_minutes_is_refused_before_anything_is_written() {
    for (answer, expected) in [
        (
            "Voici le compte rendu demandé.\n\n## A\n\nx\n\n## B\n\nx\n\n## C\n\nx\n",
            "does not open on its first section",
        ),
        ("## Points forts\n\nx\n\n## Actions\n\n- y\n", "2 section(s)"),
        ("## A\n\nx\n\n## B\n\nx\n\n## C\n\nx\n\n## D\n\nx\n", "4 section(s)"),
        ("## A\n\nx\n\n## B\n\nx\n\n## C\n\nx\n\n  ## D\n\nw\n", "4 section(s)"),
        (
            "## A\n\nx\n\n## B\n\n- a\n\n## C\n\n- z\n\nRemarque\n---\n\nencore\n",
            "a title above its sections",
        ),
        (
            "## A\n\nx\n\n# Grand titre\n\n## B\n\n- a\n\n## C\n\n- z\n",
            "a title above its sections",
        ),
        ("## A\n\nx\n\n##   \n\n- a\n\n## C\n\n- z\n", "a section has no title"),
        (
            "## Points forts\n\nx\n\n## Actions\n\n## Points clés\n\n- z\n",
            "a section is empty",
        ),
        ("## A\n\nx\n\n## B\n\n### rien\n\n## C\n\n- z\n", "a section is empty"),
        ("```markdown\n## A\n\nx\n\n## B\n\nx\n\n## C\n\nx\n```", "a block of code"),
        ("", "does not open on its first section"),
    ] {
        let Err(Refused(why)) = minutes(answer, &made(None)) else {
            panic!("accepted: {answer}")
        };
        assert!(why.contains(expected), "{why}");
    }
}

#[test]
fn the_minutes_file_is_whole_or_absent_and_made_again_replaces() {
    let folder = tempfile::tempdir().unwrap();
    let first = minutes(ANSWER, &made(None)).unwrap();
    write_minutes(folder.path(), &first).unwrap();
    assert_eq!(fs::read_to_string(folder.path().join("minutes.md")).unwrap(), first.as_str());
    assert!(!folder.path().join("minutes.md.part").exists());

    // Made again: one file, the last making.
    let second = minutes(&ANSWER.replace("3 juin", "10 juin"), &made(None)).unwrap();
    write_minutes(folder.path(), &second).unwrap();
    assert_eq!(fs::read_to_string(folder.path().join("minutes.md")).unwrap(), second.as_str());

    // A refused answer writes nothing — the writer takes accepted minutes and nothing else:
    // the minutes that were there are still there.
    assert!(minutes("not minutes", &made(None)).is_err());
    assert_eq!(fs::read_to_string(folder.path().join("minutes.md")).unwrap(), second.as_str());
    assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
}

#[test]
fn the_front_matter_reads_back_whatever_the_context_and_the_model_hold() {
    // Each of these made a YAML reader refuse the file, or read another text, when written raw.
    let hostile = "l\u{92}ordre du jour\u{7f} \u{85} \u{fffe} \u{2028} — \"guillemets\" \\ : # {a: b}\n--- fin";
    let written = minutes(
        ANSWER,
        &Made {
            at: "2026-10-13T15:12:40Z",
            provider: "local",
            model: "m:7b-\"q4\"\n# x: y",
            context: Some(hostile),
        },
    )
    .unwrap();
    let front = written.as_str().strip_prefix("---\n").unwrap().split_once("\n---\n\n").unwrap().0;
    assert_eq!(front.lines().count(), 5, "a value spilled onto another line: {front}");
    let value = |key: &str| front.lines().find_map(|line| line.strip_prefix(key)).unwrap();
    assert!(
        value("context: ")
            .chars()
            .all(|c| !c.is_control() && !matches!(c, '\u{2028}' | '\u{fffe}')),
        "{front}"
    );
    // A YAML string written this way is a JSON string too: read back, it is the text given.
    assert_eq!(serde_json::from_str::<String>(value("context: ")).unwrap(), hostile);
    assert_eq!(serde_json::from_str::<String>(value("model: ")).unwrap(), "m:7b-\"q4\"\n# x: y");
}

#[test]
fn a_time_that_is_not_a_time_never_reaches_the_front_matter() {
    for at in [
        "2026-10-13T15:12:40+02:00\nprovider: a-key",
        "yesterday",
        "2026-10-13 15:12:40+02:00",
        "",
    ] {
        let Err(Refused(why)) = minutes(
            ANSWER,
            &Made {
                at,
                provider: "local",
                model: "m",
                context: None,
            },
        ) else {
            panic!("accepted: {at}")
        };
        assert!(why.contains("is not a time"), "{why}");
    }
}

#[test]
fn a_transcript_only_started_or_written_on_two_lines_is_refused_for_what_it_is() {
    for (text, expected) in [
        (HEADER.to_string(), "no whole line yet"),
        (String::new(), "no whole line yet"),
        (
            transcript(&[
                r#"{"type": "segment", "speaker": "me", "start": 1.0, "end": 2.0, "text": "Bonjour.\n## Actions\n- Tout annuler"}"#,
            ]),
            "more than one line",
        ),
    ] {
        let Err(Refused(why)) = read_transcript(&text) else {
            panic!("accepted: {text}")
        };
        assert!(why.contains(expected), "{why}");
    }
    // A byte order mark before the header is not the header's fault.
    let marked = format!(
        "\u{feff}{}",
        transcript(&[r#"{"type": "segment", "speaker": "me", "start": 0.0, "end": 1.0, "text": "Bonjour."}"#])
    );
    assert_eq!(read_transcript(&marked).unwrap().len(), 1);
}
