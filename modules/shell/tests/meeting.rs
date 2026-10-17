//! One meeting's transcript and minutes, read from folders made here.

use std::fs;

use summit_shell::meeting::{self, Action, Read};

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

#[test]
fn the_minutes_are_read_by_the_place_of_their_sections() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("minutes.md"), MINUTES).unwrap();
    let Read::Present(minutes) = meeting::minutes(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(minutes.titles, ["Points forts", "Actions", "Points clés"].map(String::from));
    assert_eq!(
        minutes.highlights,
        "La date de l'événement est fixée au jeudi 3 juin, avec le 10 en seconde date."
    );
    assert_eq!(
        minutes.actions,
        [
            Action {
                what: "Contacter les trois intervenants et le comité scientifique".into(),
                who: "Alexandre".into(),
                when: "26 avril".into()
            },
            Action {
                what: "Pré-réserver les salles".into(),
                who: "non précisé".into(),
                when: "non précisé".into()
            },
        ]
    );
    assert_eq!(
        minutes.key_points,
        ["Réponse des intervenants attendue pour le 26 avril ; un plan B reste à trouver."]
    );
    assert_eq!(
        (minutes.provider.as_deref(), minutes.model.as_deref()),
        (Some("local"), Some("mistral"))
    );
    assert_eq!(minutes.made.as_deref(), Some("2026-10-13T15:12:40+02:00"));
}

#[test]
fn minutes_that_are_not_of_this_version_are_said_unreadable_and_none_are_absent() {
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(meeting::minutes(folder.path()), Read::Absent);
    for wrong in [
        MINUTES.replacen("---\ncontract", "contract", 1),
        MINUTES.replace("minutes/v1", "minutes/v2"),
        MINUTES.replace("## Points clés", "Points clés"),
        format!("{MINUTES}\n## Encore\n"),
    ] {
        fs::write(folder.path().join("minutes.md"), &wrong).unwrap();
        assert_eq!(meeting::minutes(folder.path()), Read::Unreadable, "{wrong}");
    }
}

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
fn the_minutes_are_read_as_summit_summary_writes_them_and_as_commonmark_reads_them() {
    let folder = tempfile::tempdir().unwrap();
    // The front matter as the producer writes it: quoted, escaped.
    let written = "---
contract: minutes/v1
made: 2026-10-13T15:12:40+02:00
provider: \"api.example.org\"
model: \"mod\\\"el\\nname\"
context: null
---

Faits marquants
---------------

La date est fixée,
en juin.

### Un sous-titre

---

  ## Actions ##

* Réserver — la salle — Alexandre — 26 avril
* Prévoir
  le café — Lou — demain
  - un détail, dans l'item

## Points clés

| Sujet | État |
| --- | --- |
| Budget | ouvert |
";
    fs::write(folder.path().join("minutes.md"), written).unwrap();
    let Read::Present(minutes) = meeting::minutes(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(minutes.provider.as_deref(), Some("api.example.org"));
    assert_eq!(minutes.model.as_deref(), Some("mod\"el\nname"));
    assert_eq!(minutes.titles, ["Faits marquants", "Actions", "Points clés"].map(String::from));
    // The sub-title and the rule are not the highlights' sentences.
    assert_eq!(minutes.highlights, "La date est fixée, en juin.");
    assert_eq!(
        minutes.actions,
        [
            Action {
                what: "Réserver — la salle".into(),
                who: "Alexandre".into(),
                when: "26 avril".into()
            },
            Action {
                what: "Prévoir le café".into(),
                who: "Lou".into(),
                when: "demain".into()
            },
        ]
    );
    // A key point not written as a list is kept as written.
    assert_eq!(minutes.key_points.len(), 1);
    assert!(minutes.key_points[0].contains("Budget"));

    for ordered in ["1. Faire — Lou — lundi\n", "+ Faire — Lou — lundi\n"] {
        fs::write(folder.path().join("minutes.md"), MINUTES.replace("- Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril\n- Pré-réserver les salles — non précisé — non précisé\n", ordered)).unwrap();
        let Read::Present(minutes) = meeting::minutes(folder.path()) else {
            panic!("not read: {ordered}")
        };
        assert_eq!(
            minutes.actions,
            [Action {
                what: "Faire".into(),
                who: "Lou".into(),
                when: "lundi".into()
            }],
            "{ordered}"
        );
    }
    // A value written `null`, quoted or not, is no value.
    fs::write(
        folder.path().join("minutes.md"),
        MINUTES.replace("model: mistral", "model: \"null\""),
    )
    .unwrap();
    let Read::Present(minutes) = meeting::minutes(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(minutes.model, None);
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
