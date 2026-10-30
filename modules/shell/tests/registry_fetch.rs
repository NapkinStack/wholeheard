//! The list read again on the user's gesture (D40, ADR-0010), tried against a server played here
//! on this machine: what it answers is taken only when it is a list this build would ship.

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use minisign::KeyPair;
use wholeheard_shell::registry;

/// A pair made for this run of the tests, never the decider's: its public key as the shell holds
/// one, and what signs a list with it.
struct TestKey {
    public: String,
    pair: KeyPair,
}

fn test_key() -> TestKey {
    let pair = KeyPair::generate_unencrypted_keypair().unwrap();
    TestKey {
        public: pair.pk.to_base64(),
        pair,
    }
}

impl TestKey {
    /// The signature file `minisign -S` would write for `list`.
    fn signed(&self, list: &str) -> String {
        minisign::sign(None, &self.pair.sk, list.as_bytes(), None, None)
            .unwrap()
            .to_string()
    }
}

/// What a server played here answers, and the paths it was asked for.
struct Served {
    address: String,
    asked: Arc<Mutex<Vec<String>>>,
}

/// Answers the list with `status`, `headers` — each line ended by CRLF — and `body`, and its
/// signature's address with `signature`, or a page not found.
fn served_with(status: u16, body: String, headers: &'static str, signature: Option<String>) -> Served {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/models.json", listener.local_addr().unwrap());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let noted = asked.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut lines = BufReader::new(stream.try_clone().unwrap()).lines();
            let first = lines.next().unwrap().unwrap();
            let path = first.split(' ').nth(1).unwrap().to_owned();
            for line in lines {
                if line.unwrap().is_empty() {
                    break;
                }
            }
            let (status, headers, body) = match (path.ends_with(".minisig"), &signature) {
                (false, _) => (status, headers, body.as_str()),
                (true, Some(signature)) => (200, "", signature.as_str()),
                (true, None) => (404, "", "Not Found"),
            };
            noted.lock().unwrap().push(path);
            let head = format!(
                "HTTP/1.1 {status} Status\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(body.as_bytes());
        }
    });
    Served { address, asked }
}

/// Answers `list`, with its signature by `key`.
fn signed_by(key: &TestKey, list: String) -> Served {
    let signature = key.signed(&list);
    served_with(200, list, "", Some(signature))
}

/// The list shipped, with one more model after it — a test's, never fetched.
fn with_another() -> String {
    registry::SHIPPED.replacen(
        "\n  ]",
        r#",
    {"address": "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q8_0.bin", "bytes": 1000, "sha256": "0000000000000000000000000000000000000000000000000000000000000000", "good": "faster"}
  ]"#,
        1,
    )
}

#[test]
fn a_list_signed_by_the_key_is_taken_whole() {
    let key = test_key();
    let served = signed_by(&key, with_another());
    let listed = registry::fetch(&served.address, Some(&key.public)).unwrap();
    assert_eq!(listed.len(), registry::shipped().len() + 1);
    assert_eq!(listed[..listed.len() - 1], registry::shipped()[..]);
    assert_eq!(
        wholeheard_shell::model::file(&listed.last().unwrap().published),
        "ggml-base-q8_0.bin"
    );
    assert_eq!(*served.asked.lock().unwrap(), ["/models.json", "/models.json.minisig"]);
}

#[test]
fn the_list_is_read_again_from_main_on_the_forge() {
    assert_eq!(
        registry::ADDRESS,
        "https://raw.githubusercontent.com/NapkinStack/wholeheard/main/modules/shell/registry/models.json"
    );
    assert_eq!(registry::host(registry::ADDRESS), "raw.githubusercontent.com");
}

#[test]
fn a_list_unsigned_signed_by_another_key_or_changed_after_it_was_signed_is_refused() {
    let key = test_key();
    let list = with_another();
    let refused = [
        ("unsigned", served_with(200, list.clone(), "", None)),
        ("signed by another key", signed_by(&test_key(), list.clone())),
        (
            "changed after it was signed",
            served_with(200, list.replace("\"bytes\": 1000", "\"bytes\": 1001"), "", Some(key.signed(&list))),
        ),
        (
            "a signature that is none",
            served_with(200, list.clone(), "", Some("<html>".into())),
        ),
    ];
    for (what, served) in refused {
        let why = registry::fetch(&served.address, Some(&key.public)).expect_err(what);
        assert!(why.contains("signature"), "{what}: {why}");
    }
}

