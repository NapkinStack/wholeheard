//! What the shell logs of the makings it starts (ADR-0007): an id each, passed to the program
//! in `WHOLEHEARD_TRACE` and logged with how it ended — never the session's path, nor a key.
//! Played programs, no meeting of anyone.
#![cfg(unix)]

use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use wholeheard_shell::{
    summarising::{Event, Summarising},
    transcription::Transcription,
};

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

fn script(path: &Path, text: &str) {
    fs::write(path, format!("#!/bin/sh\n{text}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// The lines logged with this id.
fn of(lines: &[serde_json::Value], trace: &str) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line["trace"] == trace)
        .map(|line| line["message"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn each_making_is_logged_under_the_id_its_program_was_given() {
    // The makings log from threads of their own: the log is the whole test program's.
    let kept = Kept::default();
    let writer = kept.clone();
    tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_writer(move || writer.clone())
        .init();

    let folder = tempfile::tempdir().unwrap();
    let session = folder.path().join("2026-10-20_10-00");
    fs::create_dir(&session).unwrap();

    // A transcript that fails, saying why; its program writes down the id it was given.
    let hears = folder.path().join("wholeheard-transcription");
    script(
        &hears,
        r#"printf %s "$WHOLEHEARD_TRACE" > "$1/../transcript.trace"
echo 'wholeheard-transcription: the model could not be loaded' >&2
exit 1"#,
    );
    let (_making, made) = Transcription::start(&hears, &session, Path::new("/models/ggml-a.bin")).unwrap();
    made.recv_timeout(Duration::from_secs(10)).unwrap();

    // Minutes written, with a key.
    let writes = folder.path().join("wholeheard-summary");
    script(
        &writes,
        r#"printf %s "$WHOLEHEARD_TRACE" > "$1/../minutes.trace"
echo '## A' > "$1/minutes.md"
echo '{"event":"written","file":"x"}'"#,
    );
    let (_making, events) = Summarising::start(
        &writes,
        &session,
        "https://openrouter.ai/api/v1",
        "a-model",
        Some("a-key-for-the-test"),
    )
    .unwrap();
    while !matches!(events.recv_timeout(Duration::from_secs(10)).unwrap(), Event::Ended(_)) {}

    let text = String::from_utf8(kept.0.lock().unwrap().clone()).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let heard = fs::read_to_string(folder.path().join("transcript.trace")).unwrap();
    let wrote = fs::read_to_string(folder.path().join("minutes.trace")).unwrap();
    assert!(!heard.is_empty() && heard != wrote, "{heard} {wrote}");
    assert_eq!(of(&lines, &heard), ["shell.transcript", "shell.transcript_failed"], "{text}");
    assert_eq!(of(&lines, &wrote), ["shell.minutes", "shell.minutes_written"], "{text}");
    let failed = lines.iter().find(|line| line["message"] == "shell.transcript_failed").unwrap();
    assert_eq!(failed["detail"], "wholeheard-transcription: the model could not be loaded");
    let asked = lines.iter().find(|line| line["message"] == "shell.minutes").unwrap();
    assert_eq!(
        (&asked["session"], &asked["host"], &asked["key"]),
        (&"2026-10-20_10-00".into(), &"openrouter.ai".into(), &true.into())
    );
    for said in ["a-key-for-the-test", &*folder.path().to_string_lossy()] {
        assert!(!text.contains(said), "the log holds {said:?}: {text}");
    }
}
