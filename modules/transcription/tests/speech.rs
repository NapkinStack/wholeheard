//! Apple's SpeechAnalyzer behind the step's interface (D42, ADR-0012), tried with a program played
//! here in place of `wholeheard-speech`: what it is asked, and what is made of what it says. Apple's
//! engine itself is tried on a Mac — the macOS workflow, the colleague's.
//!
//! The program played here is a few lines of shell: these tests run where one runs, never on Windows.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

use wholeheard_transcription::{
    speech::SpeechAnalyzer,
    step::{Engine, RATE},
};

/// A program in place of `wholeheard-speech`: it writes what it was asked to `asked`, then runs
/// `answer`, a few lines of shell.
fn played(folder: &Path, answer: &str) -> std::path::PathBuf {
    let program = folder.join("wholeheard-speech");
    let asked = folder.join("asked");
    fs::write(
        &program,
        format!(
            "#!/bin/sh\necho \"$@\" > '{}'\ncp \"$2\" '{}.wav' 2>/dev/null\n{answer}\n",
            asked.display(),
            asked.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    program
}

#[test]
fn a_span_is_handed_as_a_file_of_the_session_s_form_and_its_lines_become_segments() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(
        folder.path(),
        r#"echo '{"start":0.5,"end":1.25,"text":"Bonjour à tous."}'; echo '{"start":2.0,"end":3.5,"text":"On commence ?"}'"#,
    );
    let mut engine = SpeechAnalyzer::at(&program, "fr").unwrap();
    assert_eq!(engine.name(), ("SpeechAnalyzer".to_string(), "Apple fr-FR".to_string()));
    let sound = vec![0.25f32; RATE * 4];
    let heard = engine.hear(&sound, Some("fr"), None).unwrap();
    let said: Vec<_> = heard
        .iter()
        .map(|segment| (segment.start, segment.end, segment.text.as_str()))
        .collect();
    assert_eq!(said, [(0.5, 1.25, "Bonjour à tous."), (2.0, 3.5, "On commence ?")]);
    // Asked to transcribe one file, in the language's locale.
    let asked = fs::read_to_string(folder.path().join("asked")).unwrap();
    let words: Vec<_> = asked.split_whitespace().collect();
    assert_eq!((words[0], words[2]), ("transcribe", "fr-FR"), "{asked}");
    // The file is one channel, 16 kHz, 16-bit, as long as the span; and it is gone after.
    let file = fs::read(folder.path().join("asked.wav")).unwrap();
    assert_eq!(&file[22..24], &1u16.to_le_bytes());
    assert_eq!(&file[24..28], &16_000u32.to_le_bytes());
    assert_eq!(&file[34..36], &16u16.to_le_bytes());
    assert_eq!(file.len(), 44 + 2 * sound.len());
    assert!(!Path::new(words[1]).exists(), "the span's file is removed once heard");
}

#[test]
fn what_the_program_refuses_is_said_in_its_own_sentence() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(folder.path(), "echo 'SpeechTranscriber is not available on this Mac' >&2; exit 1");
    let mut engine = SpeechAnalyzer::at(&program, "fr").unwrap();
    let error = engine.hear(&vec![0.25f32; RATE], Some("fr"), None).unwrap_err();
    assert!(
        error.to_string().contains("SpeechTranscriber is not available on this Mac"),
        "{error}"
    );
}

#[test]
fn a_line_that_is_not_a_segment_refuses_the_span() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(folder.path(), r#"echo '{"start":"soon","text":"Bonjour"}'"#);
    let mut engine = SpeechAnalyzer::at(&program, "fr").unwrap();
    assert!(engine.hear(&vec![0.25f32; RATE], Some("fr"), None).is_err());
}

#[test]
fn only_a_language_it_has_a_locale_for_is_taken() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(folder.path(), "true");
    assert_eq!(SpeechAnalyzer::at(&program, "en").unwrap().name().1, "Apple en-US");
    assert!(SpeechAnalyzer::at(&program, "zz").is_err());
    assert!(SpeechAnalyzer::at(&folder.path().join("nowhere"), "fr").is_err());
}

