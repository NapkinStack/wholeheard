//! The program as a shell starts it, where it opens no device: what it refuses is logged,
//! never with a session's path. What a recording logs is the e2e's (scripts/e2e.sh).

use std::{fs, path::Path, process::Command};

/// A run refused before any device is opened, and the lines of its log.
fn refused(args: &[&str], logs: &Path) -> (String, Vec<serde_json::Value>, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-capture"))
        .args(args)
        .env("WHOLEHEARD_LOG_DIR", logs)
        .env("WHOLEHEARD_TRACE", "making-9")
        // The lowest level a user may ask: a failure is logged, with its id.
        .env("WHOLEHEARD_LOG", "error")
        .output()
        .unwrap();
    assert!(!output.status.success() && output.stdout.is_empty());
    let files: Vec<_> = fs::read_dir(logs).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("capture.") && name.ends_with(".jsonl"), "{name}");
    let text = fs::read_to_string(&files[0]).unwrap();
    let lines = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    (String::from_utf8_lossy(&output.stderr).into_owned(), lines, text)
}

#[test]
fn a_refusal_is_logged_with_its_detail_under_the_makings_id() {
    let logs = tempfile::tempdir().unwrap();
    let (err, lines, _) = refused(&["listen"], logs.path());
    assert!(err.contains("usage: wholeheard-capture"), "{err}");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["message"], "capture.failed");
    assert_eq!(lines[0]["level"], "ERROR");
    assert_eq!(lines[0]["span"]["trace"], "making-9");
    assert_eq!(lines[0]["span"]["program"], "capture");
    assert!(lines[0]["detail"].as_str().unwrap().starts_with("usage: wholeheard-capture"));
}

#[test]
fn a_sessions_folder_that_cannot_hold_one_is_logged_without_its_path() {
    let logs = tempfile::tempdir().unwrap();
    // A file where the sessions' folder should be: refused before any device is opened.
    let not_a_folder = tempfile::NamedTempFile::new().unwrap();
    let sessions = not_a_folder.path().join("sessions");
    let (_, lines, text) = refused(&["record", sessions.to_str().unwrap()], logs.path());
    assert_eq!(lines.last().unwrap()["message"], "capture.failed", "{text}");
    assert!(!text.contains(&*not_a_folder.path().to_string_lossy()), "{text}");
}

/// A log folder whose day file cannot take a byte: every write to it fails.
#[cfg(unix)]
fn full_logs(program: &str) -> tempfile::TempDir {
    let logs = tempfile::tempdir().unwrap();
    let today = std::process::Command::new("date").args(["-u", "+%F"]).output().unwrap();
    let today = String::from_utf8(today.stdout).unwrap();
    std::os::unix::fs::symlink("/dev/full", logs.path().join(format!("{program}.{}.jsonl", today.trim()))).unwrap();
    logs
}

#[cfg(unix)]
#[test]
fn a_log_that_cannot_be_written_says_nothing_on_the_error_stream() {
    let logs = full_logs("capture");
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-capture"))
        .arg("listen")
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(err.starts_with("wholeheard-capture: usage"), "{err}");
}
