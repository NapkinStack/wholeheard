//! A transcript made, tried with a program played here: a script given the same arguments as
//! `summit-transcription`. No sound, no model, nobody heard.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Mutex, time::Duration};

use summit_shell::transcription::{Made, Transcription};

/// The end of a transcript made by a script — a test that waits ten seconds has failed. One
/// at a time: a script still open for writing while another starts cannot be run.
fn made(folder: &Path, script: &str) -> (Transcription, std::sync::mpsc::Receiver<Made>) {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap();
    let path = folder.join("summit-transcription");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    Transcription::start(&path, &folder.join("session"), Path::new("/models/ggml.bin")).unwrap()
}

fn end(made: &std::sync::mpsc::Receiver<Made>) -> Made {
    made.recv_timeout(Duration::from_secs(10)).expect("the transcript never ended")
}

#[test]
fn a_transcript_is_written_when_the_program_ends_well_and_the_file_is_there() {
    let folder = tempfile::tempdir().unwrap();
    let session = folder.path().join("session");
    fs::create_dir(&session).unwrap();
    let (_, said) = made(
        folder.path(),
        r#"[ "$2" = --model ] && [ "$3" = /models/ggml.bin ] || exit 9
echo '{"event":"hearing"}'
echo '{}' > "$1/transcript.jsonl"
echo '{"event":"written"}'"#,
    );
    assert_eq!(end(&said), Made::Written(session));
}

#[test]
fn a_transcript_that_failed_says_the_programs_last_sentence() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("session")).unwrap();
    let (_, said) = made(
        folder.path(),
        "echo 'loading' >&2\necho 'summit-transcription: the model cannot be loaded' >&2\necho '' >&2\nexit 1",
    );
    assert_eq!(end(&said), Made::Failed("summit-transcription: the model cannot be loaded".into()));
    // A transcript only grows: one that failed halfway is there, and is not whole.
    let (_, said) = made(
        folder.path(),
        "echo '{}' > \"$1/transcript.jsonl\"\necho 'the engine stopped' >&2\nexit 1",
    );
    assert_eq!(end(&said), Made::Failed("the engine stopped".into()));
    // Ended well without the file is no transcript either.
    fs::remove_file(folder.path().join("session/transcript.jsonl")).unwrap();
    let (_, said) = made(folder.path(), "exit 0");
    assert_eq!(end(&said), Made::Failed(String::new()));
}

#[test]
fn a_transcript_ended_by_summit_is_not_written_and_ends_at_once() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("session")).unwrap();
    // A program of its own keeps the error stream open: the end is not held up by it.
    let (transcription, said) = made(folder.path(), "sleep 30\necho '{}' > \"$1/transcript.jsonl\"");
    std::thread::sleep(Duration::from_millis(300));
    transcription.end();
    assert_eq!(end(&said), Made::Failed(String::new()));
    assert!(!folder.path().join("session/transcript.jsonl").exists());
}

#[test]
fn the_last_recording_is_the_latest_session_of_the_folder() {
    let sessions = tempfile::tempdir().unwrap();
    assert_eq!(summit_shell::transcription::latest(sessions.path()), None);
    for name in ["2026-10-13_14-05", "2026-10-16_09-30", "2026-10-14_11-00"] {
        fs::create_dir(sessions.path().join(name)).unwrap();
        fs::write(sessions.path().join(name).join("session.json"), "{}").unwrap();
    }
    // A folder that is no session, however its name sorts, is not one.
    fs::create_dir(sessions.path().join("2027-notes")).unwrap();
    fs::write(sessions.path().join("zz.txt"), "").unwrap();
    assert_eq!(
        summit_shell::transcription::latest(sessions.path()),
        Some(sessions.path().join("2026-10-16_09-30"))
    );
    assert_eq!(summit_shell::transcription::latest(Path::new("/nowhere")), None);
}