fn run(folder: &Path, program: &Path, args: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .arg(folder)
        .args(args)
        .env("WHOLEHEARD_SPEECH", program)
        .env("WHOLEHEARD_LOG_DIR", Path::new(env!("CARGO_TARGET_TMPDIR")).join("logs"))
        .output()
        .unwrap();
    (output.status.success(), String::from_utf8_lossy(&output.stderr).into_owned())
}

/// A session of the first form, five seconds, sound on each side.
fn session(folder: &Path) {
    fs::write(
        folder.join("session.json"),
        r#"{"contract":"session-audio/v1","started":"2026-10-26T10:00:00+01:00","stopped":"2026-10-26T10:00:05+01:00"}"#,
    )
    .unwrap();
    let mut audio = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff".to_vec();
    for n in 0..RATE * 5 {
        audio.extend_from_slice(&(if (RATE..RATE * 3).contains(&n) { 8000i16 } else { 0 }).to_le_bytes());
    }
    fs::write(folder.join("me.wav"), &audio).unwrap();
    fs::write(folder.join("others.wav"), &audio).unwrap();
}

#[test]
fn the_program_writes_the_transcript_with_speechanalyzer_named_in_its_header() {
    let session_folder = tempfile::tempdir().unwrap();
    let programs = tempfile::tempdir().unwrap();
    session(session_folder.path());
    let program = played(programs.path(), r#"echo '{"start":1.0,"end":2.5,"text":"Bonjour."}'"#);
    let (ok, err) = run(session_folder.path(), &program, &["--engine", "speechanalyzer", "--language", "fr"]);
    assert!(ok, "{err}");
    let transcript = fs::read_to_string(session_folder.path().join("transcript.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = transcript.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(
        (lines[0]["engine"].as_str(), lines[0]["model"].as_str()),
        (Some("SpeechAnalyzer"), Some("Apple fr-FR"))
    );
    assert_eq!(lines.iter().filter(|line| line["type"] == "segment").count(), 2, "{transcript}");
}

#[test]
fn speechanalyzer_is_asked_for_with_a_language() {
    let session_folder = tempfile::tempdir().unwrap();
    session(session_folder.path());
    let programs = tempfile::tempdir().unwrap();
    let program = played(programs.path(), "true");
    for (args, expected) in [
        (vec!["--engine", "speechanalyzer"], "usage"),
        (vec!["--engine", "speechanalyzer", "--language", "zz"], "French or English"),
        (vec!["--engine", "another", "--language", "fr"], "usage"),
    ] {
        let (ok, err) = run(session_folder.path(), &program, &args);
        assert!(!ok && err.contains(expected) && err.trim().lines().count() == 1, "{args:?}: {err}");
    }
}

#[test]
fn a_program_that_does_not_answer_is_ended_and_its_span_said() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(folder.path(), "sleep 30");
    let mut engine = SpeechAnalyzer::at(&program, "fr")
        .unwrap()
        .patient(std::time::Duration::from_millis(500));
    let began = std::time::Instant::now();
    let error = engine.hear(&vec![0.25f32; RATE / 4], Some("fr"), None).unwrap_err();
    assert!(began.elapsed().as_secs() < 10 && error.to_string().contains("in time"), "{error}");
}

#[test]
fn the_span_is_written_in_the_folder_given_and_gone_after() {
    let folder = tempfile::tempdir().unwrap();
    let spans = folder.path().join("derived");
    let program = played(folder.path(), r#"echo '{"start":0.0,"end":0.5,"text":"Oui."}'"#);
    let mut engine = SpeechAnalyzer::at(&program, "fr").unwrap().spans_in(&spans).unwrap();
    engine.hear(&vec![0.25f32; RATE], Some("fr"), None).unwrap();
    let asked = fs::read_to_string(folder.path().join("asked")).unwrap();
    assert!(
        asked.split_whitespace().nth(1).unwrap().starts_with(spans.to_str().unwrap()),
        "{asked}"
    );
    assert_eq!(fs::read_dir(&spans).unwrap().count(), 0, "nothing of the span is left");
}
