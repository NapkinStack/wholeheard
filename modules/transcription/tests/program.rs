//! The program as a shell starts it, where it needs no model: what it refuses, and how it
//! says so. Hearing a real model is the decider's trial, not this file's.

use std::{fs, process::Command};

fn run(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .args(args)
        // Its logs in the tests' own folder, never in the user's.
        .env("WHOLEHEARD_LOG_DIR", std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("logs"))
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn what_cannot_be_heard_is_refused_in_one_sentence_before_anything_is_written() {
    let session = tempfile::tempdir().unwrap();
    let folder = session.path().to_str().unwrap();
    let model = session.path().join("a-model.bin");
    fs::write(&model, "not a model").unwrap();
    let model = model.to_str().unwrap();

    for (args, expected) in [
        (vec![folder, "--model", model], "is not a session"),
        (vec![folder], "usage: wholeheard-transcription"),
        (vec![folder, "--model", model, "--language"], "usage: wholeheard-transcription"),
    ] {
        let (ok, out, err) = run(&args);
        assert!(
            !ok && out.is_empty() && err.contains(expected) && err.trim().lines().count() == 1,
            "{args:?}: {err}"
        );
    }
    // A session as `capture` leaves it, with no sound yet: what is wrong now is the model, or the language.
    let empty = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff";
    fs::write(session.path().join("session.json"), r#"{"contract":"session-audio/v1"}"#).unwrap();
    fs::write(session.path().join("me.wav"), empty).unwrap();
    fs::write(session.path().join("others.wav"), empty).unwrap();
    for (args, expected) in [
        (
            vec![folder, "--model", "/nowhere/a-model.bin"],
            "the model is not a file that can be read",
        ),
        (vec![folder, "--model", model], "the model could not be loaded"),
        (vec![folder, "--model", model, "--language", "Français"], "the language is a code"),
        // A code the engine does not know, and its own word for "detect it", are refused too.
        (vec![folder, "--model", model, "--language", "zz"], "the language is a code"),
        (vec![folder, "--model", model, "--language", "FR"], "the language is a code"),
        (vec![folder, "--model", model, "--language", "french"], "the language is a code"),
        (vec![folder, "--model", model, "--language", "auto"], "leave it out"),
    ] {
        let (ok, out, err) = run(&args);
        assert!(
            !ok && out.is_empty() && err.contains(expected) && err.trim().lines().count() == 1,
            "{args:?}: {err}"
        );
    }
    // A session that cannot be heard is refused before the model is looked at: nothing is
    // said to be heard, whatever the model.
    for (record, sound, expected) in [
        (
            r#"{"contract":"session-audio/v0"}"#,
            &empty[..],
            "session.json names neither session-audio/v1 nor session-audio/v2",
        ),
        (
            r#"{"contract":"session-audio/v1"}"#,
            &b"not sound"[..],
            "me.wav is not an audio file of a session",
        ),
    ] {
        fs::write(session.path().join("session.json"), record).unwrap();
        fs::write(session.path().join("me.wav"), sound).unwrap();
        let (ok, out, err) = run(&[folder, "--model", "/nowhere/a-model.bin", "--language", "fr"]);
        assert!(
            !ok && out.is_empty() && err.contains(expected) && err.trim().lines().count() == 1,
            "{err}"
        );
    }
    assert!(
        !session.path().join("transcript.jsonl").exists(),
        "a transcript was started with nothing to hear it"
    );
}

#[test]
fn a_refusal_is_logged_with_its_detail_and_never_the_sessions_path() {
    let session = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    fs::write(session.path().join("session.json"), r#"{"contract":"session-audio/v0"}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .args([session.path().to_str().unwrap(), "--model", "/nowhere/a-model.bin"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .env("WHOLEHEARD_TRACE", "making-7")
        // The lowest level a user may ask: a failure is logged, with its id.
        .env("WHOLEHEARD_LOG", "error")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let files: Vec<_> = fs::read_dir(logs.path()).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("transcription.") && name.ends_with(".jsonl"), "{name}");
    let text = fs::read_to_string(&files[0]).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    let steps: Vec<_> = lines.iter().map(|line| line["message"].as_str().unwrap()).collect();
    assert_eq!(steps, ["transcription.failed"]);
    assert!(
        lines
            .iter()
            .all(|line| line["span"]["trace"] == "making-7" && line["span"]["program"] == "transcription")
    );
    assert!(
        lines[0]["detail"].as_str().unwrap().contains("names neither session-audio/v1"),
        "{text}"
    );
    assert!(!text.contains(&*session.path().to_string_lossy()), "{text}");
}

#[test]
fn a_session_read_then_refused_logs_nothing_of_its_context_and_an_odd_id_is_replaced() {
    let session = tempfile::tempdir().unwrap();
    let logs = tempfile::tempdir().unwrap();
    let empty = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff";
    fs::write(session.path().join("session.json"), r#"{"contract":"session-audio/v1"}"#).unwrap();
    fs::write(session.path().join("me.wav"), empty).unwrap();
    fs::write(session.path().join("others.wav"), empty).unwrap();
    fs::write(session.path().join("context.txt"), "Le projet Hirondelle, avec Bartholomew.\n").unwrap();
    // Read whole, context included, then refused on the model — given as the session's own folder.
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .args([session.path().to_str().unwrap(), "--model", session.path().to_str().unwrap()])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .env("WHOLEHEARD_TRACE", "not\nan id")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let file = fs::read_dir(logs.path()).unwrap().next().unwrap().unwrap().path();
    let text = fs::read_to_string(file).unwrap();
    let lines: Vec<serde_json::Value> = text.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(lines.last().unwrap()["message"], "transcription.failed", "{text}");
    let trace = lines[0]["span"]["trace"].as_str().unwrap();
    assert!(!trace.contains("not") && !trace.is_empty(), "{trace}");
    for said in ["Hirondelle", "Bartholomew", &*session.path().to_string_lossy()] {
        assert!(!text.contains(said), "the log holds {said:?}: {text}");
    }
}

/// A log folder whose day file cannot take a byte: every write to it fails.
fn full_logs(program: &str) -> tempfile::TempDir {
    let logs = tempfile::tempdir().unwrap();
    let today = std::process::Command::new("date").args(["-u", "+%F"]).output().unwrap();
    let today = String::from_utf8(today.stdout).unwrap();
    std::os::unix::fs::symlink("/dev/full", logs.path().join(format!("{program}.{}.jsonl", today.trim()))).unwrap();
    logs
}

#[test]
fn a_log_that_cannot_be_written_says_nothing_on_the_error_stream() {
    let session = tempfile::tempdir().unwrap();
    let logs = full_logs("transcription");
    let output = Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .args([session.path().to_str().unwrap(), "--model", "/nowhere/a-model.bin"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(err.starts_with("wholeheard-transcription: this folder is not a session"), "{err}");
}
