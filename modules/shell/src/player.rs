//! Plays a meeting's tracks to the window (ADR-0008): a server on the loopback interface, a
//! port the system gives, that answers only URLs carrying the run's token and the number of the
//! last offer, a track named by its place in that offer — never a path. Read only; nothing else
//! of the disk is ever served.
//!
//! What no reader of the window does is bounded: a request's headers are read for a few seconds
//! at most and never past a few kilobytes, a body is never read, a connection serves one request,
//! and only so many are held at once. A request refused holds nothing.

use std::{
    convert::Infallible,
    io,
    net::TcpListener as Listener,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use futures_util::StreamExt;
use http_body_util::{BodyExt, Empty, StreamBody, combinators::BoxBody};
use hyper::{Method, Request, Response, StatusCode, body::Frame, header, server::conn::http1, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::{io::AsyncReadExt, io::AsyncSeekExt, net::TcpListener, sync::Semaphore};
use tokio_util::io::ReaderStream;

/// How long a request's headers may take to arrive.
const HEADERS: Duration = Duration::from_secs(5);
/// How many connections are held at once; one more is closed as it comes.
const AT_ONCE: usize = 256;

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
    pub fn start() -> io::Result<Self> {
        let listener = Listener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(io::Error::other)?;
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let offered = Arc::new(Mutex::new(Offered::default()));
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let (heard, kept) = (token.clone(), Arc::clone(&offered));
        std::thread::spawn(move || runtime.block_on(serve(listener, heard, kept)));
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

async fn serve(listener: Listener, token: String, offered: Arc<Mutex<Offered>>) {
    let Ok(listener) = TcpListener::from_std(listener) else { return };
    let held = Arc::new(Semaphore::new(AT_ONCE));
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        // Too many held already: this one is closed as it came.
        let Ok(permit) = Arc::clone(&held).try_acquire_owned() else {
            continue;
        };
        let (token, offered) = (token.clone(), Arc::clone(&offered));
        tokio::spawn(async move {
            let service = service_fn(move |request: Request<hyper::body::Incoming>| {
                let path = named(request.uri().path(), &token, &offered.lock().unwrap());
                async move { Ok::<_, Infallible>(answer(request.method().clone(), request.headers().get(header::RANGE).cloned(), path).await) }
            });
            let _ = http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(HEADERS)
                .max_buf_size(16 * 1024)
                .keep_alive(false)
                .serve_connection(TokioIo::new(stream), service)
                .await;
            drop(permit);
        });
    }
}

/// The track a URL's path names, if it names one of the last offer, with the token.
fn named(url: &str, token: &str, offered: &Offered) -> Option<PathBuf> {
    let mut parts = url.strip_prefix('/')?.split('/');
    let (given, number, place) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || given != token || number.parse::<u64>().ok()? != offered.number {
        return None;
    }
    offered.tracks.get(place.parse::<usize>().ok()?).cloned()
}

type Body = BoxBody<Bytes, io::Error>;

fn empty(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(Empty::new().map_err(|never| match never {}).boxed());
    *response.status_mut() = code;
    response
}

/// An offered track, whole or by one range; anything else refused, its body never read.
async fn answer(method: Method, range: Option<header::HeaderValue>, path: Option<PathBuf>) -> Response<Body> {
    if method != Method::GET && method != Method::HEAD {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(path) = path else { return empty(StatusCode::NOT_FOUND) };
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(size) = file.metadata().await.map(|data| data.len()) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let (from, length, code) = match range
        .as_ref()
        .and_then(|range| range.to_str().ok())
        .and_then(|range| asked(range, size))
    {
        None => (0, size, StatusCode::OK),
        Some(Some((from, to))) => (from, to - from + 1, StatusCode::PARTIAL_CONTENT),
        Some(None) => {
            let mut refused = empty(StatusCode::RANGE_NOT_SATISFIABLE);
            refused
                .headers_mut()
                .insert(header::CONTENT_RANGE, format!("bytes */{size}").parse().unwrap());
            return refused;
        }
    };
    if file.seek(io::SeekFrom::Start(from)).await.is_err() {
        return empty(StatusCode::NOT_FOUND);
    }
    let body: Body = if method == Method::HEAD {
        Empty::new().map_err(|never| match never {}).boxed()
    } else {
        BodyExt::boxed(StreamBody::new(
            ReaderStream::new(file.take(length)).map(|read| read.map(Frame::data)),
        ))
    };
    let mut response = Response::new(body);
    *response.status_mut() = code;
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, "audio/wav".parse().unwrap());
    headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    headers.insert(header::CONTENT_LENGTH, length.into());
    if code == StatusCode::PARTIAL_CONTENT {
        headers.insert(
            header::CONTENT_RANGE,
            format!("bytes {from}-{}/{size}", from + length - 1).parse().unwrap(),
        );
    }
    response
}

/// What a Range header asks of a file of `size` bytes (RFC 9110 §14): none when it is to be
/// ignored — another unit, several ranges, a range that cannot be read —, the bytes asked when
/// some of them are there, and `Some(None)` when none is.
fn asked(range: &str, size: u64) -> Option<Option<(u64, u64)>> {
    let (from, to) = range.trim().strip_prefix("bytes=")?.split_once('-')?;
    if to.contains(',') {
        return None;
    }
    let (from, to) = match (from.trim(), to.trim()) {
        ("", "") => return None,
        ("", last) => {
            let last: u64 = last.parse().ok()?;
            if last == 0 || size == 0 {
                return Some(None);
            }
            (size.saturating_sub(last), size - 1)
        }
        (from, "") => (from.parse().ok()?, size.saturating_sub(1)),
        (from, to) => {
            let (from, to): (u64, u64) = (from.parse().ok()?, to.parse().ok()?);
            if from > to {
                return None;
            }
            (from, to.min(size.saturating_sub(1)))
        }
    };
    Some((from < size).then_some((from, to)))
}
