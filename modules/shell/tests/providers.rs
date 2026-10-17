//! Who can write the minutes: an Ollama made here, answering as Ollama does.

use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use summit_shell::providers::{self, ONLINE, host, local, ollama_address, ollama_models};

/// A server that answers each request with `body`, as many times as asked.
fn answering(body: &'static str, times: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming().take(times) {
            let mut stream = stream.unwrap();
            let mut asked = [0u8; 2048];
            let _ = stream.read(&mut asked);
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    address
}

#[test]
fn the_models_ollama_holds_are_offered_by_name_and_none_when_it_does_not_answer() {
    let ollama = answering(
        r#"{"models":[{"name":"qwen3:14b","size":1},{"name":"mistral:7b"},{"name":"mistral:7b"},{"size":2}]}"#,
        1,
    );
    assert_eq!(ollama_models(&ollama), ["mistral:7b", "qwen3:14b"]);
    // Not running, or not Ollama: no model, no fault.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    assert!(ollama_models(&format!("http://{closed}")).is_empty());
    assert!(ollama_models(&answering("not json", 1)).is_empty());
    assert!(ollama_models(&answering(r#"{"other":1}"#, 1)).is_empty());
    // A name `summit-summary` would refuse is not offered.
    let odd = answering(r#"{"models":[{"name":""},{"name":"  "},{"name":"a\u0000b"},{"name":"ok"}]}"#, 1);
    assert_eq!(ollama_models(&odd), ["ok"]);
    assert_eq!(ollama_address(&format!("{}/", providers::OLLAMA)), "http://localhost:11434/v1");
}

#[test]
fn an_ollama_that_takes_the_question_and_never_answers_holds_nothing_up() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        let held: Vec<_> = listener.incoming().take(1).collect();
        thread::sleep(std::time::Duration::from_secs(30));
        drop(held);
    });
    let asked = std::time::Instant::now();
    assert!(ollama_models(&address).is_empty());
    assert!(asked.elapsed() < std::time::Duration::from_secs(5), "{:?}", asked.elapsed());
}

#[test]
fn a_provider_s_host_is_what_its_key_is_filed_under() {
    assert_eq!(host("https://openrouter.ai/api/v1").as_deref(), Some("openrouter.ai"));
    assert_eq!(host("http://LocalHost:11434/v1").as_deref(), Some("localhost:11434"));
    assert_eq!(host("https://user@api.example.org?x#y").as_deref(), Some("api.example.org"));
    assert_eq!(host("https://a.org#x").as_deref(), Some("a.org"));
    // The same server however its address is written: the scheme in any case, its own port or none.
    assert_eq!(host("HTTPS://OpenRouter.ai:443/api/v1").as_deref(), Some("openrouter.ai"));
    assert_eq!(host("http://example.org:80").as_deref(), Some("example.org"));
    assert_eq!(host("https://example.org:8443").as_deref(), Some("example.org:8443"));
    for none in [
        "",
        "openrouter.ai",
        "ftp://x",
        "https://",
        "https:///v1",
        "https://openrouter.ai/api/v1not an address",
        " https://x.org",
    ] {
        assert_eq!(host(none), None, "{none}");
    }
    assert!(ONLINE.iter().all(|online| host(online.address).is_some()));
}

#[test]
fn an_address_of_this_machine_is_local_and_nothing_else_is() {
    for here in [
        "http://localhost:11434/v1",
        "http://LOCALHOST/v1",
        "http://127.0.0.1:8080/v1",
        "http://127.3.2.1/v1",
        "http://[::1]:11434/v1",
        "http://0.0.0.0:11434/v1",
        "http://[::ffff:127.0.0.1]:11434/v1",
    ] {
        assert!(local(here), "{here}");
    }
    for away in [
        "https://openrouter.ai/api/v1",
        "http://localhost.example.org/v1",
        "http://192.168.1.10:11434/v1",
        "http://user@evil.org/@localhost",
        // What summit-summary refuses is never this machine's either.
        "http://evil.invalid\\@localhost/v1",
        "http://localhost#@evil.invalid/v1",
        "http://localhost?x/v1",
        "not an address",
    ] {
        assert!(!local(away), "{away}");
    }
}
