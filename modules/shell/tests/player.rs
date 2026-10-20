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
