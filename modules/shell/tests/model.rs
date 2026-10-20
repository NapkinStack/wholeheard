//! The model fetched, tried against a server played here on this machine: a file of known
//! bytes, served whole, from where it is asked, cut short, or with its range ignored.

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::Path,
    thread,
};

use sha2::{Digest, Sha256};
use wholeheard_shell::model::{self, Published};

/// How the played server answers.
#[derive(Clone, Copy)]
enum Serve {
    /// As a server does: from the range asked, or whole.
    Well,
    /// Sends only this many bytes, then closes.
    Cut(usize),
    /// Sends the file whole whatever range is asked.
    WholeAlways,
    /// Answers this status, with a page of its own.
    Status(u16),
}

/// The file served: bytes no one would take for another file.
fn file() -> Vec<u8> {
    (0..300_000u32).map(|n| (n.wrapping_mul(2_654_435_761) >> 24) as u8).collect()
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Starts a server for `answers` requests, and gives its address and the ranges it was asked.
fn served(answers: Vec<Serve>) -> (String, thread::JoinHandle<Vec<Option<u64>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/model.bin", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let body = file();
        let mut asked = Vec::new();
        for serve in answers {
            let (mut stream, _) = listener.accept().unwrap();
            let mut from = None;
            for line in BufReader::new(stream.try_clone().unwrap()).lines() {
                let line = line.unwrap();
                if line.is_empty() {
                    break;
                }
                if let Some(range) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    from = range.trim_end_matches('-').parse().ok();
                }
            }
            asked.push(from);
            if let Serve::Status(status) = serve {
                let page = format!("HTTP/1.1 {status} Status\r\nContent-Length: 9\r\nConnection: close\r\n\r\nan error.");
                let _ = stream.write_all(page.as_bytes());
                continue;
            }
            let (status, start) = match (serve, from) {
                (Serve::WholeAlways, _) | (_, None) => ("200 OK", 0),
                (_, Some(from)) => ("206 Partial Content", from as usize),
            };
            let rest = &body[start..];
            let sent = match serve {
                Serve::Cut(bytes) => &rest[..bytes],
                _ => rest,
            };
            let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", rest.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(sent);
        }
        asked
    });
    (address, handle)
}

fn published(body: &[u8]) -> Published {
    Published {
        address: "unused",
        host: "example.org",
        bytes: body.len() as u64,
        sha256: Box::leak(hex(body).into_boxed_str()),
    }
}

#[test]
fn a_model_is_fetched_whole_checked_and_put_in_its_place() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("models/ggml.bin");
    let (address, server) = served(vec![Serve::Well]);
    let mut told = Vec::new();
    model::fetch(&published(&file()), &address, &to, &mut |held| told.push(held)).unwrap();
    assert_eq!(fs::read(&to).unwrap(), file());
    assert!(!model::part(&to).exists());
    assert_eq!(server.join().unwrap(), [None]);
    // Told as it comes, not only at the end: a long fetch shows it moves.
    assert!(told.len() > 2 && told[0] < 300_000, "{told:?}");
    assert_eq!(told.last(), Some(&300_000));
    assert!(told.windows(2).all(|two| two[0] <= two[1]), "progress goes back");
}

#[test]
fn a_fetch_that_was_cut_is_taken_up_where_it_stopped_and_never_used_as_it_is() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("ggml.bin");
    let (address, server) = served(vec![Serve::Cut(120_000), Serve::Well]);
    let said = model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap_err();
    // Cut: nothing in the model's place, what came kept apart.
    assert!(!to.exists() && fs::metadata(model::part(&to)).unwrap().len() == 120_000, "{said}");
    model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap();
    assert_eq!(fs::read(&to).unwrap(), file());
    assert_eq!(server.join().unwrap(), [None, Some(120_000)]);
}

#[test]
fn a_server_that_sends_the_whole_file_again_is_written_anew() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("ggml.bin");
    fs::write(model::part(&to), &file()[..50_000]).unwrap();
    let (address, server) = served(vec![Serve::WholeAlways]);
    model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap();
    assert_eq!(fs::read(&to).unwrap(), file());
    assert_eq!(server.join().unwrap(), [Some(50_000)]);
}

#[test]
fn a_file_that_is_not_the_published_one_is_removed_and_never_put_in_place() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("ggml.bin");
    let mut other = published(&file());
    other.sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
    let (address, server) = served(vec![Serve::Well]);
    let said = model::fetch(&other, &address, &to, &mut |_| {}).unwrap_err();
    assert!(said.contains("not the published model") && said.contains("example.org"), "{said}");
    assert!(!to.exists() && !model::part(&to).exists());
    server.join().unwrap();
    // A part longer than the model is begun again from nothing.
    fs::write(model::part(&to), vec![0; 300_001]).unwrap();
    let (address, server) = served(vec![Serve::Well]);
    model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap();
    assert_eq!(server.join().unwrap(), [None]);
}

#[test]
fn nothing_is_fetched_from_a_host_that_cannot_be_reached_and_the_size_reads_as_people_say_it() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("ggml.bin");
    let said = model::fetch(&published(&file()), "http://127.0.0.1:9/model.bin", &to, &mut |_| {}).unwrap_err();
    assert!(said.starts_with("the model could not be fetched"), "{said}");
    assert!(!to.exists() && !Path::new(&model::part(&to)).exists());
    assert_eq!(model::size(model::DEFAULT.bytes), "874 MB");
    assert_eq!(model::size(1_624_555_275), "1.6 GB");
    assert!(model::DEFAULT.address.starts_with("https://huggingface.co/") && model::DEFAULT.host == "huggingface.co");
}

#[test]
fn a_server_that_fails_leaves_the_part_as_it_was_and_one_without_it_has_it_begun_again() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("ggml.bin");
    fs::write(model::part(&to), &file()[..50_000]).unwrap();
    // An error answered to a resume is no file: the part is kept, byte for byte, and said.
    let (address, server) = served(vec![Serve::Status(503)]);
    let said = model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap_err();
    assert!(said.contains("HTTP 503"), "{said}");
    assert_eq!(fs::read(model::part(&to)).unwrap(), &file()[..50_000]);
    assert_eq!(server.join().unwrap(), [Some(50_000)]);
    // A server that no longer has what was begun: the part is removed, the next fetch starts anew.
    let (address, server) = served(vec![Serve::Status(416)]);
    let said = model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap_err();
    assert!(said.contains("from the start"), "{said}");
    assert!(!model::part(&to).exists() && !to.exists());
    server.join().unwrap();
}

#[test]
fn a_whole_part_with_the_published_fingerprint_is_put_in_place_without_asking_again() {
    let folder = tempfile::tempdir().unwrap();
    let to = folder.path().join("ggml.bin");
    fs::write(model::part(&to), file()).unwrap();
    // No server answers there: a request would fail the fetch.
    let (address, server) = served(vec![]);
    server.join().unwrap();
    model::fetch(&published(&file()), &address, &to, &mut |_| {}).unwrap();
    assert_eq!(fs::read(&to).unwrap(), file());
}
