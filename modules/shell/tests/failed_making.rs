//! A making that fails is said in plain words to the user (D29): what its program said is then
//! in the log only — the shell's failure line, with that sentence, under the id the program was
//! given. A played program, no meeting of anyone. From the verifier's probe on #286.
#![cfg(unix)]

use std::{
    fs, io,
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
    time::Duration,
};

use wholeheard_shell::summarising::{Event, Summarising};

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

#[test]
fn minutes_that_fail_leave_their_programs_sentence_in_the_log_under_their_id() {
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
    let program = folder.path().join("wholeheard-summary");
    fs::write(
        &program,
        "#!/bin/sh\nprintf %s \"$WHOLEHEARD_TRACE\" > \"$1/../minutes.trace\"\necho \"wholeheard-summary: the provider's answer is not minutes\" >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    let (_making, events) = Summarising::start(&program, &session, "http://127.0.0.1:9/v1", "m", None).unwrap();
    while !matches!(events.recv_timeout(Duration::from_secs(10)).unwrap(), Event::Ended(_)) {}
    let text = String::from_utf8(kept.0.lock().unwrap().clone()).unwrap();
    let given = fs::read_to_string(folder.path().join("minutes.trace")).unwrap();
    let failed: serde_json::Value = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|line| line["message"] == "shell.minutes_failed")
        .unwrap_or_else(|| panic!("no shell.minutes_failed: {text}"));
    assert_eq!(failed["span"]["trace"], given.as_str(), "{text}");
    assert!(
        failed["detail"].as_str().unwrap().contains("the provider's answer is not minutes"),
        "{text}"
    );
}
