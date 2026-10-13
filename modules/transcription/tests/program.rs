//! The program as a shell starts it, where it needs no model: what it refuses, and how it
//! says so. Hearing a real model is the decider's trial, not this file's.

use std::{fs, process::Command};

fn run(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_summit-transcription"))
        .args(args)
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
        (vec![folder], "usage: summit-transcription"),
        (vec![folder, "--model", model, "--language"], "usage: summit-transcription"),
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
