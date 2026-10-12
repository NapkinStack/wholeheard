//! The one connection of the module, tried against a provider played on this machine: what is
//! sent, what each failure is called, and that the host named is the host contacted.

mod common;

use std::time::Duration;

use common::{MINUTES, answering, provider};
use summit_summary::{Refused, provider::Provider};

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
    ] {
        let (address, _received) = provider(status, body);
        let Err(Refused(why)) = asking(&address, None) else {
            panic!("{status}: accepted")
        };
        assert!(
            why.contains(expected) && why.contains("the provider at local") && why.lines().count() == 1,
            "{status}: {why}"
        );
    }
    let Err(Refused(why)) = asking("http://127.0.0.1:9/v1", None) else {
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
        let Err(Refused(why)) = asking(&address, Some("a-key-for-the-test")) else {
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
fn the_host_named_is_the_host_contacted_and_this_machine_is_local_however_it_is_written() {
    use summit_summary::provider::Provider;
    let host = |address: &str| Provider::new(address, "m", None, std::time::Duration::from_secs(1)).map(|provider| provider.host());
    for here in [
        "http://localhost:11434/v1",
        "http://LOCALHOST:1/v1",
        "http://localhost.:1",
        "http://127.0.0.1:8080",
        "http://127.0.0.2",
        "http://[::1]:8080/v1",
    ] {
        assert_eq!(host(here).unwrap(), "local", "{here}");
    }
    assert_eq!(host("https://OpenRouter.ai/api/v1/").unwrap(), "openrouter.ai");
    assert_eq!(host("http://0.0.0.0:1").unwrap(), "0.0.0.0");
    // An address that could name one host and reach another, or carry a secret, is no address.
    for refused in [
        "http://127.0.0.2:18084#@localhost/v1",
        "https://someone:a-secret@api.example.com/v1",
        "https://api.example.com/v1?key=another-secret",
        "ftp://api.example.com/v1",
        "api.example.com/v1",
        "http://",
        "",
        "http://api.example.com/v1 --model x",
    ] {
        assert!(host(refused).is_err(), "accepted: {refused}");
    }
}
