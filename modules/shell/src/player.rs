//! Plays a meeting's tracks to the window (ADR-0008): a server on the loopback interface, a
//! port the system gives, that answers only URLs carrying the run's token and the number of the
//! last offer, a track named by its place in that offer — never a path. Read only; nothing else
//! of the disk is ever served.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

/// The tracks served now, and the number of their offer.
#[derive(Default)]
struct Offered {
    number: u64,
    tracks: Vec<PathBuf>,
}

/// The loopback server, for as long as the shell runs.
pub struct Served {
    base: String,
    token: String,
    offered: Arc<Mutex<Offered>>,
}

impl Served {
    /// Listens on `127.0.0.1`, a port the system gives, with a token of 128 random bits.
    pub fn start() -> std::io::Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(std::io::Error::other)?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|address| address.port())
            .ok_or_else(|| std::io::Error::other("no port"))?;
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let offered = Arc::new(Mutex::new(Offered::default()));
        let (heard, kept) = (token.clone(), Arc::clone(&offered));
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                // Turned away where it arrives: anything but reading an offered track holds no
                // thread. Only a track offered is streamed, on a thread of its own, so that a
                // slow reader holds no other.
                if !matches!(request.method(), Method::Get | Method::Head) {
                    let _ = request.respond(Response::empty(StatusCode(405)));
                    continue;
                }
                let Some(path) = named(request.url(), &heard, &kept.lock().unwrap()) else {
                    let _ = request.respond(Response::empty(StatusCode(404)));
                    continue;
                };
                std::thread::spawn(move || answer(request, path));
            }
        });
        Ok(Served {
            base: format!("http://127.0.0.1:{port}"),
            token,
            offered,
        })
    }

    /// Serves these tracks, and no longer those offered before: their URLs, in the same order.
    pub fn offer(&self, tracks: Vec<PathBuf>) -> Vec<String> {
        let mut offered = self.offered.lock().unwrap();
        offered.number += 1;
        offered.tracks = tracks;
        (0..offered.tracks.len())
            .map(|place| format!("{}/{}/{}/{place}", self.base, self.token, offered.number))
            .collect()
    }
}

/// The track a URL names, if it names one of the last offer, with the token.
fn named(url: &str, token: &str, offered: &Offered) -> Option<PathBuf> {
    let mut parts = url.strip_prefix('/')?.split('/');
    let (given, number, place) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || given != token || number.parse::<u64>().ok()? != offered.number {
        return None;
    }
    offered.tracks.get(place.parse::<usize>().ok()?).cloned()
}

/// Streams an offered track, whole or by one range.
fn answer(request: Request, path: PathBuf) {
    let refused = |request: Request, code: u16| {
        let _ = request.respond(Response::empty(StatusCode(code)));
    };
    let Ok(file) = File::open(path) else {
        return refused(request, 404);
    };
    let Ok(size) = file.metadata().map(|data| data.len()) else {
        return refused(request, 404);
    };
    let range = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Range"))
        .map(|header| header.value.as_str().to_string());
    let (from, to, code) = match range.as_deref().map(|range| bounds(range, size)) {
        None => (0, size.saturating_sub(1), 200),
        Some(Some((from, to))) => (from, to, 206),
        Some(None) => {
            let mut response = Response::empty(StatusCode(416));
            response.add_header(Header::from_bytes("Content-Range", format!("bytes */{size}")).unwrap());
            let _ = request.respond(response);
            return;
        }
    };
    let length = if size == 0 { 0 } else { to - from + 1 };
    let mut file = file;
    if file.seek(SeekFrom::Start(from)).is_err() {
        return refused(request, 404);
    }
    let mut response = Response::new(StatusCode(code), Vec::new(), file.take(length), Some(length as usize), None);
    response.add_header(Header::from_bytes("Content-Type", "audio/wav").unwrap());
    response.add_header(Header::from_bytes("Accept-Ranges", "bytes").unwrap());
    if code == 206 {
        response.add_header(Header::from_bytes("Content-Range", format!("bytes {from}-{to}/{size}")).unwrap());
    }
    let _ = request.respond(response);
}

/// A single byte range — `bytes=a-b`, `bytes=a-`, `bytes=-n` — within a file of `size` bytes.
fn bounds(range: &str, size: u64) -> Option<(u64, u64)> {
    let (from, to) = range.trim().strip_prefix("bytes=")?.split_once('-')?;
    if size == 0 || to.contains(',') {
        return None;
    }
    let (from, to) = match (from.trim(), to.trim()) {
        ("", last) => (size.saturating_sub(last.parse().ok()?), size - 1),
        (from, "") => (from.parse().ok()?, size - 1),
        (from, to) => (from.parse().ok()?, to.parse::<u64>().ok()?.min(size - 1)),
    };
    (from <= to && from < size).then_some((from, to))
}
