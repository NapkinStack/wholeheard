//! The loopback server a meeting's page plays its tracks from (ADR-0008): the tracks offered
//! last, whole or by range, to whoever holds the run's token — nothing else of the disk.

use std::{fs, io::Read};

use wholeheard_shell::player::Served;

fn get(url: &str, range: Option<&str>) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let mut call = agent.get(url);
    if let Some(range) = range {
        call = call.header("Range", range);
    }
    let mut answer = call.call().unwrap();
    let headers = answer
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_lowercase(), value.to_str().unwrap_or_default().to_string()))
        .collect();
    let mut body = Vec::new();
    answer.body_mut().as_reader().read_to_end(&mut body).unwrap();
    (answer.status().as_u16(), headers, body)
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(named, _)| named == name).map(|(_, value)| value.as_str())
}

#[test]
fn the_tracks_offered_are_served_whole_or_by_range_to_the_token_alone() {
    let folder = tempfile::tempdir().unwrap();
    let local: Vec<u8> = (0..=255u8).cycle().take(10_000).collect();
    fs::write(folder.path().join("local.wav"), &local).unwrap();
    fs::write(folder.path().join("remote.wav"), b"remote").unwrap();
    fs::write(folder.path().join("secret.txt"), b"not a track").unwrap();

    let served = Served::start().unwrap();
    let urls = served.offer(vec![folder.path().join("local.wav"), folder.path().join("remote.wav")]);
    assert_eq!(urls.len(), 2);
    assert!(urls.iter().all(|url| url.starts_with("http://127.0.0.1:")), "{urls:?}");
    // No path of the disk in a URL: a track is named by its place.
    assert!(
        urls.iter()
            .all(|url| !url.contains("local.wav") && !url.contains(folder.path().to_str().unwrap()))
    );

    let (status, headers, body) = get(&urls[0], None);
    assert_eq!((status, body.as_slice()), (200, local.as_slice()));
    assert_eq!(header(&headers, "content-type"), Some("audio/wav"));
    assert_eq!(header(&headers, "accept-ranges"), Some("bytes"));

    // Seeking: a range, then an open end.
    let (status, headers, body) = get(&urls[0], Some("bytes=100-199"));
    assert_eq!((status, body.as_slice()), (206, &local[100..200]));
    assert_eq!(header(&headers, "content-range"), Some("bytes 100-199/10000"));
    let (status, _, body) = get(&urls[0], Some("bytes=9990-"));
    assert_eq!((status, body.as_slice()), (206, &local[9990..]));
    let (status, _, _) = get(&urls[0], Some("bytes=20000-"));
    assert_eq!(status, 416);

    // Without the token, out of the offer, or as a path: nothing.
    let parts: Vec<&str> = urls[0].trim_start_matches("http://").split('/').collect();
    let (base, token) = (format!("http://{}", parts[0]), parts[1]);
    for refused in [
        format!("{base}/wrong-token/1/0"),
        format!("{base}/{token}/1/2"),
        format!("{base}/{token}/1/../secret.txt"),
        format!("{base}/{token}/1/%2e%2e/secret.txt"),
        format!("{base}/secret.txt"),
        format!("{base}/"),
    ] {
        assert_eq!(get(&refused, None).0, 404, "{refused}");
    }

    // Another meeting opened: the earlier offer is gone.
    let again = served.offer(vec![folder.path().join("remote.wav")]);
    assert_eq!(get(&urls[0], None).0, 404);
    assert_eq!(get(&again[0], None).2, b"remote");
    // A track gone meanwhile — freed to the trash — is not found, and the server goes on.
    fs::remove_file(folder.path().join("remote.wav")).unwrap();
    assert_eq!(get(&again[0], None).0, 404);
    let last = served.offer(vec![folder.path().join("local.wav")]);
    assert_eq!(get(&last[0], Some("bytes=0-0")).2, [0]);
}

#[test]
fn only_reading_is_answered_whole_or_by_one_range() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("local.wav"), vec![7u8; 1000]).unwrap();
    let served = Served::start().unwrap();
    let url = served.offer(vec![folder.path().join("local.wav")]).remove(0);
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    // HEAD: what a player asks first — the size, no body.
    let head = agent.head(&url).call().unwrap();
    assert_eq!(head.status().as_u16(), 200);
    assert_eq!(
        head.headers().get("content-length").and_then(|value| value.to_str().ok()),
        Some("1000")
    );
    // Nothing is written through it.
    assert_eq!(agent.post(&url).send("x").unwrap().status().as_u16(), 405);
    assert_eq!(agent.delete(&url).call().unwrap().status().as_u16(), 405);
    // Several ranges, another unit, a range backwards: ignored, the whole file (RFC 9110 §14.2).
    for ignored in ["bytes=0-9,20-29", "items=0-9", "bytes=5-4"] {
        let (status, _, body) = get(&url, Some(ignored));
        assert_eq!((status, body.len()), (200, 1000), "{ignored}");
    }
    assert_eq!(get(&url, Some("bytes=-10")).2, vec![7u8; 10]);
}

/// Raw bytes sent to the server, and what it answers within a second — none when it closed.
fn raw(base: &str, sent: &[u8]) -> Option<String> {
    use std::{io::Write, net::TcpStream, time::Duration};
    let address = base.trim_start_matches("http://").split('/').next().unwrap();
    let mut stream = TcpStream::connect(address).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    stream.write_all(sent).ok()?;
    let mut answer = vec![0; 512];
    let read = stream.read(&mut answer).ok()?;
    Some(String::from_utf8_lossy(&answer[..read]).into_owned())
}

#[test]
fn no_one_without_the_token_stops_or_holds_the_server() {
    use std::{io::Write, net::TcpStream, time::Duration};
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("local.wav"), vec![1u8; 100]).unwrap();
    fs::write(folder.path().join("beside.txt"), b"not offered").unwrap();
    let served = Served::start().unwrap();
    let url = served.offer(vec![folder.path().join("local.wav")]).remove(0);
    let token = url.trim_start_matches("http://").split('/').nth(1).unwrap().to_string();
    let base = url.split(&format!("/{token}/")).next().unwrap().to_string();
    // A file beside the offered one, by its name: never.
    assert_eq!(get(&format!("{base}/{token}/1/beside.txt"), None).0, 404);
    // A body announced as a petabyte, never sent: the server neither ends nor waits on it.
    let _ = raw(
        &base,
        b"POST /x HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 1000000000000000\r\n\r\n",
    );
    // Clients that send half a request and keep the line open: they hold no one else.
    let held: Vec<TcpStream> = (0..50)
        .filter_map(|_| {
            let mut stream = TcpStream::connect(base.trim_start_matches("http://")).ok()?;
            stream.write_all(b"GET /nope HTTP/1.1\r\nContent-Length: 1000000000\r\n").ok()?;
            Some(stream)
        })
        .collect();
    assert!(!held.is_empty());
    let started = std::time::Instant::now();
    let (status, _, body) = get(&url, Some("bytes=0-9"));
    assert_eq!((status, body.len()), (206, 10));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a reader waited {:?}",
        started.elapsed()
    );
    // A request that never ends its headers is let go.
    let mut slow = TcpStream::connect(base.trim_start_matches("http://")).unwrap();
    slow.write_all(b"GET /").unwrap();
    slow.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let mut rest = Vec::new();
    let _ = std::io::Read::read_to_end(&mut slow, &mut rest);
    drop(held);
    assert_eq!(get(&url, None).2.len(), 100, "the server still answers");
}
