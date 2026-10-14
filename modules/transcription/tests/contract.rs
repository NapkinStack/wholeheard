//! What the step writes is what `transcript/v1` promises: every line it writes is read
//! against the contract's own schema, and what no schema of a line can say — the order of
//! the lines, the kinds it writes — is held here. Text written here, no sound of anyone.

use std::{collections::BTreeSet, fs, path::Path};

use summit_transcription::{Making, Segment, Speaker, Transcript};

const SCHEMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/transcript/v1/line.schema.json");

fn said(start: f64, end: f64, text: &str) -> Segment {
    Segment {
        start,
        end,
        text: text.into(),
    }
}

/// A transcript as the step writes one, stopped before its last line or not.
fn written(folder: &Path, language: Option<&str>, context: Option<&str>, complete: bool) -> Vec<serde_json::Value> {
    let making = Making {
        engine: "an-engine".into(),
        model: "a-model".into(),
        language: language.map(Into::into),
        context: context.map(Into::into),
    };
    let mut transcript = Transcript::open(folder, &making).unwrap();
    transcript
        .heard(Speaker::Me, &[said(3.4, 5.1, "Première semaine de juin, je pense.")], 30.0)
        .unwrap();
    transcript
        .heard(
            Speaker::Others,
            &[
                said(0.0, 3.2, "Du coup j'ai besoin que l'on confirme la date."),
                said(31.0, 31.0, "Oui."),
            ],
            60.0,
        )
        .unwrap();
    transcript.heard(Speaker::Me, &[], 60.0).unwrap();
    if complete {
        transcript.complete(61.27).unwrap();
    }
    let text = fs::read_to_string(folder.join("transcript.jsonl")).unwrap();
    text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

#[test]
fn every_line_the_step_writes_is_one_the_contracts_schema_allows_in_the_order_its_page_says() {
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(SCHEMA).unwrap()).unwrap();
    let allows = jsonschema::validator_for(&schema).unwrap();
    let mut kinds = BTreeSet::new();
    for (number, (language, context, complete)) in [
        (Some("fr"), Some("Quatre personnes.\nDeux absentes."), true),
        (None, None, true),
        (Some("en"), None, false),
    ]
    .into_iter()
    .enumerate()
    {
        let folder = tempfile::tempdir().unwrap();
        let lines = written(folder.path(), language, context, complete);
        for line in &lines {
            assert!(allows.is_valid(line), "transcript {number}: the schema refuses {line}");
            kinds.insert(line["type"].as_str().unwrap().to_string());
        }
        // What a schema of one line cannot say: the header comes first and once, the last
        // line of a finished transcript is `complete` and comes once, and there is speech.
        let types: Vec<&str> = lines.iter().map(|line| line["type"].as_str().unwrap()).collect();
        assert_eq!((types[0], types.iter().filter(|kind| **kind == "header").count()), ("header", 1));
        assert_eq!(types.iter().filter(|kind| **kind == "complete").count(), usize::from(complete));
        assert_eq!(*types.last().unwrap() == "complete", complete);
        assert_eq!(types.iter().filter(|kind| **kind == "segment").count(), 3);
        // Each segment says its track, and keeps the place of the voice, empty in this version.
        for segment in lines.iter().filter(|line| line["type"] == "segment") {
            let track = if segment["speaker"] == "me" { "local" } else { "remote" };
            assert_eq!(
                (&segment["track"], &segment["voice"]),
                (&track.into(), &serde_json::Value::Null),
                "{segment}"
            );
        }
    }
    // The step writes the four kinds the contract names, under those names, and no other: a
    // kind renamed would still be a line the schema allows — one of a type it does not name.
    assert_eq!(kinds, ["complete", "covered", "header", "segment"].map(String::from).into());
}
