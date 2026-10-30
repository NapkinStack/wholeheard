//! The program as a user's shell starts it, against a provider played here on this machine:
//! no online service, no key of anyone, no meeting of anyone.

mod common;

use std::{fs, path::Path, process::Output};

use common::{MINUTES, answering, program, provider, providing};

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
    let mut command = program();
    command
        .arg(folder)
        .args(["--provider", address, "--model", "a-model"])
        .env_remove("WHOLEHEARD_PROVIDER_KEY");
    if let Some(key) = key {
        command.env("WHOLEHEARD_PROVIDER_KEY", key);
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
    assert!(minutes.starts_with("---\ncontract: minutes/v2\nmade: "), "{minutes}");
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
    assert!(!out.contains("sending"), "something was said to be sent with no transcript: {out}");
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
        let mut command = program();
        command
            .arg(folder.path())
            .args(["--provider", &address, "--model", model])
            .env_remove("WHOLEHEARD_PROVIDER_KEY");
        if let Some(key) = key {
            command.env("WHOLEHEARD_PROVIDER_KEY", key);
        }
        let output = command.output().unwrap();
        let (out, err) = said(&output);
        assert!(
            !output.status.success() && err.contains(expected) && err.trim().lines().count() == 1,
            "{err}"
        );
        assert!(!out.contains("sending"), "something was said to be sent: {out}");
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
    let output = program()
        .arg(folder.path())
        .args(["--provider", &address, "--model", "a-model"])
        .env_remove("WHOLEHEARD_PROVIDER_KEY")
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
fn a_key_spelled_through_a_titles_marks_is_never_written() {
    for above in ["# a-key-**for**-the-test", "# a-key-for&#45;the-test", "# a-key\\-for-the-test"] {
        let folder = session(None);
        let (address, _received) = provider("200 OK", answering(&format!("{above}\n\n{MINUTES}"), "stop"));
        let output = run(folder.path(), &address, Some("a-key-for-the-test"));
        let (_, err) = said(&output);
        assert!(!output.status.success() && err.contains("sent the key back"), "{above}: {err}");
        assert!(!folder.path().join("minutes.md").exists(), "{above}");
    }
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
        let mut command = program();
        command
            .arg(folder.path())
            .args(["--provider", &address, "--model"])
            .env_remove("WHOLEHEARD_PROVIDER_KEY");
        command
    };
    let refused = |output: Output, expected: &str| {
        let (out, err) = said(&output);
        assert!(
            !output.status.success() && err.contains(expected) && err.trim().lines().count() == 1,
            "{expected}: {err}"
        );
        assert!(!out.contains("sending"), "something was said to be sent: {out}");
    };
    refused(program().arg(not_text).output().unwrap(), "an argument is not text");
    refused(
        program().arg("a-model").env("WHOLEHEARD_PROVIDER_KEY", not_text).output().unwrap(),
        "WHOLEHEARD_PROVIDER_KEY is not text",
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
            program().arg("a-model").env("WHOLEHEARD_PROVIDER_KEY", key).output().unwrap(),
            "the key holds a space",
        );
    }
    assert!(received.try_recv().is_err(), "a request left the machine");
}

/// The program's test of a provider, as the settings' *Tester* runs it: no session.
fn tested(address: &str, key: Option<&str>) -> Output {
    let mut command = program();
    command
        .args(["--test", "--provider", address, "--model", "a-model"])
        .env_remove("WHOLEHEARD_PROVIDER_KEY");
    if let Some(key) = key {
        command.env("WHOLEHEARD_PROVIDER_KEY", key);
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
        serde_json::json!([{"role": "user", "content": wholeheard_summary::TEST}]).to_string()
    );
    let lines: Vec<serde_json::Value> = out.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert_eq!(lines[0]["event"], "sending");
    assert_eq!(lines[0]["characters"], wholeheard_summary::TEST.chars().count());
    assert_eq!(lines[1]["event"], "answered");
    assert_eq!(lines[1]["model"], "a-model");
    // The model named first is the same test.
    let (address, received) = provider("200 OK", answering("OK", "stop"));
    let output = program()
        .args(["--test", "--model", "a-model", "--provider", &address])
        .env_remove("WHOLEHEARD_PROVIDER_KEY")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", said(&output).1);
    let (_, asked) = received.recv().unwrap();
    assert!(asked.contains(wholeheard_summary::TEST), "{asked}");
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

/// What one run left in its own log folder: one file, its lines read as JSON.
fn logged(folder: &Path) -> Vec<serde_json::Value> {
    let files: Vec<_> = fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().path()).collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("summary.") && name.ends_with(".jsonl"), "{name}");
    let text = fs::read_to_string(&files[0]).unwrap();
    text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

