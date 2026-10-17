//! The step every engine shares, tried with an engine played here: it "hears" a stretch of
//! sound wherever the samples are not silence. No model, no voice of anyone.

use std::{fs, io, path::Path};

use summit_transcription::{
    Segment,
    step::{Engine, RATE, read, sound, transcribe},
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
    /// Writes these too, wherever they fall — on silence, as Whisper does.
    invents: Vec<(f64, f64, &'static str)>,
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
        for (start, end, text) in &self.invents {
            segments.push(Segment {
                start: *start,
                end: *end,
                text: (*text).into(),
            });
        }
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
    audio_at(path, seconds, speech, 8000);
}

/// The same, its voice at this amplitude.
fn audio_at(path: &Path, seconds: f64, speech: &[(f64, f64)], loudness: i16) {
    let stretches: Vec<(f64, f64, i16)> = speech.iter().map(|(from, to)| (*from, *to, loudness)).collect();
    audio_of(path, seconds, &stretches);
}

/// The same, each stretch at its own amplitude.
fn audio_of(path: &Path, seconds: f64, stretches: &[(f64, f64, i16)]) {
    let mut bytes = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff".to_vec();
    for n in 0..(seconds * RATE as f64) as usize {
        let at = n as f64 / RATE as f64;
        let loudness = stretches
            .iter()
            .find(|(from, to, _)| (*from..*to).contains(&at))
            .map_or(0, |(.., loudness)| *loudness);
        bytes.extend_from_slice(&loudness.to_le_bytes());
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
    assert_eq!(read(folder.path()).unwrap().0, "session-audio/v1");
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
fn a_first_form_session_is_measured_from_zero_as_it_always_was() {
    // A level that never moves, away from zero: sound in the first form, which is centred.
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    audio(&folder.path().join("me.wav"), 20.0, &[(0.0, 20.0)]);
    audio(&folder.path().join("others.wav"), 20.0, &[]);
    let mut engine = Played {
        unbroken: Some(0.0),
        ..Played::default()
    };
    transcribe(folder.path(), &mut engine, None, 10).unwrap();
    assert_eq!(engine.calls, 2);
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
    for not_one in ["", "{}", r#"{"contract":"session-audio/v0"}"#] {
        fs::write(folder.path().join("session.json"), not_one).unwrap();
        refused(folder.path(), &["session.json names neither session-audio/v1 nor session-audio/v2"]);
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
fn the_step_does_not_crawl_whatever_an_engine_says_of_where_a_voice_starts() {
    // One voice for forty seconds, heard five at a time: half a span at least at every call.
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
        assert!(engine.calls <= 16, "{} calls for eight spans, from {start} s in", engine.calls);
    }
}

#[test]
fn the_length_the_last_line_says_is_the_one_the_session_was_covered_to() {
    // 16,080 samples: half a hundredth past one second. Both count it to the hundredth above.
    let folder = session();
    audio(&folder.path().join("me.wav"), 1.00501, &[(0.9375, 1.1)]);
    audio(&folder.path().join("others.wav"), 1.0, &[]);
    transcribe(folder.path(), &mut Played::default(), None, 1).unwrap();
    let text = fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap();
    assert!(text.contains(r#"{"speaker":"me","type":"covered","until":1.01}"#), "{text}");
    assert!(text.trim_end().ends_with(r#"{"duration":1.01,"type":"complete"}"#), "{text}");
}

#[test]
fn what_the_engine_writes_on_a_tracks_silence_is_not_kept_and_a_quiet_voice_is() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    // A room's faint noise, 46 dB under the voice, is no voice either.
    audio_of(&folder.path().join("me.wav"), 30.0, &[(1.0, 11.0, 8000), (14.0, 18.0, 40)]);
    // The other side speaks 40 dB lower: its own level, not the loudest track's, is the measure.
    audio_at(&folder.path().join("others.wav"), 30.0, &[(19.0, 29.0)], 80);
    let mut engine = Played {
        // A sentence written on noise, a word with no length on silence, and one within a voice.
        invents: vec![(15.0, 17.0, "je dois checker"), (12.0, 12.0, "ok"), (4.0, 4.0, "oui")],
        ..Default::default()
    };
    transcribe(folder.path(), &mut engine, Some("fr"), 300).unwrap();
    assert_eq!(
        segments(folder.path()),
        [
            ("me".to_string(), 1.0, 11.0),
            ("me".to_string(), 4.0, 4.0),
            ("others".to_string(), 19.0, 29.0),
        ]
    );
}

/// The segments kept on the local track.
fn kept(folder: &Path) -> Vec<(f64, f64)> {
    segments(folder)
        .into_iter()
        .filter(|(speaker, ..)| speaker == "me")
        .map(|(_, start, end)| (start, end))
        .collect()
}

/// The local track's kept segments when it holds these stretches, the engine also writing
/// `invents` in each span of `span` seconds.
fn heard_with(seconds: f64, stretches: &[(f64, f64, i16)], invents: Vec<(f64, f64, &'static str)>, span: usize) -> Vec<(f64, f64)> {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    audio_of(&folder.path().join("me.wav"), seconds, stretches);
    audio(&folder.path().join("others.wav"), seconds, &[]);
    let mut engine = Played {
        invents,
        ..Default::default()
    };
    transcribe(folder.path(), &mut engine, Some("fr"), span).unwrap();
    kept(folder.path())
}

#[test]
fn the_level_is_the_tracks_not_each_spans() {
    // Spans of 20 s. The first holds half a second of a voice 28.5 dB under the second span's
    // ten seconds, and faint noise: judged on its own, its level would be the noise's.
    let kept = heard_with(
        40.0,
        &[(2.0, 2.5, 300), (11.0, 17.0, 40), (21.0, 31.0, 8000), (33.0, 38.0, 40)],
        // Written on the noise of each span — times from the span's start — and a word with no
        // length 0.3 s after the second span's voice.
        vec![(14.0, 16.0, "je dois checker"), (14.3, 14.3, "ok"), (11.3, 11.3, "oui")],
        20,
    );
    assert_eq!(kept, [(2.0, 2.5), (21.0, 31.0)]);
}

#[test]
fn no_sound_shorter_than_ten_seconds_sets_the_level_however_loud() {
    let quiet = (10.0, 60.0, 300);
    // A ringtone at full scale for nine seconds, 40 dB over a quiet voice.
    assert_eq!(
        heard_with(62.0, &[(0.0, 9.0, 32000), quiet], vec![], 300),
        [(0.0, 9.0), (10.0, 60.0)]
    );
    // Ten knocks, a tenth of a second each, spread over the voice.
    let mut knocks: Vec<(f64, f64, i16)> = (0..10).map(|k| (1.0 + k as f64 * 5.0, 1.1 + k as f64 * 5.0, 32000)).collect();
    knocks.push((61.0, 111.0, 300));
    let kept = heard_with(112.0, &knocks, vec![], 300);
    assert!(kept.contains(&(61.0, 111.0)), "{kept:?}");
}

#[test]
fn the_level_is_the_tenth_loudest_second_and_the_gate_34_db_under_it() {
    // A click; ten seconds of voice; a word 32 dB under the voice; and noise 36.5 dB under it.
    let kept = heard_with(
        20.0,
        &[(1.0, 1.1, 32000), (2.0, 12.0, 8000), (13.0, 13.5, 200), (14.5, 18.5, 120)],
        vec![],
        300,
    );
    assert_eq!(kept, [(1.0, 1.1), (2.0, 12.0), (13.0, 13.5)]);
}

#[test]
fn a_side_that_speaks_less_than_ten_seconds_still_loses_what_is_written_on_its_silence() {
    let kept = heard_with(30.0, &[(2.0, 5.0, 8000)], vec![(20.0, 22.0, "je dois checker")], 300);
    assert_eq!(kept, [(2.0, 5.0)]);
}
