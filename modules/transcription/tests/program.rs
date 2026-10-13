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
    fs::write(session.path().join("session.json"), "{}").unwrap();
    for (args, expected) in [
        (
            vec![folder, "--model", "/nowhere/a-model.bin"],
            "the model is not a file that can be read",
        ),
        (vec![folder, "--model", model], "the model could not be loaded"),
        (vec![folder, "--model", model, "--language", "Français"], "the language is a code"),
    ] {
        let (ok, out, err) = run(&args);
        assert!(
            !ok && out.is_empty() && err.contains(expected) && err.trim().lines().count() == 1,
            "{args:?}: {err}"
        );
    }
    assert!(
        !session.path().join("transcript.jsonl").exists(),
        "a transcript was started with nothing to hear it"
    );
}
