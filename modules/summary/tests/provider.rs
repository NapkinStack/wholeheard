//! The one connection of the module, tried against a provider played on this machine: what is
//! sent, what each failure is called, and that the host named is the host contacted.

mod common;

use std::{
    io::{BufRead, BufReader, Read, Write},
    time::Duration,
};

use common::{MINUTES, answering, answering_natively, ollama, played, provider};
use wholeheard_summary::{
    Refused,
    provider::{Provider, patience},
};

fn asking(address: &str, key: Option<&str>) -> Result<String, Refused> {
    Provider::new(address, "a-model", key, Duration::from_secs(5))?.ask("the request")
}

#[test]
fn one_request_is_sent_with_the_model_the_request_and_the_key_as_a_bearer_token() {
    let (address, received) = provider("200 OK", answering(MINUTES, "stop"));
    assert_eq!(asking(&address, Some("a-key-for-the-test")).unwrap(), MINUTES);
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
    assert!(head.to_lowercase().contains("authorization: bearer a-key-for-the-test"), "{head}");
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (body["model"].as_str(), body["messages"][0]["content"].as_str()),
        (Some("a-model"), Some("the request"))
    );
    assert!(!body.to_string().contains("a-key-for-the-test"));

    // No key, or an empty one, sends no such line; an answer that does not say how it ended is whole.
    let (address, received) = provider("200 OK", r#"{"choices": [{"message": {"content": "text"}}]}"#.to_string());
    assert_eq!(asking(&address, Some("")).unwrap(), "text");
    assert!(!received.recv().unwrap().0.to_lowercase().contains("authorization"));
}

