//! The step every engine shares, tried with an engine played here: it "hears" a stretch of
//! sound wherever the samples are not silence. No model, no voice of anyone.

use std::{fs, io, path::Path};

use summit_transcription::{
    Segment,
    step::{Engine, RATE, sound, transcribe},
};

/// Hears each stretch of non-silent samples as one segment, and says how long it was. It
/// can be told to fail at its n-th call, and it keeps what it was asked.
#[derive(Default)]
struct Played {
    /// Hears the whole span as one segment that starts this far in: an unbroken voice.
    unbroken: Option<f64>,
    calls: usize,
    fails_at: Option<usize>,
    asked: Vec<(usize, Option<String>, Option<String>)>,
}

impl Engine for Played {
    fn name(&self) -> (String, String) {
        ("played".into(), "a-model".into())
    }

    fn hear(&mut self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> io::Result<Vec<Segment>> {
        self.calls += 1;
        if self.fails_at == Some(self.calls) {
            return Err(io::Error::other("the engine stopped"));
        }
        self.asked.push((sound.len(), language.map(Into::into), context.map(Into::into)));
        if let Some(start) = self.unbroken {
            let (end, text) = (sound.len() as f64 / RATE as f64, "one voice".into());
            return Ok(vec![Segment { start, end, text }]);
        }
        let (mut segments, mut since) = (Vec::new(), None);
        for (at, sample) in sound.iter().chain(&[0.0]).enumerate() {
            match (since, *sample != 0.0) {
                (None, true) => since = Some(at),
                (Some(start), false) => {
                    let (start, end) = (start as f64 / RATE as f64, at as f64 / RATE as f64);
                    let text = format!("{:.1} s", end - start);
                    segments.push(Segment { start, end, text });
                    since = None;
                }
                _ => {}
            }
        }
        Ok(segments)
    }
}

/// An audio file of a session: `seconds` long, with sound in the given stretches. Its
/// header says "length unknown", as a recording in progress or ended by a crash does.
fn audio(path: &Path, seconds: f64, speech: &[(f64, f64)]) {
    let mut bytes = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff".to_vec();
    for n in 0..(seconds * RATE as f64) as usize {
        let at = n as f64 / RATE as f64;
        let loud = speech.iter().any(|(from, to)| (*from..*to).contains(&at));
        bytes.extend_from_slice(&(if loud { 8000i16 } else { 0 }).to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

/// The record of a recording still going on, or ended by a crash: no "stopped".
const GOING: &str = r#"{"contract":"session-audio/v1","started":"2026-10-13T14:05:12+02:00"}"#;
const STOPPED: &str = r#"{"contract":"session-audio/v1","started":"2026-10-13T14:05:12+02:00","stopped":"2026-10-13T14:06:22+02:00"}"#;

fn session() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    audio(&folder.path().join("me.wav"), 70.0, &[(3.0, 5.0), (28.5, 31.5), (40.0, 41.0)]);
    audio(&folder.path().join("others.wav"), 65.0, &[(0.5, 2.5), (59.5, 62.0)]);
    folder
}

/// (speaker, start, end) of every segment of the transcript, as written.
fn segments(folder: &Path) -> Vec<(String, f64, f64)> {
    let text = fs::read_to_string(folder.join("transcript.jsonl")).unwrap();
    text.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .filter(|line| line["type"] == "segment")
        .map(|line| {
            (
                line["speaker"].as_str().unwrap().into(),
                line["start"].as_f64().unwrap(),
                line["end"].as_f64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_session_is_heard_span_by_span_and_a_sentence_across_two_spans_is_heard_whole() {
    let folder = session();
    fs::write(folder.path().join("context.txt"), "  Quatre personnes.\n").unwrap();
    let mut engine = Played::default();
    transcribe(folder.path(), &mut engine, Some("fr"), 30).unwrap();

    // The stretch from 28.5 to 31.5 crosses the first span's end: it is one segment, not two halves.
    assert_eq!(
        segments(folder.path()),
        [
            ("me".to_string(), 3.0, 5.0),
            ("me".to_string(), 28.5, 31.5),
            ("me".to_string(), 40.0, 41.0),
            ("others".to_string(), 0.5, 2.5),
            ("others".to_string(), 59.5, 62.0),
        ]
    );
    let text = fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(
        lines[0],
        serde_json::json!({ "type": "header", "contract": "transcript/v1", "engine": "played",
        "model": "a-model", "language": "fr", "context": "Quatre personnes." })
    );
    assert_eq!(lines.last().unwrap(), &serde_json::json!({ "type": "complete", "duration": 70.0 }));
    // The third span of `others` holds no sound: the engine was not asked to find words in silence.
    assert_eq!(engine.calls, 5);
    // Every span was heard in the language asked, with the context the user attached, and no span is longer than asked.
    assert!(engine.asked.iter().all(|(length, language, context)| {
        *length <= 30 * RATE && language.as_deref() == Some("fr") && context.as_deref() == Some("Quatre personnes.")
    }));
}

#[test]
fn a_step_that_stops_halfway_is_run_again_and_gives_the_same_transcript_as_one_never_stopped() {
    let whole = session();
    transcribe(whole.path(), &mut Played::default(), None, 30).unwrap();
    let expected = fs::read_to_string(whole.path().join("transcript.jsonl")).unwrap();

    for fails_at in 1..=5 {
        let folder = session();
        let (mut engine, mut again) = (Played::default(), Played::default());
        engine.fails_at = Some(fails_at);
        assert!(transcribe(folder.path(), &mut engine, None, 30).is_err());
        // Run again: it goes on from where it was, and asks nothing of what is covered.
        transcribe(folder.path(), &mut again, None, 30).unwrap();
        let written = fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap();
        assert_eq!(written, expected, "stopped at call {fails_at}");
        assert!(
            again.calls + fails_at - 1 <= 6,
            "stopped at call {fails_at}: {} calls again",
            again.calls
        );
    }
    // Run a third time on a finished transcript: the engine is not asked, the file does not change.
    let mut idle = Played::default();
    transcribe(whole.path(), &mut idle, None, 30).unwrap();
    assert_eq!(
        (idle.calls, fs::read_to_string(whole.path().join("transcript.jsonl")).unwrap()),
        (0, expected)
    );
}

#[test]
fn a_file_that_is_not_what_a_session_holds_is_said_and_not_converted() {
    let folder = session();
    // The header says "length unknown": the sound reads to the end of the file all the same.
    assert_eq!(sound(&folder.path().join("me.wav")).unwrap().len(), 70 * RATE);

    let wav = fs::read(folder.path().join("me.wav")).unwrap();
    let with = |at: usize, value: &[u8]| {
        let mut bytes = wav.clone();
        bytes[at..at + value.len()].copy_from_slice(value);
        bytes
    };
    let refused = |folder: &Path, expected: &[&str]| {
        let mut engine = Played::default();
        let said = transcribe(folder, &mut engine, None, 30)
            .expect_err("heard all the same")
            .to_string();
        assert!(expected.iter().all(|part| said.contains(part)), "{said}");
        assert_eq!(engine.calls, 0, "{said}: the engine heard something first");
        assert!(!folder.join("transcript.jsonl").exists(), "{said}: a transcript was left");
    };
    for (bytes, expected) in [
        (with(22, &2u16.to_le_bytes()), "2 channel(s)"),
        (with(24, &48_000u32.to_le_bytes()), "48000 Hz"),
        (with(34, &32u16.to_le_bytes()), "32 bits"),
        (b"some notes, not sound".to_vec(), "is not an audio file of a session"),
        (wav[..42].to_vec(), "is not an audio file of a session"),
        (Vec::new(), "is not an audio file of a session"),
    ] {
        // Whichever of the two files is wrong, nothing is heard of the other.
        for file in ["me.wav", "others.wav"] {
            let good = fs::read(folder.path().join(file)).unwrap();
            fs::write(folder.path().join(file), &bytes).unwrap();
            refused(folder.path(), &[file, expected]);
            fs::write(folder.path().join(file), good).unwrap();
        }
    }
    fs::remove_file(folder.path().join("others.wav")).unwrap();
    refused(folder.path(), &["others.wav cannot be read"]);
    audio(&folder.path().join("others.wav"), 65.0, &[(0.5, 2.5)]);
    // A folder that is not a session of this version is not heard either.
    for not_one in ["", "{}", r#"{"contract":"session-audio/v2"}"#] {
        fs::write(folder.path().join("session.json"), not_one).unwrap();
        refused(folder.path(), &["session.json does not name session-audio/v1"]);
    }
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    // A context that is not text is said too, before anything is heard.
    fs::write(folder.path().join("context.txt"), [0xff, 0xfe]).unwrap();
    refused(folder.path(), &["context.txt is not text"]);
}

#[test]
fn a_recording_still_going_on_is_heard_up_to_here_and_again_when_there_is_more() {
    let folder = session();
    fs::write(folder.path().join("session.json"), GOING).unwrap();
    // The file ends in the middle of a sentence: that one waits for the rest.
    audio(&folder.path().join("me.wav"), 40.0, &[(3.0, 5.0), (39.0, 40.0)]);
    audio(&folder.path().join("others.wav"), 40.0, &[(0.5, 2.5)]);
    transcribe(folder.path(), &mut Played::default(), None, 30).unwrap();
    let so_far = fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap();
    assert!(!so_far.contains(r#""complete""#) && segments(folder.path()).len() == 2, "{so_far}");

    // The files grew, the recording was stopped: the step goes on, and only then says the end.
    audio(&folder.path().join("me.wav"), 70.0, &[(3.0, 5.0), (39.0, 41.0), (50.0, 52.0)]);
    audio(&folder.path().join("others.wav"), 70.0, &[(0.5, 2.5)]);
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    transcribe(folder.path(), &mut Played::default(), None, 30).unwrap();
    let whole = fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap();
    assert!(
        whole.starts_with(&so_far) && whole.trim_end().ends_with(r#"{"duration":70.0,"type":"complete"}"#),
        "{whole}"
    );
    let written = segments(folder.path());
    assert_eq!(
        written.iter().map(|(_, start, end)| (*start, *end)).collect::<Vec<_>>(),
        [(3.0, 5.0), (0.5, 2.5), (39.0, 41.0), (50.0, 52.0)]
    );
}

#[test]
fn the_step_neither_crawls_on_an_unbroken_voice_nor_cuts_a_stretch_that_starts_with_a_span() {
    // One voice for forty seconds, heard five at a time: whatever the engine says of where
    // it starts, half a span at least every two calls.
    for start in [0.02, 1.0, 2.4, 2.5, 4.5] {
        let folder = session();
        audio(&folder.path().join("me.wav"), 40.0, &[(0.0, 40.0)]);
        audio(&folder.path().join("others.wav"), 40.0, &[]);
        let unbroken = Some(start);
        let mut engine = Played {
            unbroken,
            ..Played::default()
        };
        transcribe(folder.path(), &mut engine, None, 5).unwrap();
        assert!(engine.calls <= 33, "{} calls for eight spans, from {start} s in", engine.calls);
    }

    // A stretch that starts three tenths into a span and reaches its end is still one segment.
    let folder = session();
    audio(&folder.path().join("me.wav"), 70.0, &[(30.3, 60.2)]);
    transcribe(folder.path(), &mut Played::default(), None, 30).unwrap();
    assert_eq!(segments(folder.path())[0], ("me".to_string(), 30.3, 60.2));
}
