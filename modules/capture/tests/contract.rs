//! What the program says while it records is what `recording/v1` promises: every event it
//! can say is read against the contract's own schema, and the events are the five the
//! contract names, under those names. No device is opened.

use std::{collections::BTreeSet, fs, path::Path};

use summit_capture::Event;

const SCHEMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/recording/v1/event.schema.json");

#[test]
fn every_event_the_program_says_is_one_the_contracts_schema_allows_and_there_are_five() {
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(SCHEMA).unwrap()).unwrap();
    let allows = jsonschema::validator_for(&schema).unwrap();
    let folder = Path::new("/home/someone/Summit/2026-10-14_14-05");
    let events = [
        Event::Unavailable {
            source: 0,
            reason: "the system names no default device",
        },
        Event::Unavailable {
            source: 1,
            reason: "the system names no default output",
        },
        Event::Started {
            folder,
            sources: ["Digital Microphone", "Speaker"],
        },
        Event::Started {
            folder,
            sources: ["unavailable", "Speaker"],
        },
        Event::Silent { source: 0 },
        Event::Heard { source: 1 },
        Event::Stopped { folder, seconds: 0.0 },
        Event::Stopped { folder, seconds: 1912.4 },
    ];
    let mut said = BTreeSet::new();
    for event in &events {
        let line = event.line();
        assert!(allows.is_valid(&line), "the schema refuses {line}");
        // One line of the output: an event never spreads over two.
        assert!(!line.to_string().contains('\n'), "{line}");
        said.insert(line["event"].as_str().unwrap().to_string());
    }
    // The five the contract names, and no other: an event renamed would still be a line the
    // schema allows — one of a name it does not know, which a consumer ignores.
    assert_eq!(
        said,
        ["heard", "silent", "started", "stopped", "unavailable"].map(String::from).into()
    );
}
