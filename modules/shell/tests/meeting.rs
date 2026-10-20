//! One meeting's transcript, read from folders made here.

use std::fs;

use wholeheard_shell::meeting::{self, Read};

#[test]
fn the_transcript_is_read_in_the_meeting_s_order_with_each_segment_s_track_and_voice() {
    let folder = tempfile::tempdir().unwrap();
    let lines = [
        r#"{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"ggml-large-v3-turbo-q8_0","language":"fr","context":null}"#,
        r#"{"type":"segment","speaker":"others","track":"remote","voice":null,"start":8.0,"end":11.5,"text":"Bonjour Claire."}"#,
        r#"{"type":"segment","speaker":"me","track":"local","voice":"1","start":1.0,"end":7.2,"text":"Bonjour à tous."}"#,
        r#"{"type":"covered","speaker":"me","until":30.0}"#,
        // Written before tracks were: its track is its speaker's.
        r#"{"type":"segment","speaker":"me","start":12.0,"end":13.0,"text":"Parfait."}"#,
        "not a line of the contract",
        r#"{"type":"segment","speaker":"me","track":"elsewhere","start":14.0,"end":15.0,"text":"?"}"#,
        r#"{"type":"complete","duration":965.0}"#,
    ];
    fs::write(folder.path().join("transcript.jsonl"), lines.join("\n") + "\n").unwrap();
    let Read::Present(transcript) = meeting::transcript(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(
        (transcript.engine.as_deref(), transcript.model.as_deref()),
        (Some("whisper.cpp"), Some("ggml-large-v3-turbo-q8_0"))
    );
    assert!(transcript.complete);
    assert_eq!(transcript.unread, 2);
    let said: Vec<_> = transcript
        .segments
        .iter()
        .map(|s| (s.start, s.track.as_str(), s.voice.as_deref(), s.text.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (1.0, "local", Some("1"), "Bonjour à tous."),
            (8.0, "remote", None, "Bonjour Claire."),
            (12.0, "local", None, "Parfait.")
        ]
    );
}

#[test]
fn a_transcript_without_its_header_is_unreadable_and_one_being_made_is_not_complete() {
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(meeting::transcript(folder.path()), Read::Absent);
    let segment = r#"{"type":"segment","speaker":"me","track":"local","start":1.0,"end":2.0,"text":"Oui."}"#;
    fs::write(folder.path().join("transcript.jsonl"), format!("{segment}\n")).unwrap();
    assert_eq!(meeting::transcript(folder.path()), Read::Unreadable);
    let header = r#"{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}"#;
    fs::write(folder.path().join("transcript.jsonl"), format!("{header}\n{segment}\n")).unwrap();
    let Read::Present(transcript) = meeting::transcript(folder.path()) else {
        panic!("not read")
    };
    assert!(!transcript.complete);
    assert_eq!(transcript.segments.len(), 1);
}

#[test]
fn transcript_lines_the_contract_does_not_allow_are_left_out_and_counted() {
    let folder = tempfile::tempdir().unwrap();
    let header = r#"{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}"#;
    let lines = [
        header,
        r#"{"type":"segment","speaker":"me","track":"remote","start":1.0,"end":2.0,"text":"a track that is not the speaker's"}"#,
        r#"{"type":"segment","speaker":"someone","track":"local","start":1.0,"end":2.0,"text":"a speaker of no kind"}"#,
        r#"{"type":"segment","speaker":"me","track":"local","start":5.0,"end":4.0,"text":"an end before its start"}"#,
        r#"{"type":"segment","speaker":"me","track":"local","start":5.0,"text":"no end"}"#,
        header,
        r#"{"type":"segment","speaker":"others","track":"remote","start":3.0,"end":4.0,"text":"remote"}"#,
        r#"{"type":"segment","speaker":"me","track":"local","start":3.0,"end":4.0,"text":"local"}"#,
    ];
    // The last line is still being written: no line feed, not read, not counted.
    let text = lines.join("\n") + "\n" + r#"{"type":"segment","speaker":"me","#;
    fs::write(folder.path().join("transcript.jsonl"), text).unwrap();
    let Read::Present(transcript) = meeting::transcript(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(transcript.unread, 5);
    // Two segments at one instant: the local first.
    let said: Vec<_> = transcript.segments.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(said, ["local", "remote"]);

    // A header that is not the first line, or of another version: not a transcript of this one.
    for first in [lines[6].to_string(), header.replace("transcript/v1", "transcript/v2")] {
        fs::write(folder.path().join("transcript.jsonl"), format!("{first}\n{header}\n")).unwrap();
        assert_eq!(meeting::transcript(folder.path()), Read::Unreadable, "{first}");
    }
}

#[test]
fn an_instant_said_at_once_is_kept_and_a_line_missing_what_the_contract_asks_is_not() {
    let folder = tempfile::tempdir().unwrap();
    let header = r#"{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}"#;
    let lines = [
        header,
        // A start equal to its end: a word said at once, as real transcripts hold hundreds.
        r#"{"type":"segment","speaker":"me","track":"local","start":2.0,"end":2.0,"text":"Oui."}"#,
        r#"{"type":"segment","speaker":"someone","start":3.0,"end":4.0,"text":"a speaker of no kind, no track"}"#,
        r#"{"type":"segment","speaker":"me","track":"local","start":5.0,"end":6.0}"#,
    ];
    fs::write(folder.path().join("transcript.jsonl"), lines.join("\n") + "\n").unwrap();
    let Read::Present(transcript) = meeting::transcript(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(transcript.unread, 2);
    assert_eq!(transcript.segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["Oui."]);
    // A first line of the right contract that is not a header is no header.
    let not_a_header = header.replace(r#""type":"header""#, r#""type":"segment""#);
    fs::write(folder.path().join("transcript.jsonl"), format!("{not_a_header}\n")).unwrap();
    assert_eq!(meeting::transcript(folder.path()), Read::Unreadable);
}

#[test]
fn a_meeting_s_shape_is_how_much_each_side_spoke_in_each_part() {
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(
        folder.path().join("transcript.jsonl"),
        concat!(
            r#"{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}"#,
            "\n",
            r#"{"type":"segment","speaker":"me","track":"local","start":0.0,"end":5.0,"text":"a"}"#,
            "\n",
            r#"{"type":"segment","speaker":"others","track":"remote","start":5.0,"end":10.0,"text":"b"}"#,
            "\n",
        ),
    )
    .unwrap();
    let Read::Present(transcript) = meeting::transcript(folder.path()) else {
        panic!("not read")
    };
    let [here, remote] = meeting::shape(&transcript, 10.0, 4);
    assert_eq!(here, [1.0, 1.0, 0.0, 0.0]);
    assert_eq!(remote, [0.0, 0.0, 1.0, 1.0]);
    // Half a part spoken is half; nothing beyond the end; no length, no shape to draw.
    let [here, _] = meeting::shape(&transcript, 20.0, 4);
    assert_eq!(here, [1.0, 0.0, 0.0, 0.0]);
    let [here, _] = meeting::shape(&transcript, 40.0, 4);
    assert_eq!(here, [0.5, 0.0, 0.0, 0.0]);
    assert_eq!(meeting::shape(&transcript, 0.0, 4), [vec![0.0; 4], vec![0.0; 4]]);
    // Two lines of one side over the same part count once, never more than the whole part; a line
    // that starts within a part counts from there.
    std::fs::write(
        folder.path().join("transcript.jsonl"),
        concat!(
            r#"{"type":"header","contract":"transcript/v1","engine":"e","model":"m","language":null,"context":null}"#,
            "\n",
            r#"{"type":"segment","speaker":"me","track":"local","start":0.0,"end":2.5,"text":"a"}"#,
            "\n",
            r#"{"type":"segment","speaker":"me","track":"local","start":0.0,"end":2.5,"text":"a again"}"#,
            "\n",
            r#"{"type":"segment","speaker":"others","track":"remote","start":3.75,"end":5.0,"text":"b"}"#,
            "\n",
        ),
    )
    .unwrap();
    let Read::Present(transcript) = meeting::transcript(folder.path()) else {
        panic!("not read")
    };
    let [here, remote] = meeting::shape(&transcript, 10.0, 4);
    assert_eq!(here, [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(remote, [0.0, 0.5, 0.0, 0.0]);
}