#[test]
fn with_no_key_built_in_nothing_is_asked_and_it_is_said_to_wait_on_it() {
    let key = test_key();
    let served = signed_by(&key, with_another());
    let why = registry::fetch(&served.address, None).unwrap_err();
    assert!(why.contains("waits on the decider's key"), "{why}");
    assert!(served.asked.lock().unwrap().is_empty());
}

#[test]
fn a_signed_list_is_still_refused_when_this_build_would_not_ship_it() {
    let key = test_key();
    let first_not_the_default = registry::SHIPPED.replacen("ggml-large-v3-turbo-q8_0.bin", "ggml-other.bin", 1);
    let refused = [
        ("not JSON", "<html>".to_owned()),
        ("a version unknown", registry::SHIPPED.replace("\"version\": 1", "\"version\": 2")),
        ("larger than 64 KB", format!("{}{}", with_another(), " ".repeat(registry::LIMIT))),
        ("its first model not this build's default", first_not_the_default),
    ];
    for (what, list) in refused {
        let served = signed_by(&key, list);
        assert!(
            registry::fetch(&served.address, Some(&key.public)).is_err(),
            "{what} should be refused"
        );
    }
}

#[test]
fn what_does_not_answer_as_a_page_is_refused() {
    let key = test_key();
    let signature = key.signed(&with_another());
    for (what, status, body) in [
        ("a page not found", 404, "Not Found".to_owned()),
        ("a server's error", 500, with_another()),
    ] {
        let served = served_with(status, body, "", Some(signature.clone()));
        assert!(
            registry::fetch(&served.address, Some(&key.public)).is_err(),
            "{what} should be refused"
        );
    }
}

#[test]
fn a_server_that_does_not_answer_is_said_so() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/models.json", listener.local_addr().unwrap());
    drop(listener);
    assert!(registry::fetch(&address, Some(&test_key().public)).is_err());
}

#[test]
fn a_list_sent_on_from_another_address_is_refused() {
    // The host the button names is the one read: a redirect is not followed, wherever it goes.
    let key = test_key();
    let elsewhere = signed_by(&key, with_another());
    let location: &'static str = Box::leak(format!("Location: {}\r\n", elsewhere.address).into_boxed_str());
    let served = served_with(302, String::new(), location, None);
    assert!(registry::fetch(&served.address, Some(&key.public)).is_err());
    assert!(elsewhere.asked.lock().unwrap().is_empty());
}

#[test]
fn the_check_of_the_list_in_the_repository_refuses_a_signature_that_does_not_verify_once_a_key_is_built_in() {
    let key = test_key();
    let list = with_another();
    let good = key.signed(&list);
    assert!(registry::in_the_repository(&list, Some(&good), Some(&key.public)).is_ok());
    assert!(registry::in_the_repository(&list, None, Some(&key.public)).is_err());
    assert!(registry::in_the_repository(&list, Some(&test_key().signed(&list)), Some(&key.public)).is_err());
    assert!(registry::in_the_repository(&format!("{list} "), Some(&good), Some(&key.public)).is_err());
    // No key yet: nothing can be verified, and the check says so rather than saying it passed.
    let said = registry::in_the_repository(&list, None, None).unwrap();
    assert!(said.contains("waits on the decider's key"), "{said}");
    assert!(!said.contains("verifies"), "{said}");
}

#[test]
fn the_list_in_the_repository_verifies_or_waits_on_the_key_and_the_check_says_which() {
    let signature = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/registry/models.json.minisig")).ok();
    let said = registry::in_the_repository(registry::SHIPPED, signature.as_deref(), registry::PUBLIC_KEY)
        .expect("registry/models.json is signed by the key built in: minisign -Sm it, then commit its .minisig");
    // Straight to the error stream: what the test harness captures would not be seen when it passes.
    writeln!(std::io::stderr(), "registry/models.json: {said}").unwrap();
}
