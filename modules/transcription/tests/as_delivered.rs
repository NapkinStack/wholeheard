//! The step on a session recorded as `session-audio/v2` describes it: tracks and a clock
//! made here — notes around a rest, no voice of anyone — and an engine played here.

use std::{fs, io, path::Path};

use summit_transcription::{
    Segment,
    step::{Engine, RATE, read, transcribe},
};

/// Hears one segment, from the first sample that moves as a note does to the last.
#[derive(Default)]
struct Played(usize);

impl Engine for Played {
    fn name(&self) -> (String, String) {
        ("played".into(), "a-model".into())
    }

    fn hear(&mut self, sound: &[f32], _: Option<&str>, _: Option<&str>) -> io::Result<Vec<Segment>> {
        self.0 += 1;
        let moves: Vec<bool> = sound.windows(2).map(|pair| (pair[1] - pair[0]).abs() > 0.01).collect();
        let edges = moves.iter().position(|moves| *moves).zip(moves.iter().rposition(|moves| *moves));
        let segment = |(start, end): (usize, usize)| Segment {
            start: start as f64 / RATE as f64,
            end: end as f64 / RATE as f64,
            text: "a note".into(),
        };
        Ok(edges.map(segment).into_iter().collect())
    }
}

/// A track as the capture writes one — 24 bits, the extensible header, lengths "unknown" —
/// resting at `rest`, with a note in the given seconds and the capture's zeros in others.
fn track(path: &Path, rate: u32, channels: u16, seconds: u32, rest: f32, note: (f64, f64), zeros: &[(u32, u32)]) {
    let mut bytes = Vec::from(*b"RIFF\xff\xff\xff\xffWAVEfmt ");
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&0xFFFEu16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 3).to_le_bytes());
    bytes.extend_from_slice(&(channels * 3).to_le_bytes());
    bytes.extend_from_slice(&[24, 0, 22, 0, 24, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
    bytes.extend_from_slice(b"data\xff\xff\xff\xff");
    for frame in 0..rate * seconds {
        let at = f64::from(frame) / f64::from(rate);
        let sound = match (
            zeros.iter().any(|(from, to)| (*from..*to).contains(&frame)),
            (note.0..note.1).contains(&at),
        ) {
            (true, _) => 0.0,
            (_, true) => rest + 0.3 * (std::f64::consts::TAU * 1_000.0 * at).sin() as f32,
            _ => rest,
        };
        let value = (sound * 8_388_608.0).round() as i32;
        (0..channels).for_each(|_| bytes.extend_from_slice(&value.to_le_bytes()[..3]));
    }
    fs::write(path, bytes).unwrap();
}

fn record(folder: &Path, tracks: &str, clock: &str) {
    let record = format!(
        r#"{{"contract":"session-audio/v2","started":"2026-10-13T14:05:12+02:00","stopped":"2026-10-13T14:05:32+02:00","tracks":{tracks}}}"#
    );
    fs::write(folder.join("session.json"), record).unwrap();
    fs::write(folder.join("clock.jsonl"), clock).unwrap();
}

fn lines(folder: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(folder.join("transcript.jsonl")).unwrap();
    text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

const BOTH: &str = r#"{"local":{"device":"a microphone"},"remote":{"device":"a loudspeaker"}}"#;

#[test]
fn a_session_as_delivered_is_heard_on_its_clock_and_its_silences_are_not_handed_to_the_engine() {
    let session = tempfile::tempdir().unwrap();
    let folder = session.path();
    // The microphone: two channels at 48,000, resting at 0.3, a note from 3 s to 5 s.
    track(&folder.join("local.wav"), 48_000, 2, 20, 0.3, (3.0, 5.0), &[]);
    // What was played: one channel at 44,100, resting at -0.2. It opens on half a second of
    // the capture's zeros, its first frame delivered a second into the session; a note a
    // second into the track; the capture's zeros again from 6 s to 8 s.
    track(
        &folder.join("remote.wav"),
        44_100,
        1,
        12,
        -0.2,
        (1.0, 2.0),
        &[(0, 22_050), (264_600, 352_800)],
    );
    let clock = "{\"track\":\"local\",\"sample\":0,\"at\":0.0}\n{\"track\":\"remote\",\"silence\":[0,22050]}\n\
                 {\"track\":\"remote\",\"sample\":22050,\"at\":1.0}\n{\"track\":\"remote\",\"silence\":[264600,352800]}\n";
    record(folder, BOTH, clock);
    assert_eq!(read(folder).unwrap().0, "session-audio/v2");

    let mut engine = Played::default();
    transcribe(folder, &mut engine, None, 10).unwrap();
    let lines = lines(folder);
    let heard: Vec<(&str, f64, f64)> = lines
        .iter()
        .filter(|line| line["type"] == "segment")
        .map(|line| {
            (
                line["speaker"].as_str().unwrap(),
                line["start"].as_f64().unwrap(),
                line["end"].as_f64().unwrap(),
            )
        })
        .collect();
    // Each note where it was heard in the session: the played one half a second after its place in its track.
    let near = |heard: (&str, f64, f64), expected: (&str, f64, f64)| {
        heard.0 == expected.0 && (heard.1 - expected.1).abs() <= 0.02 && (heard.2 - expected.2).abs() <= 0.02
    };
    assert!(
        heard.len() == 2 && near(heard[0], ("me", 3.0, 5.0)) && near(heard[1], ("others", 1.5, 2.5)),
        "{heard:?}"
    );
    assert_eq!(lines.last().unwrap(), &serde_json::json!({ "type": "complete", "duration": 20.0 }));
    // A span where a source only rests, away from zero, is silence — and so is one of the
    // capture's zeros: the engine heard the two spans that hold a note, and no other.
    assert_eq!(engine.0, 2);
}

#[test]
fn a_span_that_only_goes_from_where_a_source_left_to_where_it_came_back_is_silence() {
    let session = tempfile::tempdir().unwrap();
    let folder = session.path();
    // Left at 0.2 after five seconds, delivered nothing for thirty, came back at -0.1 — which
    // the track tells by where it rests: the note is outside it.
    track(&folder.join("remote.wav"), 48_000, 1, 5, 0.2, (9.0, 9.0), &[]);
    let mut bytes = fs::read(folder.join("remote.wav")).unwrap();
    bytes.extend(vec![0u8; 30 * 48_000 * 3]);
    track(&folder.join("local.wav"), 48_000, 1, 5, -0.1, (9.0, 9.0), &[]);
    bytes.extend_from_slice(&fs::read(folder.join("local.wav")).unwrap()[68..]);
    fs::write(folder.join("remote.wav"), bytes).unwrap();
    fs::remove_file(folder.join("local.wav")).unwrap();
    record(
        folder,
        r#"{"remote":{"device":"a loudspeaker"}}"#,
        "{\"track\":\"remote\",\"silence\":[240000,1680000]}\n",
    );

    let mut engine = Played::default();
    transcribe(folder, &mut engine, None, 10).unwrap();
    // Of four spans, the two in the middle of the line are silence; the two that hold where
    // the source left and came back are heard, and no word is found there. A side with no
    // track said nothing.
    assert_eq!(engine.0, 2);
    assert!(lines(folder).iter().all(|line| line["type"] != "segment"));
}

#[test]
fn a_session_whose_tracks_are_not_what_its_record_says_is_refused_with_the_files_name() {
    let session = tempfile::tempdir().unwrap();
    let folder = session.path();
    let refused = |expected: &str| {
        let said = transcribe(folder, &mut Played::default(), None, 10).unwrap_err().to_string();
        assert!(said.contains(expected) && !folder.join("transcript.jsonl").exists(), "{said}");
    };
    record(folder, BOTH, "");
    track(&folder.join("remote.wav"), 48_000, 1, 1, 0.0, (0.0, 0.0), &[]);
    refused("local.wav cannot be read");
    fs::write(folder.join("local.wav"), "not sound").unwrap();
    refused("local.wav is not a track of a session");
    record(folder, "{}", "");
    refused("names no track this version reads");
    record(folder, r#"{"local":"a microphone"}"#, "");
    refused("names no track this version reads");
}
