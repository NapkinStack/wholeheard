//! A transcript taken up again after the step that wrote it stopped — cleanly, or killed:
//! tried on text written here, no engine, no sound of anyone (rules/security.md, 2).

use std::{fs, path::Path};

use summit_transcription::{Making, Segment, Speaker, Transcript};

fn making() -> Making {
    Making {
        engine: "an-engine".into(),
        model: "a-model".into(),
        language: Some("fr".into()),
        context: Some("Quatre personnes.".into()),
    }
}

fn said(start: f64, end: f64, text: &str) -> Segment {
    Segment {
        start,
        end,
        text: text.into(),
    }
}

/// The file as any reader finds it: every line whole, each a JSON object with its type.
fn lines(folder: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(folder.join("transcript.jsonl")).unwrap();
    assert!(text.is_empty() || text.ends_with('\n'), "the file ends on a line being written");
    text.lines()
        .map(|line| serde_json::from_str(line).expect("a line that is not JSON"))
        .collect()
}

fn types(folder: &Path) -> Vec<String> {
    lines(folder)
        .iter()
        .map(|line| line["type"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_step_stopped_and_run_again_takes_up_where_it_was_and_hears_nothing_twice() {
    let folder = tempfile::tempdir().unwrap();
    let first = [said(1.0, 2.0, "Un."), said(12.0, 13.0, "Deux.")];
    let second = [said(31.0, 32.0, "Trois.")];
    {
        let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
        transcript.heard(Speaker::Me, &first, 30.0).unwrap();
    }
    // Run again: it says where to take up, and hearing the first span once more adds nothing.
    let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
    assert_eq!((transcript.covered(Speaker::Me), transcript.covered(Speaker::Others)), (30.0, 0.0));
    let before = fs::read(folder.path().join("transcript.jsonl")).unwrap();
    transcript.heard(Speaker::Me, &first, 30.0).unwrap();
    assert_eq!(
        fs::read(folder.path().join("transcript.jsonl")).unwrap(),
        before,
        "a span heard twice changed the file"
    );
    transcript.heard(Speaker::Me, &second, 60.0).unwrap();
    let after = fs::read(folder.path().join("transcript.jsonl")).unwrap();
    assert!(
        after.len() > before.len() && after.starts_with(&before),
        "a line already written was changed"
    );
    transcript.complete(60.0).unwrap();
    transcript.complete(60.0).unwrap();
    assert_eq!(
        types(folder.path()),
        ["header", "segment", "segment", "covered", "segment", "covered", "complete"]
    );
    assert!(Transcript::open(folder.path(), &making()).unwrap().is_complete());
}

#[test]
fn a_step_killed_while_writing_leaves_a_transcript_that_is_continued_without_a_scar() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("transcript.jsonl");
    {
        let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
        transcript.heard(Speaker::Me, &[said(1.0, 2.0, "Un.")], 30.0).unwrap();
    }
    // Killed in the middle of a line: what a reader ignores, and what must not be glued to the next line.
    let whole = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        format!(
            "{whole}{}",
            r#"{"type": "segment", "speaker": "me", "start": 31.0, "end": 32.0, "text": "Tro"#
        ),
    )
    .unwrap();
    let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), whole, "the line being written was kept");
    transcript.heard(Speaker::Me, &[said(31.0, 32.0, "Trois.")], 60.0).unwrap();
    assert_eq!(types(folder.path()), ["header", "segment", "covered", "segment", "covered"]);

    // Killed after a span's segments and before the line that says the span is done: run
    // again, the span's segments are not there twice.
    let done = fs::read_to_string(&path).unwrap();
    let without_the_last_covered = done.trim_end().rsplit_once('\n').unwrap().0.to_string() + "\n";
    fs::write(&path, &without_the_last_covered).unwrap();
    let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
    assert_eq!(transcript.covered(Speaker::Me), 30.0);
    transcript
        .heard(Speaker::Me, &[said(31.0, 32.0, "Trois."), said(40.0, 41.0, "Quatre.")], 60.0)
        .unwrap();
    let starts: Vec<f64> = lines(folder.path())
        .iter()
        .filter(|line| line["type"] == "segment")
        .map(|line| line["start"].as_f64().unwrap())
        .collect();
    assert_eq!(starts, [1.0, 31.0, 40.0]);
}

#[test]
fn a_transcript_of_another_making_is_never_continued() {
    let folder = tempfile::tempdir().unwrap();
    Transcript::open(folder.path(), &making())
        .unwrap()
        .heard(Speaker::Me, &[said(1.0, 2.0, "Un.")], 30.0)
        .unwrap();
    let before = fs::read(folder.path().join("transcript.jsonl")).unwrap();
    for other in [
        Making {
            model: "another-model".into(),
            ..making()
        },
        Making {
            engine: "another-engine".into(),
            ..making()
        },
        Making { context: None, ..making() },
        Making {
            language: None,
            ..making()
        },
    ] {
        let refused = Transcript::open(folder.path(), &other)
            .err()
            .expect("continued under another making");
        assert!(refused.to_string().contains("make it again"), "{refused}");
    }
    fs::write(folder.path().join("other.jsonl"), "not a transcript\n").unwrap();
    assert_eq!(
        fs::read(folder.path().join("transcript.jsonl")).unwrap(),
        before,
        "a refusal changed the file"
    );

    // A file that is no transcript is refused too, and left as it is.
    let elsewhere = tempfile::tempdir().unwrap();
    fs::write(elsewhere.path().join("transcript.jsonl"), "some notes\n").unwrap();
    assert!(Transcript::open(elsewhere.path(), &making()).is_err());
    assert_eq!(
        fs::read_to_string(elsewhere.path().join("transcript.jsonl")).unwrap(),
        "some notes\n"
    );
}

