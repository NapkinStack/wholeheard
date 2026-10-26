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
    recording::{Recording, Said},
    summarising::{Event, Summarising},
    transcription::{Hearer, Transcription},
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
        .filter(|line| line["span"]["trace"] == trace)
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
        .with_current_span(true)
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
echo "wholeheard-transcription: $1/derived/ cannot be written" >&2
exit 1"#,
    );
    let (_making, made) = Transcription::start(&hears, &session, &Hearer::Model("/models/ggml-a.bin".into())).unwrap();
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

    // A recording whose program fails, naming the session's full path as an older version did.
    let records = folder.path().join("wholeheard-capture");
    script(
        &records,
        r#"printf %s "$WHOLEHEARD_TRACE" > "$2/../recording.trace"
echo "{\"event\":\"started\",\"folder\":\"$2/2026-10-20_10-00\"}"
echo "wholeheard-capture: $2/2026-10-20_10-00/local.wav: No space left on device" >&2
exit 1"#,
    );
    let sessions = folder.path().join("sessions");
    fs::create_dir(&sessions).unwrap();
    let (_recording, said) = Recording::start(&records, &sessions).unwrap();
    while !matches!(said.recv_timeout(Duration::from_secs(10)).unwrap(), Said::Failed(_)) {}

    // A program that is not there: the making is said to have failed, not left begun.
    assert!(
        Transcription::start(
            &folder.path().join("nowhere"),
            &session,
            &Hearer::Model("/models/ggml-a.bin".into())
        )
        .is_err()
    );
    assert!(Summarising::start(&folder.path().join("nowhere"), &session, "http://127.0.0.1:9/v1", "m", None).is_err());
    assert!(Recording::start(&folder.path().join("nowhere"), &sessions).is_err());

    let text = String::from_utf8(kept.0.lock().unwrap().clone()).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let heard = fs::read_to_string(folder.path().join("transcript.trace")).unwrap();
    let wrote = fs::read_to_string(folder.path().join("minutes.trace")).unwrap();
    assert!(!heard.is_empty() && heard != wrote, "{heard} {wrote}");
    assert_eq!(of(&lines, &heard), ["shell.transcript", "shell.transcript_failed"], "{text}");
    assert_eq!(of(&lines, &wrote), ["shell.minutes", "shell.minutes_written"], "{text}");
    let failed = lines.iter().find(|line| line["message"] == "shell.transcript_failed").unwrap();
    // An older program named the session's path: the log holds the folder of sessions as a word.
    assert_eq!(
        failed["detail"],
        "wholeheard-transcription: <sessions>/2026-10-20_10-00/derived/ cannot be written"
    );
    let asked = lines.iter().find(|line| line["message"] == "shell.minutes").unwrap();
    assert_eq!(
        (&asked["session"], &asked["host"], &asked["key"]),
        (&"2026-10-20_10-00".into(), &"openrouter.ai".into(), &true.into())
    );
    let recorded = fs::read_to_string(folder.path().join("recording.trace")).unwrap();
    assert_eq!(
        of(&lines, &recorded),
        ["shell.recording", "shell.recording_started", "shell.recording_failed"],
        "{text}"
    );
    let failed = lines.iter().find(|line| line["message"] == "shell.recording_failed").unwrap();
    assert_eq!(
        failed["detail"],
        "wholeheard-capture: <sessions>/2026-10-20_10-00/local.wav: No space left on device"
    );
    let missing: Vec<_> = lines
        .iter()
        .filter(|line| line["message"].as_str().unwrap().starts_with("shell.transcript"))
        .collect();
    assert_eq!(missing.len(), 4, "{text}");
    assert_eq!(missing[3]["message"], "shell.transcript_failed");
    let failed: Vec<_> = lines.iter().map(|line| line["message"].as_str().unwrap()).collect();
    assert_eq!(failed.iter().filter(|step| **step == "shell.minutes_failed").count(), 1, "{text}");
    assert_eq!(failed.iter().filter(|step| **step == "shell.recording_failed").count(), 2, "{text}");
    for said in ["a-key-for-the-test", "/models", &*folder.path().to_string_lossy()] {
        assert!(!text.contains(said), "the log holds {said:?}: {text}");
    }
}
