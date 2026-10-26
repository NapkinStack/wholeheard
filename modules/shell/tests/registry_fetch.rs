//! The list read again on the user's gesture (D40, ADR-0010), tried against a server played here
//! on this machine: what it answers is taken only when it is a list this build would ship.

use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    thread,
};

use wholeheard_shell::registry;

/// Answers one request with `status` and `body`, and gives the address it listens at.
fn served(status: u16, body: String) -> String {
    served_with(status, body, "")
}

/// The same, with `headers` — each line ended by CRLF — after the status line.
fn served_with(status: u16, body: String, headers: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/models.json", listener.local_addr().unwrap());
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        for line in BufReader::new(stream.try_clone().unwrap()).lines() {
            if line.unwrap().is_empty() {
                break;
            }
        }
        let head = format!(
            "HTTP/1.1 {status} Status\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(body.as_bytes());
    });
    address
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
fn a_list_that_reads_is_taken_whole() {
    let listed = registry::fetch(&served(200, with_another())).unwrap();
    assert_eq!(listed.len(), registry::shipped().len() + 1);
    assert_eq!(listed[..listed.len() - 1], registry::shipped()[..]);
    assert_eq!(
        wholeheard_shell::model::file(&listed.last().unwrap().published),
        "ggml-base-q8_0.bin"
    );
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
fn what_is_not_a_list_this_build_would_ship_is_refused() {
    let first_not_the_default = registry::SHIPPED.replacen("ggml-large-v3-turbo-q8_0.bin", "ggml-other.bin", 1);
    let refused = [
        ("a page not found", served(404, "Not Found".into())),
        ("a server's error", served(500, with_another())),
        ("not JSON", served(200, "<html>".into())),
        (
            "a version unknown",
            served(200, registry::SHIPPED.replace("\"version\": 1", "\"version\": 2")),
        ),
        (
            "larger than 64 KB",
            served(200, format!("{}{}", with_another(), " ".repeat(registry::LIMIT))),
        ),
        ("its first model not this build's default", served(200, first_not_the_default)),
    ];
    for (what, address) in refused {
        assert!(registry::fetch(&address).is_err(), "{what} should be refused");
    }
}

#[test]
fn a_server_that_does_not_answer_is_said_so() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/models.json", listener.local_addr().unwrap());
    drop(listener);
    assert!(registry::fetch(&address).is_err());
}

#[test]
fn a_list_sent_on_from_another_address_is_refused() {
    // The host the button names is the one read: a redirect is not followed, wherever it goes.
    let elsewhere = served(200, with_another());
    let location: &'static str = Box::leak(format!("Location: {elsewhere}\r\n").into_boxed_str());
    assert!(registry::fetch(&served_with(302, String::new(), location)).is_err());
}
