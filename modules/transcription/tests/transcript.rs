//! What `transcript/v1` promises of a session's transcript, tried on text written here: no
//! engine, no sound of anyone (rules/security.md, 2).

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
fn a_transcript_opens_on_what_made_it_and_holds_what_was_heard_with_its_times_and_speaker() {
    let folder = tempfile::tempdir().unwrap();
    let mut transcript = Transcript::open(folder.path(), &making()).unwrap();
    transcript
        .heard(Speaker::Others, &[said(0.0, 3.204, "Du coup, on confirme\nla   date ?")], 30.0)
        .unwrap();
    transcript
        .heard(Speaker::Me, &[said(3.4, 5.1, "Première semaine de juin.")], 30.0)
        .unwrap();
    transcript.complete(302.337).unwrap();

    let written = lines(folder.path());
    assert_eq!(
        written[0],
        serde_json::json!({ "type": "header", "contract": "transcript/v1", "engine": "an-engine",
        "model": "a-model", "language": "fr", "context": "Quatre personnes." })
    );
    // One line per segment, its text on one line, its times to the hundredth.
    assert_eq!(
        written[1],
        serde_json::json!({ "type": "segment", "speaker": "others", "start": 0.0, "end": 3.2, "text": "Du coup, on confirme la date ?" })
    );
    assert_eq!(
        written[2],
        serde_json::json!({ "type": "covered", "speaker": "others", "until": 30.0 })
    );
    assert_eq!(written[3]["speaker"], "me");
    assert_eq!(written[5], serde_json::json!({ "type": "complete", "duration": 302.34 }));
    assert_eq!(
        types(folder.path()),
        ["header", "segment", "covered", "segment", "covered", "complete"]
    );
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
