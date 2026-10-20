//! The loopback server a meeting's page plays from (ADR-0008, ADR-0009): the meeting offered
//! last, its tracks as one sound, whole or by range, to whoever holds the run's token — nothing
//! else of the disk.

use std::{fs, io::Read, path::Path};

use wholeheard_shell::{mix::Mix, player::Served};

fn get(url: &str, method: &str, range: Option<&str>) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
    let mut answer = match (method, range) {
        ("HEAD", _) => agent.head(url).call(),
        (_, Some(range)) => agent.get(url).header("Range", range).call(),
        (_, None) => agent.get(url).call(),
    }
    .unwrap();
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

/// A track of one channel of 16 bits at 48 kHz: a ramp, `frames` long.
fn track(path: &Path, frames: usize, step: i16) {
    let data: Vec<u8> = (0..frames).flat_map(|n| ((n % 200) as i16 * step).to_le_bytes()).collect();
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&16u32.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&48_000u32.to_le_bytes());
    file.extend_from_slice(&96_000u32.to_le_bytes());
    file.extend_from_slice(&2u16.to_le_bytes());
    file.extend_from_slice(&16u16.to_le_bytes());
    file.extend_from_slice(b"data");
    file.extend_from_slice(&(data.len() as u32).to_le_bytes());
    file.extend_from_slice(&data);
    fs::write(path, file).unwrap();
}

#[test]
fn the_meeting_offered_is_served_as_one_sound_whole_or_by_range_to_the_token_alone() {
    let folder = tempfile::tempdir().unwrap();
    let (local, remote) = (folder.path().join("local.wav"), folder.path().join("remote.wav"));
    track(&local, 150_000, 10);
    track(&remote, 100_000, 7);
    fs::write(folder.path().join("secret.txt"), b"not a track").unwrap();
    let offered = vec![(local.clone(), 0.0), (remote.clone(), 0.5)];
    let mix = Mix::of(&offered).unwrap();

    let served = Served::start().unwrap();
    let url = served.offer(offered.clone()).unwrap();
    assert!(url.starts_with("http://127.0.0.1:"), "{url}");
    // No path of the disk in the URL.
    assert!(
        !url.contains("local.wav") && !url.contains(folder.path().to_str().unwrap()),
        "{url}"
    );

    let (status, headers, body) = get(&url, "GET", None);
    assert_eq!(status, 200);
    assert!(body == mix.read(0, mix.size()).unwrap(), "the mix, whole, as made here");
    assert_eq!(header(&headers, "content-type"), Some("audio/wav"));
    assert_eq!(header(&headers, "accept-ranges"), Some("bytes"));
    assert_eq!(header(&headers, "content-length"), Some(mix.size().to_string().as_str()));

    // Seeking: a range across pieces, an open end, the last bytes; past the end, refused.
    let (status, headers, body) = get(&url, "GET", Some("bytes=100001-300000"));
    assert_eq!(status, 206);
    assert!(body == mix.read(100_001, 200_000).unwrap());
    assert_eq!(
        header(&headers, "content-range"),
        Some(format!("bytes 100001-300000/{}", mix.size()).as_str())
    );
    let (status, _, body) = get(&url, "GET", Some("bytes=250000-"));
    assert_eq!((status, body.len() as u64), (206, mix.size() - 250_000));
    let (status, _, body) = get(&url, "GET", Some("bytes=-7"));
    assert_eq!((status, body), (206, mix.read(mix.size() - 7, 7).unwrap()));
    assert_eq!(get(&url, "GET", Some(&format!("bytes={}-", mix.size()))).0, 416);
    // Several ranges, another unit: the whole.
    assert_eq!(get(&url, "GET", Some("bytes=0-1,5-9")).0, 200);
    assert_eq!(get(&url, "GET", Some("items=0-1")).0, 200);
    // HEAD says its size and sends nothing.
    let (status, headers, body) = get(&url, "HEAD", None);
    assert!(status == 200 && body.is_empty() && header(&headers, "content-length") == Some(mix.size().to_string().as_str()));

    // Without the token, for another offer, or as a path: nothing.
    let parts: Vec<&str> = url.trim_start_matches("http://").split('/').collect();
    let (base, token, number) = (format!("http://{}", parts[0]), parts[1], parts[2]);
    let next = number.parse::<u64>().unwrap() + 1;
    for refused in [
        format!("{base}/wrong-token/{number}/mix"),
        format!("{base}/{token}/{next}/mix"),
        format!("{base}/{token}/{number}/0"),
        format!("{base}/{token}/{number}/mix/x"),
        format!("{base}/{token}/{number}/../secret.txt"),
        format!("{base}/{token}/{number}/%2e%2e/secret.txt"),
        format!("{base}/secret.txt"),
        format!("{base}/"),
    ] {
        assert_eq!(get(&refused, "GET", None).0, 404, "{refused}");
    }

    // The same meeting drawn again: the same URL, still served — a player may be reading it.
    assert_eq!(served.offer(offered.clone()).as_deref(), Some(url.as_str()));
    assert_eq!(get(&url, "GET", Some("bytes=44-45")).0, 206);
    // Another meeting opened: the earlier offer is gone.
    let again = served.offer(vec![(remote.clone(), 0.0)]).unwrap();
    assert_eq!(get(&url, "GET", None).0, 404);
    let alone = Mix::of(&[(remote.clone(), 0.0)]).unwrap();
    assert!(get(&again, "GET", None).2 == alone.read(0, alone.size()).unwrap());
    // A track gone meanwhile — freed to the trash — is not found, and the server goes on.
    fs::remove_file(&remote).unwrap();
    assert_eq!(get(&again, "GET", None).0, 404);
    // A meeting with no track that reads as one has no URL.
    assert_eq!(served.offer(vec![(folder.path().join("secret.txt"), 0.0)]), None);
    let last = served.offer(vec![(local, 0.0)]).unwrap();
    assert_eq!(get(&last, "GET", Some("bytes=44-45")).2.len(), 2);
}
