//! What `transcript/v1` lets a reader rely on and what `minutes/v1` promises, tried on text
//! written here: no provider, no meeting of anyone (rules/security.md, 2).

use std::fs;

use wholeheard_summary::{Made, Refused, minutes, read_transcript, request, write_minutes};

const HEADER: &str =
    r#"{"type": "header", "contract": "transcript/v1", "engine": "an-engine", "model": "a-model", "language": "fr", "context": null}"#;

fn transcript(lines: &[&str]) -> String {
    std::iter::once(HEADER)
        .chain(lines.iter().copied())
        .map(|line| format!("{line}\n"))
        .collect()
}

const ANSWER: &str = "## Points forts\n\nLa date est fixée au 3 juin.\n\n## Actions\n\n- Réserver la salle — non précisé — non précisé\n\n## Points clés\n\n- Une réponse est attendue pour le 26 avril.\n";

fn at(time: &str) -> chrono::DateTime<chrono::FixedOffset> {
    chrono::DateTime::parse_from_rfc3339(time).unwrap()
}

fn made(context: Option<&'static str>) -> Made<'static> {
    Made {
        at: at("2026-10-13T15:12:40+02:00"),
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
        "in the language the meeting is held in, the three titles included",
        "never give them an action",
        "even when nobody took it and no date was set",
        "A task belongs here, not in the third section",
        "one said as a need",
        "written by a speech recogniser",
        "write it as heard and say it is uncertain",
        "A sentence written twice counts once",
        "who speaks on which side through it",
        "write what was meant",
        "whoever sat beside them",
        "is certain from the rest of the meeting",
        "a name or a figure misheard",
        "A task said as \"I\" names nobody",
        "an owner without doubt only when the meeting names them",
        "with \"probably\", or none",
        "an echo",
    ] {
        assert!(asked.contains(rule), "the request lost the rule: {rule}");
    }
    // The rules come before the meeting: after it, they read as part of what was said.
    let meeting = asked.find("Transcript:\n").unwrap();
    for rule in [
        "written by a speech recogniser",
        "Read the meeting through them",
        "counts once",
        "The user's context",
        "A task said as",
    ] {
        assert!(asked.find(rule).unwrap() < meeting, "after the meeting: {rule}");
    }
    // Nothing in it reads as a title to copy: the model once titled French minutes "Highlights".
    assert!(
        !asked.lines().any(|line| line.starts_with('#')) && !asked.to_lowercase().contains("highlights"),
        "{asked}"
    );
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
            "4 section(s)",
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
        ("", "0 section(s)"),
        ("## A\rx\r## B\ry\r\r```\r## C\r`c`", "a block of code"),
        (
            "### Voici le compte rendu\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n",
            "does not open on its first section",
        ),
        (
            "🎉🎉🎉\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n",
            "does not open on its first section",
        ),
        ("- …\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n", "does not open on its first section"),
        ("> …\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n", "does not open on its first section"),
        (
            "![](logo.png)\n\n## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n",
            "does not open on its first section",
        ),
        ("## A\n\n\u{3164}\n\n## B\n\ny\n\n## C\n\nz\n", "a section is empty"),
        (
            "## A\n\nx\n\n## B\n\n- \u{115f}\u{1160}\u{ffa0}\n\n## C\n\nz\n",
            "a section is empty",
        ),
        ("## \u{3164}\n\nx\n\n## B\n\ny\n\n## C\n\nz\n", "a section has no title"),
        ("---\n\n## A\n\nx\n\n## B\n\nx\n\n## C\n\nx\n", "does not open on its first section"),
        (
            "## A\n\nx\n\n## B\n\n- a\n\n## C\n\n- z\n\n> ## Quatre\n> x\n",
            "a title inside a quotation or a list",
        ),
        (
            "## A\n\nx\n\n## B\n\n- a\n\n## C\n\n- z\n- ## Quatre\n",
            "a title inside a quotation or a list",
        ),
        (
            "## A\n\nx\n\n## B\n\n- a\n\n## C\n\n- z\n\n    ## Quatre\n",
            "a title inside a quotation or a list",
        ),
        ("## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n\nQuatre\n-\n", "4 section(s)"),
        ("## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n\n    du code\n", "a block of code"),
        ("## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n\n<h2>Quatre</h2>\n", "it holds HTML"),
        ("## A\n\nx\n\n## B\n\n<!-- rien -->\n\n## C\n\nz\n", "it holds HTML"),
        ("## A\n\nx\n\n## B\n\n***\n\n## C\n\nz\n", "a section is empty"),
        ("## A\n\nx\n\n## B\n\n\u{200b}\n\n## C\n\nz\n", "a section is empty"),
        ("## A\n\nx\n\n## ****\n\ny\n\n## C\n\nz\n", "a section has no title"),
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
            at: at("2026-10-13T15:12:40.987654321Z"),
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
fn the_time_is_written_to_the_second_with_its_offset() {
    // A time is one by its type; a fraction of a second, or another way to write the offset, is not kept.
    for (given, written) in [
        ("2026-10-13T15:12:40.123456789+02:00", "made: 2026-10-13T15:12:40+02:00"),
        ("2026-02-28T23:59:59-05:45", "made: 2026-02-28T23:59:59-05:45"),
        ("2026-10-13T13:12:40Z", "made: 2026-10-13T13:12:40+00:00"),
    ] {
        let made = Made {
            at: at(given),
            provider: "local",
            model: "m",
            context: None,
        };
        assert_eq!(minutes(ANSWER, &made).unwrap().as_str().lines().nth(2).unwrap(), written);
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

#[test]
fn minutes_a_model_may_reasonably_write_are_accepted() {
    // Each was refused by a reading of lines: a rule after a list, titles written as
    // underlines, a table, a rule between sections, a mark before the first title.
    for answer in [
        "## A\n\nx\n\n## B\n\n- a — b — c\n---\n\n## C\n\n- z\n",
        "Points forts\n------------\n\nx\n\nActions\n-------\n\n- a — b — c\n\nPoints clés\n-----------\n\n- z\n",
        "## A\n\nx\n\n---\n\n## B\n\n1. a\n   - b\n\n## C\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        "\u{feff}## A\n\nx\n\n## B\n\n### Cette semaine\n\n- a\n\n## C\n\n`3 juin`\n",
        "## A\r\n\r\nx\r\n\r\n## B\r\n\r\n- a\r\n\r\n## C\r\n\r\n- z\r\n",
    ] {
        assert!(
            minutes(answer, &made(None)).is_ok(),
            "refused: {answer:?} — {:?}",
            minutes(answer, &made(None))
        );
    }
    // Whatever ended its lines, the file is written with one kind of line end.
    let written = minutes("## A\r\n\r\nx\r\n\r\n## B\r\ry\r\r## C\n\nz\r\n", &made(None)).unwrap();
    assert!(!written.as_str().contains('\r') && written.as_str().ends_with("## A\n\nx\n\n## B\n\ny\n\n## C\n\nz\n"));
}

#[test]
fn a_transcript_holds_its_header_first_and_each_segment_once() {
    let segment = r#"{"type": "segment", "speaker": "me", "start": 1.0, "end": 2.0, "text": "Bonjour."}"#;
    // Said twice — a step run again badly — it is read once; an empty line between two is no line.
    assert_eq!(read_transcript(&transcript(&[segment, "", segment])).unwrap().len(), 1);
    let at_zero = segment.replace("1.0", "0.0").replace("2.0", "1.0");
    let at_minus_zero = at_zero.replace(r#""start": 0.0"#, r#""start": -0.0"#);
    assert_eq!(
        read_transcript(&transcript(&[&at_zero, &at_minus_zero, &at_zero.replace("0.0,", "0e0,")]))
            .unwrap()
            .len(),
        1
    );
    for (text, expected) in [
        (format!("\n{}", transcript(&[segment])), "does not name transcript/v1"),
        (transcript(&[&segment.replace("1.0", "-1.0")]), "speaker me, -1 to 2"),
    ] {
        let Err(Refused(why)) = read_transcript(&text) else {
            panic!("accepted: {text}")
        };
        assert!(why.contains(expected), "{why}");
    }
}

#[test]
fn a_writing_that_fails_at_its_last_step_leaves_no_draft() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("minutes.md")).unwrap();
    assert!(write_minutes(folder.path(), &minutes(ANSWER, &made(None)).unwrap()).is_err());
    assert!(!folder.path().join("minutes.md.part").exists());
}

#[cfg(unix)]
#[test]
fn a_link_at_the_drafts_name_is_never_followed() {
    let folder = tempfile::tempdir().unwrap();
    let elsewhere = folder.path().join("someone-else.txt");
    fs::write(&elsewhere, "theirs\n").unwrap();
    std::os::unix::fs::symlink(&elsewhere, folder.path().join("minutes.md.part")).unwrap();
    let written = minutes(ANSWER, &made(None)).unwrap();
    write_minutes(folder.path(), &written).unwrap();
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "theirs\n");
    assert!(
        !fs::symlink_metadata(folder.path().join("minutes.md"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(folder.path().join("minutes.md")).unwrap(), written.as_str());
    assert!(fs::symlink_metadata(folder.path().join("minutes.md.part")).is_err());
}

#[cfg(unix)]
#[test]
fn a_writing_that_fails_leaves_the_minutes_that_were_there() {
    use std::os::unix::fs::PermissionsExt;
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("minutes.md"), "the minutes that were there\n").unwrap();
    // A folder that cannot be written to: no draft can be made.
    fs::set_permissions(folder.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let failed = write_minutes(folder.path(), &minutes(ANSWER, &made(None)).unwrap());
    fs::set_permissions(folder.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(failed.is_err());
    assert_eq!(
        fs::read_to_string(folder.path().join("minutes.md")).unwrap(),
        "the minutes that were there\n"
    );
    assert!(fs::symlink_metadata(folder.path().join("minutes.md.part")).is_err());
}
