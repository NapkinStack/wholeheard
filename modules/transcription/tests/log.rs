//! What the step logs while it hears a session (ADR-0007): each span, how long it took and how
//! many segments it gave — never a word heard, the context, nor the session's path.

use std::{
    fs, io,
    sync::{Arc, Mutex},
};

use wholeheard_transcription::{
    Segment,
    step::{Engine, RATE, transcribe},
};

/// Hears every span as one sentence nobody may read in a log.
struct Heard;

impl Engine for Heard {
    fn name(&self) -> (String, String) {
        ("played".into(), "a-model".into())
    }

    fn hear(&mut self, sound: &[f32], _: Option<&str>, _: Option<&str>) -> io::Result<Vec<Segment>> {
        let end = sound.len() as f64 / RATE as f64;
        let text = "le chiffre secret est 4217".into();
        Ok(vec![Segment { start: 0.0, end, text }])
    }
}

/// Where the log is written, for the test to read.
#[derive(Clone, Default)]
struct Kept(Arc<Mutex<Vec<u8>>>);

impl io::Write for Kept {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn audio(path: &std::path::Path, seconds: usize) {
    let mut bytes = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff".to_vec();
    for n in 0..seconds * RATE {
        bytes.extend_from_slice(&(if n % 40 < 20 { 8000i16 } else { -8000 }).to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn each_span_is_logged_and_nothing_that_was_said() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(
        folder.path().join("session.json"),
        r#"{"contract":"session-audio/v1","started":"2026-10-13T14:05:12+02:00","stopped":"2026-10-13T14:05:32+02:00"}"#,
    )
    .unwrap();
    fs::write(folder.path().join("context.txt"), "Quatre personnes, le projet Hirondelle.\n").unwrap();
    audio(&folder.path().join("me.wav"), 20);
    audio(&folder.path().join("others.wav"), 10);

    let kept = Kept::default();
    let writer = kept.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || transcribe(folder.path(), &mut Heard, Some("fr"), 10)).unwrap();

    let text = String::from_utf8(kept.0.lock().unwrap().clone()).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let steps: Vec<_> = lines.iter().map(|line| line["message"].as_str().unwrap()).collect();
    assert_eq!(
        steps,
        [
            "transcription.tracks",
            "transcription.echo",
            "transcription.span",
            "transcription.span",
            "transcription.span"
        ],
        "{text}"
    );
    assert_eq!((lines[0]["me"].as_f64(), lines[0]["others"].as_f64()), (Some(20.0), Some(10.0)));
    assert_eq!((lines[2]["speaker"].as_str(), lines[2]["segments"].as_u64()), (Some("me"), Some(1)));
    assert_eq!(
        (lines[4]["speaker"].as_str(), lines[4]["until"].as_f64()),
        (Some("others"), Some(10.0))
    );
    for said in ["secret", "4217", "Quatre", "Hirondelle", &*folder.path().to_string_lossy()] {
        assert!(!text.contains(said), "the log holds {said:?}: {text}");
    }
    // What was heard is in the transcript, where it belongs.
    assert!(fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap().contains("4217"));
}
