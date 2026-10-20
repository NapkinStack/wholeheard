//! What the engine heard, kept apart from the tracks and made again the same. Sound made here,
//! no voice of anyone.

mod sound;

use std::{fs, io, path::Path};

use sound::{energy, room, speaking};
use wholeheard_transcription::{
    Segment,
    step::{DERIVED, Engine, MADE, RATE, transcribe},
};

/// An engine that writes how loud each span it hears is, and keeps what it was given.
#[derive(Default)]
struct Loudness {
    heard: Vec<Vec<f32>>,
    /// Says each span holds a word in its first tenth of a second only: nothing to hear again.
    short: bool,
}

impl Engine for Loudness {
    fn name(&self) -> (String, String) {
        ("loudness".into(), "a-model".into())
    }

    fn hear(&mut self, sound: &[f32], _: Option<&str>, _: Option<&str>) -> io::Result<Vec<Segment>> {
        self.heard.push(sound.to_vec());
        let end = if self.short { 0.1 } else { sound.len() as f64 / RATE as f64 };
        Ok(vec![Segment {
            start: 0.0,
            end,
            text: format!("{:.6}", energy(sound)),
        }])
    }
}

fn write_wav(path: &Path, sound: &[f32]) {
    let mut bytes = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff".to_vec();
    bytes.extend(
        sound
            .iter()
            .flat_map(|s| ((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes()),
    );
    fs::write(path, bytes).unwrap();
}

fn samples(path: &Path) -> Vec<f32> {
    fs::read(path).unwrap()[44..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect()
}

const STOPPED: &str = r#"{"contract":"session-audio/v1","started":"2026-10-13T14:05:12+02:00","stopped":"2026-10-13T14:06:22+02:00"}"#;
const GOING: &str = r#"{"contract":"session-audio/v1","started":"2026-10-13T14:05:12+02:00"}"#;

/// The two tracks of a meeting of 30 s, with the echo of loudspeakers in the microphone or not.
fn tracks(echo: bool) -> (Vec<f32>, Vec<f32>) {
    let remote = speaking(30.0, 0.0, 0.3, 5);
    let mut local = speaking(30.0, 2.0, 0.2, 6);
    if echo {
        local = local.iter().zip(room(&remote, 0.5)).map(|(a, b)| a + b).collect();
    }
    (local, remote)
}

fn session(echo: bool) -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    let (local, remote) = tracks(echo);
    write_wav(&folder.path().join("me.wav"), &local);
    write_wav(&folder.path().join("others.wav"), &remote);
    folder
}

fn recorded(path: &Path) -> Vec<f32> {
    fs::read(path).unwrap()[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f32::from(i16::from_le_bytes(*b)) / 32768.0)
        .collect()
}

#[test]
fn what_the_engine_heard_is_kept_apart_says_what_made_it_and_is_made_again_the_same() {
    for echo in [true, false] {
        let folder = session(echo);
        let mut engine = Loudness::default();
        transcribe(folder.path(), &mut engine, Some("fr"), 300).unwrap();
        let derived = folder.path().join(DERIVED);
        // What is kept is what the engine was given, to the bit, in a file that says so.
        assert_eq!(
            engine.heard,
            [samples(&derived.join("local.wav")), samples(&derived.join("remote.wav"))]
        );
        let header = fs::read(derived.join("local.wav")).unwrap()[..44].to_vec();
        let field = |at: usize, size: usize| header[at..at + size].iter().rev().fold(0u32, |n, &b| n << 8 | u32::from(b));
        assert_eq!((field(20, 2), field(22, 2), field(24, 4), field(34, 2)), (3, 1, 16_000, 32));
        let made: serde_json::Value = serde_json::from_slice(&fs::read(derived.join(MADE)).unwrap()).unwrap();
        assert_eq!(made["local"]["echo"]["taken_out"], echo, "{made}");
        assert_eq!(
            (&made["rate"], &made["local"]["echo"]["threshold_db"]),
            (&16_000.into(), &3.0.into())
        );
        let measured = made["local"]["echo"]["measured_db"].as_f64().unwrap();
        assert_eq!(measured >= 3.0, echo, "{made}");
        assert!(made["local"]["echo"]["delay_ms"].is_number(), "{made}");
        assert_eq!(made["local"]["from"], serde_json::json!(["me.wav", "others.wav"]));
        assert_eq!(made["remote"]["from"], serde_json::json!(["others.wav"]));
        // The microphone with no echo is what was recorded, to the sample; the remote always is.
        assert_eq!(samples(&derived.join("remote.wav")), recorded(&folder.path().join("others.wav")));
        assert_eq!(
            samples(&derived.join("local.wav")) == recorded(&folder.path().join("me.wav")),
            !echo
        );
        // The tracks are not touched.
        let tracks = [
            fs::read(folder.path().join("me.wav")).unwrap(),
            fs::read(folder.path().join("others.wav")).unwrap(),
        ];
        // Deleted — what was made and the transcript — they are made again, the same to the byte.
        let before: Vec<Vec<u8>> = ["local.wav", "remote.wav", MADE]
            .iter()
            .map(|name| fs::read(derived.join(name)).unwrap())
            .collect();
        let transcript = fs::read(folder.path().join("transcript.jsonl")).unwrap();
        fs::remove_dir_all(&derived).unwrap();
        transcribe(folder.path(), &mut Loudness::default(), Some("fr"), 300).unwrap();
        let again: Vec<Vec<u8>> = ["local.wav", "remote.wav", MADE]
            .iter()
            .map(|name| fs::read(derived.join(name)).unwrap())
            .collect();
        assert_eq!(before, again);
        fs::remove_file(folder.path().join("transcript.jsonl")).unwrap();
        fs::remove_dir_all(&derived).unwrap();
        transcribe(folder.path(), &mut Loudness::default(), Some("fr"), 300).unwrap();
        assert_eq!(fs::read(folder.path().join("transcript.jsonl")).unwrap(), transcript);
        assert_eq!(
            [
                fs::read(folder.path().join("me.wav")).unwrap(),
                fs::read(folder.path().join("others.wav")).unwrap()
            ],
            tracks
        );
    }
}

#[test]
fn a_file_of_what_was_heard_deleted_alone_is_made_again() {
    let folder = session(true);
    transcribe(folder.path(), &mut Loudness::default(), Some("fr"), 300).unwrap();
    let local = folder.path().join(DERIVED).join("local.wav");
    let before = fs::read(&local).unwrap();
    fs::remove_file(&local).unwrap();
    transcribe(folder.path(), &mut Loudness::default(), Some("fr"), 300).unwrap();
    assert_eq!(fs::read(&local).unwrap(), before);
}

#[test]
fn whether_there_is_an_echo_is_decided_once_a_transcript_has_begun_and_kept() {
    // The first 8 s of a recording going on, the remote not heard yet: no echo to measure.
    let folder = tempfile::tempdir().unwrap();
    let remote = speaking(30.0, 10.0, 0.3, 5);
    let local: Vec<f32> = speaking(30.0, 0.0, 0.2, 6)
        .iter()
        .zip(room(&remote, 0.5))
        .map(|(a, b)| a + b)
        .collect();
    let begun = 8 * RATE;
    fs::write(folder.path().join("session.json"), GOING).unwrap();
    write_wav(&folder.path().join("me.wav"), &local[..begun]);
    write_wav(&folder.path().join("others.wav"), &remote[..begun]);
    let mut engine = Loudness {
        short: true,
        ..Default::default()
    };
    transcribe(folder.path(), &mut engine, Some("fr"), 300).unwrap();
    let made =
        |folder: &Path| -> serde_json::Value { serde_json::from_slice(&fs::read(folder.join(DERIVED).join(MADE)).unwrap()).unwrap() };
    assert_eq!(made(folder.path())["local"]["echo"]["taken_out"], false);
    // The whole meeting, stopped, the echo in it: the transcript goes on, and its beginning was
    // heard with no canceller — so is the rest, and what is kept is what the engine was given.
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    write_wav(&folder.path().join("me.wav"), &local);
    write_wav(&folder.path().join("others.wav"), &remote);
    let mut engine = Loudness {
        short: true,
        ..Default::default()
    };
    transcribe(folder.path(), &mut engine, Some("fr"), 300).unwrap();
    assert_eq!(made(folder.path())["local"]["echo"]["taken_out"], false);
    let kept = samples(&folder.path().join(DERIVED).join("local.wav"));
    assert_eq!(kept, recorded(&folder.path().join("me.wav")));
    assert_eq!(engine.heard[0], kept[begun..]);
    // Begun again from nothing, the whole meeting is measured: the echo is taken out.
    fs::remove_file(folder.path().join("transcript.jsonl")).unwrap();
    transcribe(folder.path(), &mut Loudness::default(), Some("fr"), 300).unwrap();
    assert_eq!(made(folder.path())["local"]["echo"]["taken_out"], true);
}

#[test]
fn a_place_for_what_was_heard_that_cannot_be_written_is_said_by_its_name() {
    use std::os::unix::fs::PermissionsExt;
    let folder = session(false);
    fs::create_dir(folder.path().join(DERIVED)).unwrap();
    fs::set_permissions(folder.path().join(DERIVED), fs::Permissions::from_mode(0o555)).unwrap();
    let refused = transcribe(folder.path(), &mut Loudness::default(), None, 30)
        .unwrap_err()
        .to_string();
    fs::set_permissions(folder.path().join(DERIVED), fs::Permissions::from_mode(0o755)).unwrap();
    // The sentence is logged: the session's path is not in it (ADR-0007).
    assert!(refused.starts_with("derived/ cannot be written"), "{refused}");
    assert!(!refused.contains(&*folder.path().to_string_lossy()), "{refused}");
}
