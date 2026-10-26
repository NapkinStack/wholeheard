//! A transcript made, tried with a program played here: a script given the same arguments as
//! `wholeheard-transcription`. No sound, no model, nobody heard.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Mutex, time::Duration};

use wholeheard_shell::transcription::{Hearer, Made, Transcription};

/// The end of a transcript made by a script — a test that waits ten seconds has failed. One
/// at a time: a script still open for writing while another starts cannot be run.
fn made(folder: &Path, script: &str) -> (Transcription, std::sync::mpsc::Receiver<Made>) {
    made_by(folder, script, &Hearer::Model(Path::new("/models/ggml.bin").to_path_buf()))
}

/// The same, heard by `hearer` (#459: every script of this file is written and started here).
fn made_by(folder: &Path, script: &str, hearer: &Hearer) -> (Transcription, std::sync::mpsc::Receiver<Made>) {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap();
    let path = folder.join("wholeheard-transcription");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    Transcription::start(&path, &folder.join("session"), hearer).unwrap()
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
        "echo 'loading' >&2\necho 'wholeheard-transcription: the model cannot be loaded' >&2\necho '' >&2\nexit 1",
    );
    assert_eq!(
        end(&said),
        Made::Failed("wholeheard-transcription: the model cannot be loaded".into())
    );
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

/// Whether a process of this number is still there.
fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .status()
        .unwrap()
        .success()
}

#[test]
fn a_transcript_ended_by_wholeheard_ends_with_what_it_started_and_is_not_written() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("session")).unwrap();
    let started = folder.path().join("started");
    let script = format!(
        "sleep 30 &\necho $! > {}\nwait\necho '{{}}' > \"$1/transcript.jsonl\"",
        started.display()
    );
    let (transcription, said) = made(folder.path(), &script);
    std::thread::sleep(Duration::from_millis(300));
    let child = fs::read_to_string(&started).unwrap();
    assert!(alive(&child));
    transcription.end();
    assert_eq!(end(&said), Made::Failed(String::new()));
    assert!(!alive(&child), "what the program started is left running");
    assert!(!folder.path().join("session/transcript.jsonl").exists());
}

#[test]
fn what_the_program_said_last_is_kept_when_what_it_started_holds_its_error_stream() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("session")).unwrap();
    let started = folder.path().join("started");
    let script = format!("echo 'the engine stopped' >&2\nsleep 30 &\necho $! > {}\nexit 1", started.display());
    let (_, said) = made(folder.path(), &script);
    let begun = std::time::Instant::now();
    assert_eq!(end(&said), Made::Failed("the engine stopped".into()));
    assert!(begun.elapsed() < Duration::from_secs(3), "{:?}", begun.elapsed());
    let child = fs::read_to_string(&started).unwrap();
    std::process::Command::new("kill").arg(child.trim()).status().unwrap();
}

#[test]
fn the_last_recording_is_the_latest_session_of_the_folder() {
    let sessions = tempfile::tempdir().unwrap();
    assert_eq!(wholeheard_shell::transcription::latest(sessions.path()), None);
    for name in ["2026-10-13_14-05", "2026-10-16_09-30", "2026-10-14_11-00"] {
        fs::create_dir(sessions.path().join(name)).unwrap();
        fs::write(sessions.path().join(name).join("session.json"), "{}").unwrap();
    }
    // A folder that is no session, however its name sorts, is not one.
    fs::create_dir(sessions.path().join("2027-notes")).unwrap();
    fs::write(sessions.path().join("zz.txt"), "").unwrap();
    assert_eq!(
        wholeheard_shell::transcription::latest(sessions.path()),
        Some(sessions.path().join("2026-10-16_09-30"))
    );
    assert_eq!(wholeheard_shell::transcription::latest(Path::new("/nowhere")), None);
}

#[test]
fn apple_s_speechanalyzer_is_asked_for_by_name_in_the_language_of_the_interface() {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("session")).unwrap();
    let asked = folder.path().join("asked");
    let hearer = Hearer::SpeechAnalyzer { language: "fr".into() };
    let (_running, made) = made_by(
        folder.path(),
        &format!("echo \"$@\" > '{}'\ntouch \"$1/transcript.jsonl\"", asked.display()),
        &hearer,
    );
    assert!(matches!(end(&made), Made::Written(_)));
    let said = fs::read_to_string(&asked).unwrap();
    assert!(said.trim().ends_with("--engine speechanalyzer --language fr"), "{said}");
}

#[test]
fn speechanalyzer_hears_only_while_this_mac_offers_it_with_its_model() {
    use wholeheard_shell::settings::Engine;
    let model = Path::new("/models/ggml.bin");
    let whisper = Hearer::Model(model.to_path_buf());
    assert_eq!(
        Hearer::chosen(Engine::SpeechAnalyzer, model, "fr", || true),
        Hearer::SpeechAnalyzer { language: "fr".into() }
    );
    assert_eq!(
        Hearer::chosen(Engine::SpeechAnalyzer, model, "fr", || false),
        whisper,
        "no longer offered: Whisper hears"
    );
    let mut asked = false;
    assert_eq!(
        Hearer::chosen(Engine::Whisper, model, "fr", || {
            asked = true;
            true
        }),
        whisper
    );
    assert!(!asked, "the Mac is asked nothing while Whisper is chosen");
    assert_eq!(Hearer::SpeechAnalyzer { language: "en".into() }.name(), "SpeechAnalyzer");
    assert_eq!(whisper.name(), "ggml.bin");
}