fn steps(lines: &[serde_json::Value]) -> Vec<&str> {
    lines.iter().map(|line| line["message"].as_str().unwrap()).collect()
}

#[test]
fn the_log_says_each_step_under_the_makings_id_and_nothing_of_the_meeting() {
    let folder = session(Some("Quatre personnes, réunion d'organisation.\n"));
    let logs = tempfile::tempdir().unwrap();
    let (address, _received) = provider("200 OK", answering(MINUTES, "stop"));
    let output = program()
        .arg(folder.path())
        .args(["--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_PROVIDER_KEY", "a-key-for-the-test")
        .env("WHOLEHEARD_TRACE", "making-42")
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .env_remove("WHOLEHEARD_LOG")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", said(&output).1);

    let lines = logged(logs.path());
    assert_eq!(
        steps(&lines),
        [
            "summary.started",
            "summary.read",
            "summary.sending",
            "summary.answered",
            "summary.written"
        ]
    );
    for line in &lines {
        assert_eq!(line["span"]["trace"], "making-42", "{line}");
        assert_eq!(line["span"]["program"], "summary", "{line}");
        assert_eq!(line["level"], "INFO", "{line}");
    }
    assert_eq!(lines[2]["host"], "local");
    assert_eq!(lines[2]["key"], true);
    let text = fs::read_to_string(fs::read_dir(logs.path()).unwrap().next().unwrap().unwrap().path()).unwrap();
    let whole = folder.path().to_string_lossy();
    for said in [
        "3 juin",
        "Quatre personnes",
        "a-key-for-the-test",
        "Points forts",
        "others",
        &*whole,
    ] {
        assert!(!text.contains(said), "the log holds {said:?}: {text}");
    }
}

#[test]
fn a_failure_is_logged_with_its_detail() {
    let folder = session(None);
    let logs = tempfile::tempdir().unwrap();
    let (address, _received) = provider("401 Unauthorized", r#"{"error": "no"}"#.to_string());
    let output = program()
        .arg(folder.path())
        .args(["--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let lines = logged(logs.path());
    let failed = lines.last().unwrap();
    assert_eq!(failed["message"], "summary.failed");
    assert_eq!(failed["level"], "ERROR");
    assert!(
        failed["detail"].as_str().unwrap().contains("refused the key (HTTP 401)"),
        "{failed}"
    );
    // Without an id from the shell, the run has one of its own.
    assert!(!failed["span"]["trace"].as_str().unwrap().is_empty());
}

#[test]
fn a_log_that_cannot_be_written_loses_nothing() {
    let folder = session(None);
    let not_a_folder = tempfile::NamedTempFile::new().unwrap();
    let (address, _received) = provider("200 OK", answering(MINUTES, "stop"));
    let output = program()
        .arg(folder.path())
        .args(["--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", not_a_folder.path())
        .output()
        .unwrap();
    let (out, err) = said(&output);
    assert!(output.status.success() && err.is_empty(), "{err}");
    assert!(out.contains("\"written\"") && folder.path().join("minutes.md").exists());
}

#[test]
fn a_level_asked_shows_more_and_a_library_says_nothing() {
    let logs = tempfile::tempdir().unwrap();
    let (address, _received) = provider("200 OK", answering("Bonjour.", "stop"));
    let output = program()
        .args(["--test", "--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .env("WHOLEHEARD_LOG", "debug")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", said(&output).1);
    let lines = logged(logs.path());
    assert_eq!(steps(&lines), ["summary.testing", "summary.answered"]);
}

#[test]
fn a_failure_says_nothing_of_the_meeting_and_keeps_its_id_at_any_level() {
    let logs = tempfile::tempdir().unwrap();
    let odd = tempfile::tempdir().unwrap();
    // A speaker outside the contract, holding words of the meeting.
    fs::write(
        odd.path().join("transcript.jsonl"),
        TRANSCRIPT.replace(r#""speaker": "others""#, r#""speaker": "On parle de zebracolor demain""#),
    )
    .unwrap();
    let empty = tempfile::tempdir().unwrap();
    for folder in [odd.path(), empty.path()] {
        let output = program()
            .arg(folder)
            .args(["--provider", "http://127.0.0.1:9/v1", "--model", "a-model"])
            .env("WHOLEHEARD_LOG_DIR", logs.path())
            .env("WHOLEHEARD_LOG", "error")
            .env("WHOLEHEARD_TRACE", "making-43")
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
    let lines = logged(logs.path());
    assert_eq!(steps(&lines), ["summary.failed", "summary.failed"]);
    assert!(
        lines
            .iter()
            .all(|line| line["span"]["trace"] == "making-43" && line["span"]["program"] == "summary")
    );
    let text = fs::read_to_string(fs::read_dir(logs.path()).unwrap().next().unwrap().unwrap().path()).unwrap();
    for said in [
        "zebracolor",
        "3 juin",
        &*odd.path().to_string_lossy(),
        &*empty.path().to_string_lossy(),
    ] {
        assert!(!text.contains(said), "the log holds {said:?}: {text}");
    }
}

#[test]
fn a_run_removes_the_day_files_of_more_than_fourteen_days() {
    let logs = tempfile::tempdir().unwrap();
    let old = fs::File::create(logs.path().join("shell.2026-01-01.jsonl")).unwrap();
    old.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(15 * 24 * 3600))
        .unwrap();
    let (address, _received) = provider("200 OK", answering("Bonjour.", "stop"));
    let output = program()
        .args(["--test", "--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!logs.path().join("shell.2026-01-01.jsonl").exists());
    assert_eq!(logged(logs.path()).len(), 2);
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
    let folder = session(None);
    let logs = full_logs("summary");
    let (address, _received) = provider("401 Unauthorized", r#"{"error": "no"}"#.to_string());
    let output = program()
        .arg(folder.path())
        .args(["--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .output()
        .unwrap();
    let (_, err) = said(&output);
    assert!(!output.status.success());
    assert_eq!(err.lines().collect::<Vec<_>>().len(), 1, "{err}");
    assert!(err.starts_with("wholeheard-summary: ") && err.contains("refused the key"), "{err}");
}

#[test]
fn a_log_turned_off_leaves_nothing() {
    let logs = tempfile::tempdir().unwrap();
    let (address, _received) = provider("200 OK", answering("Bonjour.", "stop"));
    let output = program()
        .args(["--test", "--provider", &address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", logs.path())
        .env("WHOLEHEARD_LOG", "off")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_dir(logs.path()).unwrap().count(), 0);
}

/// The last line of the output, as JSON.
fn last(output: &Output) -> serde_json::Value {
    let out = said(output).0;
    serde_json::from_str(out.lines().last().unwrap_or("null")).unwrap()
}

#[test]
fn a_failure_a_person_acts_on_differently_ends_the_output_with_its_code() {
    let coded = |output: &Output| {
        let line = last(output);
        let (out, err) = said(output);
        assert!(!output.status.success() && err.trim().lines().count() == 1, "{err}");
        assert_eq!(line["event"], "failed", "{out}");
        assert_eq!(out.matches(r#""failed""#).count(), 1, "said once: {out}");
        line["code"].as_str().unwrap().to_string()
    };
    // No cause a person acts on differently: no failed line.
    let uncoded = |output: &Output| {
        assert!(!output.status.success() && !said(output).0.contains("failed"), "{}", said(output).0);
    };
    // The provider's answers, each with its gesture.
    for (status, body, code) in [
        ("401 Unauthorized", "{}".to_string(), "provider.refused_key"),
        ("403 Forbidden", "{}".to_string(), "provider.refused_key"),
        ("402 Payment Required", "{}".to_string(), "provider.no_credit"),
        ("404 Not Found", "{}".to_string(), "provider.unknown_model"),
        ("500 Internal Server Error", "oops".to_string(), "provider.failed"),
        ("300 Multiple Choices", String::new(), "provider.redirected"),
        // 304 and 305 send nowhere one can follow (RFC 9110 §15.4): another refusal.
        ("304 Not Modified", String::new(), "provider.failed"),
        ("305 Use Proxy", String::new(), "provider.failed"),
        (
            "200 OK",
            r###"{"choices": [{"message": {"content": "## A"###.to_string(),
            "provider.failed",
        ),
        ("200 OK", answering(MINUTES, "content_filter"), "provider.failed"),
        ("200 OK", answering(MINUTES, "length"), "answer.cut_short"),
        (
            "302 Found\r\nLocation: http://127.0.0.1:9/elsewhere",
            String::new(),
            "provider.redirected",
        ),
        ("200 OK", r#"{"choices": []}"#.to_string(), "provider.failed"),
        (
            "200 OK",
            answering("Bien sûr ! Voici le compte rendu.", "stop"),
            "answer.not_minutes",
        ),
    ] {
        let folder = session(None);
        let (address, _received) = provider(status, body.clone());
        assert_eq!(coded(&run(folder.path(), &address, None)), code, "{status} {body}");
        // A test meets the provider's causes the same way.
        if !status.starts_with("200") {
            let (address, _received) = provider(status, body);
            assert_eq!(coded(&tested(&address, None)), code, "test: {status}");
        }
    }
    let folder = session(None);
    assert_eq!(coded(&run(folder.path(), "http://127.0.0.1:9/v1", None)), "provider.unreachable");
    // The session's own causes, before anything leaves.
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(coded(&run(empty.path(), "http://127.0.0.1:9/v1", None)), "transcript.unusable");
    let silent = tempfile::tempdir().unwrap();
    fs::write(
        silent.path().join("transcript.jsonl"),
        TRANSCRIPT.lines().next().unwrap().to_string() + "\n",
    )
    .unwrap();
    assert_eq!(coded(&run(silent.path(), "http://127.0.0.1:9/v1", None)), "transcript.no_speech");
    // Settings that cannot be used as given.
    let folder = session(None);
    assert_eq!(coded(&run(folder.path(), "ftp://example.org/v1", None)), "settings.invalid");
    assert_eq!(coded(&run(folder.path(), "http://local/v1", None)), "settings.invalid");
    // A test meets them before anything leaves, too.
    let output = tested("ftp://example.org/v1", None);
    assert!(!said(&output).0.contains("sending"), "{}", said(&output).0);
    assert_eq!(coded(&output), "settings.invalid");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let output = program()
            .arg(folder.path())
            .args(["--provider", "http://127.0.0.1:9/v1", "--model", "a-model"])
            .env("WHOLEHEARD_PROVIDER_KEY", std::ffi::OsStr::from_bytes(&[0xff, 0xfe]))
            .output()
            .unwrap();
        assert_eq!(coded(&output), "settings.invalid");
    }
    assert_eq!(
        coded(&run(folder.path(), "http://127.0.0.1:9/v1", Some("a key\nwith a line"))),
        "settings.invalid"
    );
    // Minutes that cannot be written where they go.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let folder = session(None);
        let (address, _received) = provider("200 OK", answering(MINUTES, "stop"));
        fs::set_permissions(folder.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let output = run(folder.path(), &address, None);
        fs::set_permissions(folder.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(coded(&output), "minutes.unwritable");
    }
    // A cause no person acts on differently is said by its sentence alone: no failed line.
    uncoded(&program().arg("--provider").output().unwrap());
    let folder = session(Some("context"));
    fs::write(folder.path().join("context.txt"), [0xff, 0xfe]).unwrap();
    uncoded(&run(folder.path(), "http://127.0.0.1:9/v1", None));
    let folder = session(None);
    let (address, _received) = provider("200 OK", answering(&format!("{MINUTES}\n- Bearer a-key-for-the-test\n"), "stop"));
    uncoded(&run(folder.path(), &address, Some("a-key-for-the-test")));
}

#[test]
fn an_answer_that_is_not_minutes_is_asked_for_once_more_and_only_that_one() {
    // Not minutes, then minutes: written, from the second answer, the same request sent twice.
    let folder = session(None);
    let (address, received) = providing(vec![
        (
            "200 OK",
            answering("## Réunion\n\nUn titre et deux sections.\n\n## Actions\n\n- Aucune.\n", "stop"),
        ),
        ("200 OK", answering(MINUTES, "stop")),
    ]);
    let output = run(folder.path(), &address, None);
    let (out, err) = said(&output);
    assert!(output.status.success(), "{err}");
    assert!(fs::read_to_string(folder.path().join("minutes.md")).unwrap().ends_with(MINUTES));
    let (first, second) = (received.recv().unwrap().1, received.recv().unwrap().1);
    assert_eq!(first, second);
    assert!(received.try_recv().is_err(), "asked a third time");
    // What leaves is said once, before anything leaves.
    assert_eq!(out.matches(r#""event":"sending""#).count(), 1, "{out}");
    // Not minutes twice: failed as before, after two requests, nothing written.
    let folder = session(None);
    let (address, received) = provider("200 OK", answering("Bien sûr ! Voici le compte rendu.", "stop"));
    let output = run(folder.path(), &address, None);
    let (out, err) = said(&output);
    assert!(
        !output.status.success() && err.contains("is not minutes") && out.contains(r#""code":"answer.not_minutes""#),
        "{err}"
    );
    assert!(received.recv().is_ok() && received.recv().is_ok() && received.try_recv().is_err());
    assert!(!folder.path().join("minutes.md").exists());
    // Any other failure is not asked again: one request.
    for (status, body) in [
        ("200 OK", answering(MINUTES, "length")),
        ("500 Internal Server Error", "oops".to_string()),
    ] {
        let folder = session(None);
        let (address, received) = provider(status, body);
        assert!(!run(folder.path(), &address, None).status.success());
        assert!(received.recv().is_ok() && received.try_recv().is_err(), "{status}: asked again");
    }
}
