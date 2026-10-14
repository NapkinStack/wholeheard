//! What the engine heard, kept apart from the tracks and made again the same. Sound made here,
//! no voice of anyone.

mod sound;

use std::{fs, io, path::Path};

use sound::{energy, room, speaking};
use summit_transcription::{
    Segment,
    step::{DERIVED, Engine, MADE, RATE, transcribe},
};

/// An engine that writes how loud each span it hears is: what it was given, told back.
struct Loudness;

impl Engine for Loudness {
    fn name(&self) -> (String, String) {
        ("loudness".into(), "a-model".into())
    }

    fn hear(&mut self, sound: &[f32], _: Option<&str>, _: Option<&str>) -> io::Result<Vec<Segment>> {
        let end = sound.len() as f64 / RATE as f64;
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

fn session(echo: bool) -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("session.json"), STOPPED).unwrap();
    let remote = speaking(30.0, 0.0, 0.3, 5);
    let mut local = speaking(30.0, 2.0, 0.2, 6);
    if echo {
        local = local.iter().zip(room(&remote, 0.5)).map(|(a, b)| a + b).collect();
    }
    write_wav(&folder.path().join("me.wav"), &local);
    write_wav(&folder.path().join("others.wav"), &remote);
    folder
}

#[test]
fn what_the_engine_heard_is_kept_apart_says_what_made_it_and_is_made_again_the_same() {
    for echo in [true, false] {
        let folder = session(echo);
        transcribe(folder.path(), &mut Loudness, Some("fr"), 300).unwrap();
        let derived = folder.path().join(DERIVED);
        let made: serde_json::Value = serde_json::from_slice(&fs::read(derived.join(MADE)).unwrap()).unwrap();
        assert_eq!(made["local"]["echo"]["taken_out"], echo, "{made}");
        assert_eq!(made["local"]["from"], serde_json::json!(["me.wav", "others.wav"]));
        assert_eq!(made["remote"]["from"], serde_json::json!(["others.wav"]));
        // The microphone with no echo is what was recorded, to the sample; the remote always is.
        let recorded = |name: &str| -> Vec<f32> {
            fs::read(folder.path().join(name)).unwrap()[44..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| f32::from(i16::from_le_bytes(*b)) / 32768.0)
                .collect()
        };
        assert_eq!(samples(&derived.join("remote.wav")), recorded("others.wav"));
        assert_eq!(samples(&derived.join("local.wav")) == recorded("me.wav"), !echo);
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
        transcribe(folder.path(), &mut Loudness, Some("fr"), 300).unwrap();
        let again: Vec<Vec<u8>> = ["local.wav", "remote.wav", MADE]
            .iter()
            .map(|name| fs::read(derived.join(name)).unwrap())
            .collect();
        assert_eq!(before, again);
        fs::remove_file(folder.path().join("transcript.jsonl")).unwrap();
        fs::remove_dir_all(&derived).unwrap();
        transcribe(folder.path(), &mut Loudness, Some("fr"), 300).unwrap();
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
