//! The program as a user's shell starts it, against a provider played here on this machine:
//! no online service, no key of anyone, no meeting of anyone.

mod common;

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use common::{MINUTES, answering, provider};

const TRANSCRIPT: &str = concat!(
    r#"{"type": "header", "contract": "transcript/v1", "engine": "an-engine", "model": "a-model", "language": "fr", "context": null}"#,
    "\n",
    r#"{"type": "segment", "speaker": "others", "start": 0.0, "end": 3.2, "text": "On confirme la date du 3 juin ?"}"#,
    "\n",
);
fn session(context: Option<&str>) -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("transcript.jsonl"), TRANSCRIPT).unwrap();
    if let Some(context) = context {
        fs::write(folder.path().join("context.txt"), context).unwrap();
    }
    folder
}

fn run(folder: &Path, address: &str, key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_summit-summary"));
    command
        .arg(folder)
        .args(["--provider", address, "--model", "a-model"])
        .env_remove("SUMMIT_PROVIDER_KEY");
    if let Some(key) = key {
        command.env("SUMMIT_PROVIDER_KEY", key);
    }
    command.output().unwrap()
}

fn said(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_minutes_are_written_from_the_transcript_the_context_and_the_providers_answer() {
    let folder = session(Some("Quatre personnes, réunion d'organisation.\n"));
    let (address, received) = provider("200 OK", answering(MINUTES, "stop"));
    let output = run(folder.path(), &address, Some("a-key-for-the-test"));
    let (out, err) = said(&output);
    assert!(output.status.success(), "{err}");

    let (head, asked) = received.recv().unwrap();
    assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
    assert!(head.to_lowercase().contains("authorization: bearer a-key-for-the-test"), "{head}");
    let asked: serde_json::Value = serde_json::from_str(&asked).unwrap();
    assert_eq!(asked["model"], "a-model");
    let content = asked["messages"][0]["content"].as_str().unwrap();
    assert!(
        content.contains("Quatre personnes, réunion d'organisation.")
            && content.contains("[00:00] others: On confirme la date du 3 juin ?")
    );

    let minutes = fs::read_to_string(folder.path().join("minutes.md")).unwrap();
    assert!(minutes.starts_with("---\ncontract: minutes/v1\nmade: "), "{minutes}");
    assert!(
        minutes.contains("\nprovider: \"local\"\nmodel: \"a-model\"\ncontext: \"Quatre personnes, réunion d'organisation.\"\n---\n"),
        "{minutes}"
    );
    assert!(minutes.ends_with(MINUTES) && !minutes.contains("a-key-for-the-test"));
    // Said before anything left, then once written; the key is in neither.
    let events: Vec<serde_json::Value> = out.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(
        events.iter().map(|event| event["event"].as_str().unwrap()).collect::<Vec<_>>(),
        ["sending", "written"]
    );
    assert_eq!(events[0]["host"], "local");
    assert!(!out.contains("a-key-for-the-test") && !err.contains("a-key-for-the-test"));
    assert_eq!(fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap(), TRANSCRIPT);
}

#[test]
fn a_provider_that_fails_loses_nothing_and_says_why_in_one_sentence() {
    let cut = answering(MINUTES, "length");
    let not_minutes = answering("Bien sûr ! Voici le compte rendu.", "stop");
    for (status, body, expected) in [
        ("401 Unauthorized", r#"{"error": "no"}"#.to_string(), "refused the key"),
        (
            "404 Not Found",
            r#"{"error": "no such model"}"#.to_string(),
            "does not know this model or this address",
        ),
        ("500 Internal Server Error", "oops".to_string(), "failed (HTTP 500)"),
        ("200 OK", cut, "cut its answer short"),
        ("200 OK", not_minutes, "is not minutes"),
        ("200 OK", r#"{"choices": []}"#.to_string(), "answered without any text"),
        (
            "200 OK",
            answering(MINUTES, "content_filter"),
            "stopped before the end of its answer",
        ),
        (
            "200 OK",
            r###"{"choices": [{"message": {"content": "## A"###.to_string(),
            "could not be read to its end",
        ),
        (
            "200 OK",
            r#"{"choices": [{"message": {"content": 42}, "finish_reason": "stop"}]}"#.to_string(),
            "answered without any text",
        ),
        (
            "302 Found\r\nLocation: http://127.0.0.1:9/elsewhere",
            String::new(),
            "was not followed",
        ),
    ] {
        let folder = session(None);
        fs::write(folder.path().join("minutes.md"), "the minutes that were there\n").unwrap();
        let (address, _received) = provider(status, body);
        let output = run(folder.path(), &address, None);
        let (_, err) = said(&output);
        assert!(!output.status.success(), "{status}: accepted");
        assert!(err.contains(expected) && err.trim().lines().count() == 1, "{status}: {err}");
        // Nothing lost, and nothing that looks like finished minutes left behind.
        assert_eq!(
            fs::read_to_string(folder.path().join("minutes.md")).unwrap(),
            "the minutes that were there\n"
        );
        assert_eq!(fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap(), TRANSCRIPT);
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 2, "{status}: a file was left behind");
    }
}

#[test]
fn a_provider_that_is_not_there_and_a_session_without_a_transcript_are_said() {
    let folder = session(None);
    let output = run(folder.path(), "http://127.0.0.1:9/v1", None);
    assert!(
        !output.status.success() && said(&output).1.contains("could not be reached"),
        "{}",
        said(&output).1
    );
    assert!(!folder.path().join("minutes.md").exists());

    let empty = tempfile::tempdir().unwrap();
    let output = run(empty.path(), "http://127.0.0.1:9/v1", None);
    let (out, err) = said(&output);
    assert!(!output.status.success() && err.contains("has no transcript to read"), "{err}");
    assert!(out.is_empty(), "something was said to be sent with no transcript: {out}");
}

#[test]
fn what_cannot_be_sent_as_it_is_is_refused_before_anything_leaves() {
    let folder = session(None);
    let (address, received) = provider("200 OK", answering(MINUTES, "stop"));
    for (model, key, expected) in [
        ("a\nb", None, "the model's name"),
        ("", None, "the model's name"),
        ("m", Some("a key\nwith a line"), "the key holds a space or a character"),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_summit-summary"));
        command
            .arg(folder.path())
            .args(["--provider", &address, "--model", model])
            .env_remove("SUMMIT_PROVIDER_KEY");
        if let Some(key) = key {
            command.env("SUMMIT_PROVIDER_KEY", key);
        }
        let output = command.output().unwrap();
        let (out, err) = said(&output);
        assert!(
            !output.status.success() && err.contains(expected) && err.trim().lines().count() == 1,
            "{err}"
        );
        assert!(out.is_empty(), "something was said to be sent: {out}");
    }
    fs::write(folder.path().join("context.txt"), [0xff, 0xfe, 0x00]).unwrap();
    let output = run(folder.path(), &address, None);
    assert!(!output.status.success() && said(&output).1.contains("context.txt is not text"));
    assert!(received.try_recv().is_err(), "a request left the machine");
}

#[test]
fn a_proxy_named_by_the_environment_is_never_used() {
    // The second server plays the proxy: it must see nothing, and the provider everything.
    let folder = session(None);
    let (address, received) = provider("200 OK", answering(MINUTES, "stop"));
    let (proxy, seen_by_proxy) = provider("200 OK", String::new());
    let proxy = proxy.trim_end_matches("/v1").to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_summit-summary"))
        .arg(folder.path())
        .args(["--provider", &address, "--model", "a-model"])
        .env_remove("SUMMIT_PROVIDER_KEY")
        .env_remove("NO_PROXY")
        .env_remove("no_proxy")
        .envs([
            ("HTTP_PROXY", &proxy),
            ("http_proxy", &proxy),
            ("ALL_PROXY", &proxy),
            ("all_proxy", &proxy),
            ("HTTPS_PROXY", &proxy),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", said(&output).1);
    assert!(received.recv().is_ok());
    assert!(seen_by_proxy.try_recv().is_err(), "the proxy of the environment was contacted");
}

#[test]
fn a_key_the_provider_sends_back_in_its_answer_is_never_written() {
    let folder = session(None);
    fs::write(folder.path().join("minutes.md"), "the minutes that were there\n").unwrap();
    let (address, _received) = provider("200 OK", answering(&format!("{MINUTES}\n- Bearer a-key-for-the-test\n"), "stop"));
    let output = run(folder.path(), &address, Some("a-key-for-the-test"));
    let (out, err) = said(&output);
    assert!(
        !output.status.success() && err.contains("sent the key back") && err.trim().lines().count() == 1,
        "{err}"
    );
    assert!(!out.contains("a-key-for-the-test") && !err.contains("a-key-for-the-test"));
    assert_eq!(
        fs::read_to_string(folder.path().join("minutes.md")).unwrap(),
        "the minutes that were there\n"
    );
}

#[cfg(unix)]
#[test]
fn what_is_not_text_or_cannot_be_read_is_refused_in_one_sentence_and_nothing_leaves() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let not_text = OsStr::from_bytes(&[0x66, 0xff, 0xfe]);
    let folder = session(None);
    let (address, received) = provider("200 OK", answering(MINUTES, "stop"));
    let program = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_summit-summary"));
        command
            .arg(folder.path())
            .args(["--provider", &address, "--model"])
            .env_remove("SUMMIT_PROVIDER_KEY");
        command
    };
    let refused = |output: Output, expected: &str| {
        let (out, err) = said(&output);
        assert!(
            !output.status.success() && err.contains(expected) && err.trim().lines().count() == 1,
            "{expected}: {err}"
        );
        assert!(out.is_empty(), "something was said to be sent: {out}");
    };
    refused(program().arg(not_text).output().unwrap(), "an argument is not text");
    refused(
        program().arg("a-model").env("SUMMIT_PROVIDER_KEY", not_text).output().unwrap(),
        "SUMMIT_PROVIDER_KEY is not text",
    );
    // A context that is there and cannot be read is not the same as no context.
    fs::create_dir(folder.path().join("context.txt")).unwrap();
    refused(
        program().arg("a-model").output().unwrap(),
        "context.txt is there and cannot be read",
    );
    // Nor one that leads nowhere, nor a key with a space a server would drop before sending it back.
    fs::remove_dir(folder.path().join("context.txt")).unwrap();
    std::os::unix::fs::symlink("nowhere", folder.path().join("context.txt")).unwrap();
    refused(
        program().arg("a-model").output().unwrap(),
        "context.txt is there and cannot be read",
    );
    fs::remove_file(folder.path().join("context.txt")).unwrap();
    for key in ["a-key-for-the-test ", " a-key-for-the-test", "a key"] {
        refused(
            program().arg("a-model").env("SUMMIT_PROVIDER_KEY", key).output().unwrap(),
            "the key holds a space",
        );
    }
    assert!(received.try_recv().is_err(), "a request left the machine");
}

/// The program's test of a provider, as the settings' *Tester* runs it: no session.
fn tested(address: &str, key: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_summit-summary"));
    command
        .args(["--test", "--provider", address, "--model", "a-model"])
        .env_remove("SUMMIT_PROVIDER_KEY");
    if let Some(key) = key {
        command.env("SUMMIT_PROVIDER_KEY", key);
    }
    command.output().unwrap()
}

#[test]
fn a_test_sends_one_fixed_sentence_and_says_the_model_answered() {
    let (address, received) = provider("200 OK", answering("OK", "stop"));
    let output = tested(&address, Some("a-key-for-the-test"));
    let (out, err) = said(&output);
    assert!(output.status.success(), "{err}");
    let (head, asked) = received.recv().unwrap();
    assert!(head.to_lowercase().contains("authorization: bearer a-key-for-the-test"), "{head}");
    let asked: serde_json::Value = serde_json::from_str(&asked).unwrap();
    assert_eq!(asked["model"], "a-model");
    // Nothing of a meeting: one sentence of the program's own.
    assert_eq!(
        asked["messages"].to_string(),
        serde_json::json!([{"role": "user", "content": summit_summary::TEST}]).to_string()
    );
    let lines: Vec<serde_json::Value> = out.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert_eq!(lines[0]["event"], "sending");
    assert_eq!(lines[0]["characters"], summit_summary::TEST.chars().count());
    assert_eq!(lines[1]["event"], "answered");
    assert_eq!(lines[1]["model"], "a-model");
    // The model named first is the same test.
    let (address, received) = provider("200 OK", answering("OK", "stop"));
    let output = Command::new(env!("CARGO_BIN_EXE_summit-summary"))
        .args(["--test", "--model", "a-model", "--provider", &address])
        .env_remove("SUMMIT_PROVIDER_KEY")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", said(&output).1);
    let (_, asked) = received.recv().unwrap();
    assert!(asked.contains(summit_summary::TEST), "{asked}");
}

#[test]
fn a_test_that_fails_says_why_in_one_sentence() {
    for (status, body, expected) in [
        ("401 Unauthorized", r#"{"error": "no"}"#.to_string(), "refused the key"),
        (
            "404 Not Found",
            r#"{"error": "no"}"#.to_string(),
            "does not know this model or this address",
        ),
        ("200 OK", r#"{"choices": []}"#.to_string(), "answered without any text"),
    ] {
        let (address, _received) = provider(status, body);
        let output = tested(&address, None);
        let (_, err) = said(&output);
        assert!(!output.status.success(), "{status}: accepted");
        assert!(err.contains(expected) && err.trim().lines().count() == 1, "{status}: {err}");
    }
    let output = tested("http://127.0.0.1:9/v1", None);
    assert!(!output.status.success() && said(&output).1.trim().lines().count() == 1);
}