#[test]
fn each_failure_of_a_provider_has_its_own_sentence() {
    for (status, body, expected) in [
        ("401 Unauthorized", r#"{"error": "no"}"#.to_string(), "refused the key (HTTP 401)"),
        ("404 Not Found", "{}".to_string(), "does not know this model or this address"),
        (
            "402 Payment Required",
            r#"{"error": {"message": "Insufficient credits"}}"#.to_string(),
            "refused for lack of credit (HTTP 402): add credit, or raise the key's limit, with this provider",
        ),
        ("500 Internal Server Error", "oops".to_string(), "failed (HTTP 500)"),
        ("200 OK", answering(MINUTES, "length"), "cut its answer short"),
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
        ("200 OK", "<html>".to_string(), "could not be read to its end"),
        (
            "200 OK",
            r#"{"choices": [{"message": {"content": 42}, "finish_reason": "stop"}]}"#.to_string(),
            "answered without any text",
        ),
        ("200 OK", r#"{"choices": []}"#.to_string(), "answered without any text"),
        (
            "200 OK",
            r#"{"choices": [{"message": {"content": "text"}, "finish_reason": 7}]}"#.to_string(),
            "stopped before the end of its answer",
        ),
        ("304 Not Modified", String::new(), "failed (HTTP 304)"),
    ] {
        let (address, _received) = provider(status, body);
        let Err(Refused(why, _)) = asking(&address, None) else {
            panic!("{status}: accepted")
        };
        assert!(
            why.contains(expected) && why.contains("the provider on this computer") && why.lines().count() == 1,
            "{status}: {why}"
        );
    }
    let Err(Refused(why, _)) = asking("http://127.0.0.1:9/v1", None) else {
        panic!("accepted")
    };
    assert!(why.contains("could not be reached"), "{why}");
}

#[test]
fn a_redirection_is_refused_and_the_other_host_hears_nothing() {
    for status in [
        "301 Moved Permanently",
        "302 Found",
        "307 Temporary Redirect",
        "308 Permanent Redirect",
    ] {
        let (elsewhere, heard_elsewhere) = provider("200 OK", answering(MINUTES, "stop"));
        let moved: &'static str = Box::leak(format!("{status}\r\nLocation: {elsewhere}/chat/completions").into_boxed_str());
        let (address, received) = provider(moved, String::new());
        let Err(Refused(why, _)) = asking(&address, Some("a-key-for-the-test")) else {
            panic!("{status}: followed")
        };
        assert!(why.contains("was not followed"), "{why}");
        assert!(
            received.recv().is_ok() && heard_elsewhere.try_recv().is_err(),
            "{status}: the other host was contacted"
        );
    }
}

#[test]
fn the_host_named_is_the_host_the_request_goes_to() {
    let host = |address: &str| Provider::new(address, "m", None, Duration::from_secs(1)).map(|provider| provider.host());
    // This machine: its name, sent to its address so that no resolver says otherwise, and its addresses.
    for here in [
        "http://localhost:11434/v1",
        "http://LOCALHOST:1/v1",
        "http://127.0.0.1:8080",
        "http://127.0.0.2",
        "http://[::1]:8080/v1",
        "http://[::ffff:127.0.0.1]:1",
        "http://0.0.0.0:1",
    ] {
        assert_eq!(host(here).unwrap(), "local", "{here}");
    }
    assert!(
        format!(
            "{:?}",
            Provider::new("http://LocalHost:1/v1", "m", None, Duration::from_secs(1)).unwrap()
        )
        .contains("//127.0.0.1:1/")
    );
    assert_eq!(host("https://OpenRouter.ai/api/v1/").unwrap(), "openrouter.ai");
    assert_eq!(host("http://10.1.2.3:8080/v1").unwrap(), "10.1.2.3");
    // What could name one host and reach another, carry a secret, or be read as a number in
    // another base by the system, is no address.
    let refused = "http://127.0.0.2:18084#@localhost/v1 https://someone:a-secret@api.example.com/v1 https://api.example.com/v1?key=another-secret ftp://api.example.com/v1 api.example.com/v1 http:// http://127.0.0.1.:8080/v1 http://localhost.:8080/v1 http://local:8080/v1
        http://.:8080/v1 http://127.1:8080 http://2130706433 http://0x7f000001 http://010.1.2.3:8080 http://[::1%lo]:8080 http://api.example.com:65536/v1 http://api.example.com:8080x/v1 http://api.example.com:+8080/v1 http://[127.0.0.1]:8080/v1 http://[0.0.0.0]:1 http://[10.1.2.3]:1 http://[localhost]:1 http://[example.test]:1";
    for refused in refused.split_whitespace().chain(["", "http://api.example.com/v1 --model x"]) {
        assert!(host(refused).is_err(), "accepted: {refused} as {:?}", host(refused));
    }
}

#[test]
fn a_provider_shown_to_whoever_debugs_never_shows_its_key() {
    let provider = Provider::new("http://127.0.0.1:1/v1", "m", Some("a-key-for-the-test"), Duration::from_secs(1)).unwrap();
    assert!(!format!("{provider:?}").contains("a-key-for-the-test"));
    for (model, key) in [("  ", None), ("m", Some("   "))] {
        assert!(Provider::new("http://127.0.0.1:1/v1", model, key, Duration::from_secs(1)).is_err());
    }
}

#[test]
fn ollama_is_asked_on_its_own_route_with_room_for_the_whole_request() {
    let request = "é".repeat(50_000);
    let (address, received) = ollama(vec![("200 OK", answering_natively(MINUTES, "stop"))]);
    let answer = Provider::new(&address, "a-model", None, Duration::from_secs(5))
        .unwrap()
        .ask(&request);
    assert_eq!(answer.unwrap(), MINUTES);
    assert!(
        received
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .0
            .starts_with("GET /api/version ")
    );
    let (head, body) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(head.starts_with("POST /api/chat "), "{head}");
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (
            body["model"].as_str(),
            body["stream"].as_bool(),
            body["messages"][0]["content"].as_str()
        ),
        (Some("a-model"), Some(false), Some(request.as_str()))
    );
    // Two bytes to a token at the very least, and room to answer beyond them, in whole steps.
    assert_eq!(body["options"]["num_ctx"], 57_344, "{}", body["options"]);
    assert_eq!(body["options"]["num_predict"], 4096, "{}", body["options"]);
}

#[test]
fn ollama_room_grows_with_the_request_and_a_short_one_still_can_answer() {
    for (bytes, room) in [(0, 4096), (10, 8192), (8192, 8192), (8194, 12_288), (16_384, 12_288)] {
        let (address, received) = ollama(vec![("200 OK", answering_natively("text", "stop"))]);
        Provider::new(&address, "m", None, Duration::from_secs(5))
            .unwrap()
            .ask(&"a".repeat(bytes))
            .unwrap();
        received.recv().unwrap();
        let body: serde_json::Value = serde_json::from_str(&received.recv().unwrap().1).unwrap();
        assert_eq!(body["options"]["num_ctx"], room, "{bytes} bytes");
    }
}

#[test]
fn ollama_failures_are_called_as_any_providers() {
    for (status, body, code) in [
        ("200 OK", answering_natively("half of", "length"), "answer.cut_short"),
        ("200 OK", answering_natively("text", "unload"), "provider.failed"),
        ("200 OK", answering_natively(" ", "stop"), "provider.failed"),
        (
            "404 Not Found",
            r#"{"error":"model not found"}"#.to_string(),
            "provider.unknown_model",
        ),
        (
            "500 Internal Server Error",
            r#"{"error":"model requires more system memory"}"#.to_string(),
            "provider.failed",
        ),
    ] {
        let (address, _received) = ollama(vec![(status, body)]);
        assert_eq!(asking(&address, None).unwrap_err().1, Some(code), "{status}");
    }
}

#[test]
fn a_provider_that_is_not_ollama_is_asked_on_the_openai_route() {
    // It knows nothing of that question (the double answers 404), or answers something else to it.
    let (address, received) = provider("200 OK", answering("text", "stop"));
    assert_eq!(asking(&address, None).unwrap(), "text");
    let (head, body) = received.recv().unwrap();
    assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
    assert!(!body.contains("num_ctx"), "{body}");

    let (address, received) = played(Some(r#"{"name":"another"}"#), vec![("200 OK", answering("text", "stop"))]);
    assert_eq!(asking(&address, None).unwrap(), "text");
    received.recv().unwrap();
    assert!(received.recv().unwrap().0.starts_with("POST /v1/chat/completions "));
}

#[test]
fn a_provider_given_a_key_is_never_asked_what_it_is() {
    let (address, received) = ollama(vec![("200 OK", answering("text", "stop"))]);
    assert_eq!(asking(&address, Some("a-key-for-the-test")).unwrap(), "text");
    assert!(received.recv().unwrap().0.starts_with("POST /v1/chat/completions "));
}

#[test]
fn an_address_not_ending_in_v1_is_never_asked_what_it_is() {
    let (address, received) = ollama(vec![("200 OK", answering("text", "stop"))]);
    assert_eq!(asking(&format!("{address}/other"), None).unwrap(), "text");
    assert!(received.recv().unwrap().0.starts_with("POST /v1/other/chat/completions "));
}

#[test]
fn ollama_is_waited_for_according_to_what_it_has_to_read() {
    assert_eq!(patience(Duration::from_secs(600), ""), Duration::from_secs(600));
    assert_eq!(patience(Duration::from_secs(600), &"a".repeat(20_000)), Duration::from_secs(1600));
}

/// Answers the version question with this status, then whatever it is asked, this much later, as Ollama would.
fn ollama_by_hand(version: &'static str, after: Duration) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        while let Ok((stream, _)) = listener.accept() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream);
                let (mut first, mut length) = (String::new(), 0);
                reader.read_line(&mut first).unwrap();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                }
                let mut asked = vec![0; length];
                reader.read_exact(&mut asked).unwrap();
                let (status, body) = if first.starts_with("GET /api/version ") {
                    (version, r#"{"version":"0.12.0"}"#.to_string())
                } else {
                    std::thread::sleep(after);
                    let body = if first.starts_with("POST /api/chat ") {
                        answering_natively("from its own route", "stop")
                    } else {
                        answering("from the openai route", "stop")
                    };
                    ("200 OK", body)
                };
                let answer = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = reader.get_mut().write_all(answer.as_bytes());
            });
        }
    });
    address
}

