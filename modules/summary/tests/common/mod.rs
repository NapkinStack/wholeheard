//! A provider played on this machine, for the tests that need one: no online service, no
//! key of anyone.
#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::mpsc::{self, Receiver},
    thread,
};

pub const MINUTES: &str = "## Points forts\n\nLa date est fixée au 3 juin.\n\n## Actions\n\n- Aucune.\n\n## Points clés\n\n- Le 3 juin.\n";

/// A provider that answers one request with `status` and `body`, and hands back what it was
/// sent: the request's head and its body.
pub fn provider(status: &'static str, body: String) -> (String, Receiver<(String, String)>) {
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

pub fn answering(text: &str, finish: &str) -> String {
    serde_json::json!({ "choices": [{ "message": { "role": "assistant", "content": text }, "finish_reason": finish }] }).to_string()
}
