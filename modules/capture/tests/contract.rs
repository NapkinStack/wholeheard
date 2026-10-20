//! What the program says while it records is what `recording/v1` promises: every event it
//! can say is read against the contract's own schema, and the events are the five the
//! contract names, under those names. No device is opened.

use std::{collections::BTreeSet, fs, path::Path};

use wholeheard_capture::Event;

const SCHEMA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/recording/v1/event.schema.json");

#[test]
fn every_event_the_program_says_is_one_the_contracts_schema_allows_and_there_are_seven() {
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(SCHEMA).unwrap()).unwrap();
    let allows = jsonschema::validator_for(&schema).unwrap();
    let folder = Path::new("sessions/2026-10-14_14-05");
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
        Event::Level { source: 0, level: 0.0 },
        Event::Level { source: 1, level: 0.4251 },
        Event::Level { source: 0, level: 1.0 },
        Event::Device {
            source: 0,
            device: Some("Headset Microphone"),
        },
        Event::Device { source: 1, device: None },
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
    // The seven the contract names, and no other: an event renamed would still be a line the
    // schema allows — one of a name it does not know, which a consumer ignores.
    assert_eq!(
        said,
        ["device", "heard", "level", "silent", "started", "stopped", "unavailable"]
            .map(String::from)
            .into()
    );
}

#[test]
fn a_level_is_the_peak_on_a_meter_s_scale_rounded_to_a_hundredth() {
    assert_eq!(wholeheard_capture::level(0.0), 0.0);
    assert_eq!(wholeheard_capture::level(0.0005), 0.0);
    assert_eq!(wholeheard_capture::level(1.0), 1.0);
    assert!((wholeheard_capture::level(0.1) - 2.0 / 3.0).abs() < 1e-4, "-20 dB is two thirds up");
    assert!((wholeheard_capture::level(0.01) - 1.0 / 3.0).abs() < 1e-4, "-40 dB is a third up");
    assert_eq!(Event::Level { source: 1, level: 0.4251 }.line()["level"], 0.43);
    assert_eq!(Event::Level { source: 0, level: 1.7 }.line()["level"], 1.0);
}

#[test]
fn a_device_changed_is_another_recorded_and_one_gone_is_none_left() {
    use cpal::ErrorKind;
    // D31: what the system says of a stream's device is a move — to another, or to none.
    assert_eq!(wholeheard_capture::moved(ErrorKind::DeviceChanged), Some(true));
    assert_eq!(wholeheard_capture::moved(ErrorKind::DeviceNotAvailable), Some(false));
    // Any other trouble of the stream is no move.
    for other in [ErrorKind::DeviceBusy, ErrorKind::BackendError] {
        assert_eq!(wholeheard_capture::moved(other), None, "{other:?}");
    }
}