#[test]
fn ollama_is_waited_for_beyond_the_patience_given_as_long_as_it_has_to_read() {
    // One second given, five more for these 100 bytes: an answer after two comes.
    let address = ollama_by_hand("200 OK", Duration::from_secs(2));
    let provider = Provider::new(&address, "m", None, Duration::from_secs(1)).unwrap();
    assert_eq!(provider.ask(&"a".repeat(100)).unwrap(), "from its own route");

    // And what is said is what was waited: one given, one for these 20 bytes.
    let address = ollama_by_hand("200 OK", Duration::from_secs(4));
    let provider = Provider::new(&address, "m", None, Duration::from_secs(1)).unwrap();
    let refused = provider.ask(&"a".repeat(20)).unwrap_err();
    assert_eq!(refused.1, Some("provider.too_slow"));
    assert!(refused.0.contains("within 2 s"), "{}", refused.0);
}

#[test]
fn a_failure_with_a_version_in_it_is_not_ollama() {
    let address = ollama_by_hand("500 Internal Server Error", Duration::ZERO);
    assert_eq!(asking(&address, None).unwrap(), "from the openai route");
}

#[test]
fn a_provider_that_is_not_ollama_is_given_the_patience_it_was_given() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let provider = Provider::new(&address, "m", None, Duration::from_secs(1)).unwrap();
    // A question of its version unanswered waits as long as the answer: both under the given patience.
    let began = std::time::Instant::now();
    let refused = provider.ask(&"a".repeat(1_000_000)).unwrap_err();
    assert_eq!(refused.1, Some("provider.too_slow"));
    assert!(began.elapsed() < Duration::from_secs(10), "{:?}", began.elapsed());
    drop(listener);
}