#[test]
fn a_file_that_is_refused_is_left_exactly_as_it_is() {
    // Each of these was changed by an opening that refused it, or taken for an empty transcript.
    let other = Making {
        engine: "another-engine".into(),
        ..making()
    };
    let torn = r#"{"type": "segment", "speaker": "me", "sta"#;
    let theirs = format!(
        "{}\n{torn}",
        serde_json::json!({ "type": "header", "contract": "transcript/v1", "engine": "another-engine", "model": "a-model", "language": "fr", "context": "Quatre personnes." })
    );
    let cases: [(&str, Vec<u8>, &Making); 6] = [
        ("another making, with a line being written", theirs.clone().into_bytes(), &making()),
        ("notes, their last line unfinished", b"my notes\nline two".to_vec(), &making()),
        ("one word and no line feed", b"hello".to_vec(), &making()),
        ("not text", vec![0x52, 0x49, 0xff, 0xfe, b'\n', 0x00, 0xff, b'\n'], &making()),
        ("not text, no line feed", vec![0xff; 512], &making()),
        (
            "a later version of the contract",
            theirs.replace("transcript/v1", "transcript/v2").into_bytes(),
            &other,
        ),
    ];
    for (what, bytes, making) in cases {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("transcript.jsonl"), &bytes).unwrap();
        assert!(Transcript::open(folder.path(), making).is_err(), "{what}: opened");
        assert_eq!(fs::read(folder.path().join("transcript.jsonl")).unwrap(), bytes, "{what}: changed");
    }
}

#[test]
fn a_transcript_is_taken_up_whatever_a_kill_left_and_refused_when_a_line_is_not_its_own() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("transcript.jsonl");
    // Killed while the header was being written: there is no whole line yet, and it is written again.
    let header = serde_json::json!({ "type": "header", "contract": "transcript/v1", "engine": "an-engine", "model": "a-model", "language": "fr", "context": "Quatre personnes." }).to_string();
    // ... or nothing at all: the file was created, and the step died there.
    fs::write(&path, "").unwrap();
    drop(Transcript::open(folder.path(), &making()).unwrap());
    assert_eq!(types(folder.path()), ["header"]);
    fs::write(&path, &header[..40]).unwrap();
    Transcript::open(folder.path(), &making())
        .unwrap()
        .heard(Speaker::Me, &[said(0.37, 2.0, "Un."), said(2.87, 4.0, "Deux.")], 30.0)
        .unwrap();
    assert_eq!(types(folder.path()), ["header", "segment", "segment", "covered"]);

    // Killed before the line that says the span is done, then the span heard again: the two
    // segments that were written are not written a second time, the third is.
    let whole = fs::read_to_string(&path).unwrap();
    fs::write(&path, whole.trim_end().rsplit_once('\n').unwrap().0.to_string() + "\n").unwrap();
    let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
    transcript
        .heard(
            Speaker::Me,
            &[said(0.37, 2.0, "Un."), said(2.87, 4.0, "Deux."), said(9.0, 10.0, "Trois.")],
            30.0,
        )
        .unwrap();
    let starts: Vec<f64> = lines(folder.path())
        .iter()
        .filter(|line| line["type"] == "segment")
        .map(|line| line["start"].as_f64().unwrap())
        .collect();
    assert_eq!(starts, [0.37, 2.87, 9.0]);

    // A header with a field of a later version is this making all the same: a reader skips what it does not know.
    let known = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        known.replacen(r#""type":"header""#, r#""made":"2026-10-13","type":"header""#, 1),
    )
    .unwrap();
    assert_eq!(Transcript::open(folder.path(), &making()).unwrap().covered(Speaker::Me), 30.0);

    // A whole line that is not the contract's — a third speaker, an end before its start, a
    // second header, something that is not JSON — and the transcript is not continued.
    for line in [
        r#"{"type":"segment","speaker":"them","start":40.0,"end":41.0,"text":"x"}"#,
        r#"{"type":"segment","speaker":"me","start":41.0,"end":40.0,"text":"x"}"#,
        &header,
        "not json",
    ] {
        let elsewhere = tempfile::tempdir().unwrap();
        let bytes = format!("{known}{line}\n");
        fs::write(elsewhere.path().join("transcript.jsonl"), &bytes).unwrap();
        let refused = Transcript::open(elsewhere.path(), &making()).err().expect("continued").to_string();
        assert!(refused.contains("make it again"), "{line}: {refused}");
        assert_eq!(fs::read_to_string(elsewhere.path().join("transcript.jsonl")).unwrap(), bytes);
    }
}
