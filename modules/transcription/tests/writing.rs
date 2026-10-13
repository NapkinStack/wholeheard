//! What `transcript/v1` promises of a transcript as it is written, tried on text written here:
//! no engine, no sound of anyone (rules/security.md, 2).

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
    let mut transcript = Transcript::start(folder.path(), &making()).unwrap();
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
fn what_an_engine_gives_that_is_no_segment_or_no_time_is_not_written() {
    let folder = tempfile::tempdir().unwrap();
    let mut transcript = Transcript::start(folder.path(), &making()).unwrap();
    // No text; an end before its start; the same start again.
    transcript
        .heard(
            Speaker::Me,
            &[
                said(0.0, 1.0, "Au début."),
                said(2.0, 3.0, "  \n "),
                said(5.0, 4.0, "À l'envers."),
                said(5.004, 6.0, "Deux fois."),
            ],
            30.0,
        )
        .unwrap();
    let segments: Vec<_> = lines(folder.path()).into_iter().filter(|line| line["type"] == "segment").collect();
    assert_eq!(segments.len(), 2);
    assert_eq!(
        (segments[0]["start"].as_f64(), segments[0]["text"].as_str()),
        (Some(0.0), Some("Au début."))
    );
    assert_eq!((segments[1]["start"].as_f64(), segments[1]["end"].as_f64()), (Some(5.0), Some(5.0)));
    // Given out of order, they are written in the order they were said.
    let call = [said(50.0, 51.0, "Après."), said(40.0, 41.0, "D'abord.")];
    transcript.heard(Speaker::Me, &call, 60.0).unwrap();
    let starts = |folder: &Path| -> Vec<f64> {
        lines(folder)
            .iter()
            .filter(|line| line["type"] == "segment")
            .map(|line| line["start"].as_f64().unwrap())
            .collect()
    };
    assert_eq!(starts(folder.path()), [0.0, 5.0, 40.0, 50.0]);
    // Two starts five hundredths apart are two segments, wherever the hundredths fall.
    let close = [
        said(60.095, 60.12, "Un."),
        said(60.145, 60.9, "Deux."),
        said(60.195, 61.2, "Trois."),
    ];
    transcript.heard(Speaker::Me, &close, 65.0).unwrap();
    assert_eq!(starts(folder.path())[4..].len(), 3);
    // The same start for the other speaker is another segment.
    transcript
        .heard(Speaker::Others, &[said(5.0, 6.0, "En même temps.")], 30.0)
        .unwrap();
    assert_eq!(types(folder.path()).iter().filter(|kind| *kind == "segment").count(), 8);

    // A time that is no number of seconds is refused, and nothing of the call is written.
    let before = fs::read(folder.path().join("transcript.jsonl")).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -100.0, -0.4, 1e300] {
        for (start, end, until) in [(bad, 81.0, 90.0), (80.0, bad, 90.0), (80.0, 81.0, bad)] {
            let call = [said(75.0, 76.0, "Bon."), said(start, end, "Non.")];
            assert!(transcript.heard(Speaker::Me, &call, until).is_err(), "{start} {end} {until}");
        }
        assert!(transcript.complete(bad).is_err(), "{bad}");
    }
    // A segment that starts after the span it was heard in is no segment of that span.
    assert!(transcript.heard(Speaker::Me, &[said(100.0, 101.0, "Plus tard.")], 90.0).is_err());
    assert_eq!(fs::read(folder.path().join("transcript.jsonl")).unwrap(), before);
}

#[test]
fn the_end_is_the_last_line_and_a_transcript_that_is_there_is_not_started_again() {
    let folder = tempfile::tempdir().unwrap();
    let mut transcript = Transcript::start(folder.path(), &making()).unwrap();
    transcript.heard(Speaker::Me, &[said(1.0, 2.0, "Un.")], 30.0).unwrap();
    transcript.complete(30.0).unwrap();
    transcript.complete(30.0).unwrap();
    let before = fs::read(folder.path().join("transcript.jsonl")).unwrap();
    assert!(transcript.heard(Speaker::Me, &[said(31.0, 32.0, "Après la fin.")], 60.0).is_err());
    assert!(Transcript::start(folder.path(), &making()).is_err());
    assert_eq!(fs::read(folder.path().join("transcript.jsonl")).unwrap(), before);
    assert_eq!(types(folder.path()).last().unwrap(), "complete");
}
