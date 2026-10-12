//! The program as a user's shell starts it, against a provider played here on this machine:
//! no online service, no key of anyone, no meeting of anyone.

use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    path::Path,
    process::{Command, Output},
    sync::mpsc::{self, Receiver},
    thread,
};

const TRANSCRIPT: &str = concat!(
    r#"{"type": "header", "contract": "transcript/v1", "engine": "an-engine", "model": "a-model", "language": "fr", "context": null}"#,
    "\n",
    r#"{"type": "segment", "speaker": "others", "start": 0.0, "end": 3.2, "text": "On confirme la date du 3 juin ?"}"#,
    "\n",
);
const MINUTES: &str = "## Points forts\n\nLa date est fixée au 3 juin.\n\n## Actions\n\n- Aucune.\n\n## Points clés\n\n- Le 3 juin.\n";

/// A provider that answers one request with `status` and `body`, and hands back what it was
/// sent: the request's head and its body.
fn provider(status: &'static str, body: String) -> (String, Receiver<(String, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sent, received) = mpsc::channel();
    thread::spawn(move || {
        let Ok((stream, _)) = listener.accept() else { return };
        let mut reader = BufReader::new(stream);
        let (mut head, mut length) = (String::new(), 0);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
            head.push_str(&line);
        }
        let mut asked = vec![0; length];
        reader.read_exact(&mut asked).unwrap();
        let _ = sent.send((head, String::from_utf8_lossy(&asked).into_owned()));
        let answer = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = reader.get_mut().write_all(answer.as_bytes());
    });
    (address, received)
}

fn answering(text: &str, finish: &str) -> String {
    serde_json::json!({ "choices": [{ "message": { "role": "assistant", "content": text }, "finish_reason": finish }] }).to_string()
}

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
            "does not know the model a-model",
        ),
        ("500 Internal Server Error", "oops".to_string(), "failed (HTTP 500)"),
        ("200 OK", cut, "cut its answer short"),
        ("200 OK", not_minutes, "is not minutes"),
        ("200 OK", r#"{"choices": []}"#.to_string(), "answered without any text"),
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
fn the_minutes_name_the_host_that_answered_and_never_more() {
    use summit_summary::provider::Provider;
    let host = |address| {
        Provider {
            address,
            model: "m",
            key: None,
            patience: std::time::Duration::from_secs(1),
        }
        .host()
    };
    assert_eq!(host("http://localhost:11434/v1"), "local");
    assert_eq!(host("http://127.0.0.1:8080"), "local");
    assert_eq!(host("http://[::1]:8080/v1"), "local");
    assert_eq!(host("https://openrouter.ai/api/v1"), "openrouter.ai");
    assert_eq!(
        host("https://someone:a-secret@api.example.com:8443/v1?key=another-secret"),
        "api.example.com"
    );
}
