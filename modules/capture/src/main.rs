//! `summit-capture` — records a conversation into a session's folder, with no window.
//!
//!     summit-capture record <sessions-folder>     until Ctrl-C, a TERM signal, or "stop" on its input
//!     summit-capture devices                      what the system offers
//!
//! It says what happens as one JSON object per line on its output: `started`, `unavailable`,
//! `silent`, `heard`, `stopped`. Whoever started it reads them; nothing else is printed there.
//!
//! The sound is on disk as it is heard: killed, it leaves two files that read, and a
//! `session.json` without `stopped`.

use std::{
    io::{BufRead, Write},
    path::Path,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, Sender},
    },
    time::{Duration, Instant},
};

use cpal::{
    Device, FromSample, Host, Sample, SampleFormat, SizedSample, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde_json::json;
use summit_capture::{Heard, RATE, Track, Watch, new_folder, write_session};

/// A source is told silent after this long without a sound.
const SILENT_AFTER: f64 = 10.0;
/// A source may run this far behind the session's clock before zeros are written for it.
const LATE: f64 = 1.5;

const SOURCES: [&str; 2] = ["me", "others"];

/// Says an event to whoever started the program (`contracts/recording/v1`). Nobody may be
/// listening any more — a shell that crashed, a pipe that closed: the recording goes on.
fn say(event: serde_json::Value) {
    let _ = writeln!(std::io::stdout().lock(), "{event}");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["record", sessions] => record(Path::new(sessions)),
        ["devices"] => devices(),
        _ => Err("usage: summit-capture record <sessions-folder> | summit-capture devices".into()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("summit-capture: {reason}");
            ExitCode::FAILURE
        }
    }
}

/// The host that can hear what plays, where the system has one of its own; the default
/// otherwise. Chosen by what the system offers, not by which system it is.
fn host() -> Host {
    cpal::available_hosts()
        .into_iter()
        .find(|id| id.name() == "PipeWire")
        .and_then(|id| cpal::host_from_id(id).ok())
        .unwrap_or_else(cpal::default_host)
}

fn devices() -> Result<(), String> {
    let host = host();
    println!("{}", json!({ "host": host.id().name() }));
    for device in host.devices().map_err(|error| error.to_string())? {
        println!(
            "{}",
            json!({ "device": named(&device), "input": device.supports_input(), "output": device.supports_output() })
        );
    }
    Ok(())
}

fn named(device: &Device) -> String {
    device
        .description()
        .map(|description| description.name().to_string())
        .or_else(|_| device.id().map(|id| id.to_string()))
        .unwrap_or_else(|_| "unnamed".into())
}

/// A source being recorded: its file, and the stream that feeds it while it lives.
struct Source {
    track: Track,
    watch: Watch,
    _stream: Option<Stream>,
}

fn record(sessions: &Path) -> Result<(), String> {
    let host = host();
    let (sound, heard) = mpsc::channel::<(usize, Vec<f32>)>();
    let started = chrono::Local::now();
    let folder = new_folder(sessions, &started.format("%Y-%m-%d_%H-%M").to_string()).map_err(|e| e.to_string())?;

    // "me" is the default microphone; "others" is what the default output plays. Each is
    // opened on its own: a source that cannot be opened is written as silence and said, the
    // other is still recorded.
    let wanted = [(host.default_input_device(), false), (host.default_output_device(), true)];
    let mut names = Vec::new();
    let mut sources = Vec::new();
    for (index, (device, playback)) in wanted.into_iter().enumerate() {
        let path = folder.join(format!("{}.wav", SOURCES[index]));
        let opened = device
            .ok_or_else(|| "the system names no default device".to_string())
            .and_then(|device| {
                open(&device, playback, index, sound.clone()).map(|(stream, rate, channels)| (named(&device), stream, rate, channels))
            });
        let (name, stream, rate, channels) = match opened {
            Ok((name, stream, rate, channels)) => (name, Some(stream), rate, channels),
            Err(reason) => {
                say(json!({ "event": "unavailable", "source": SOURCES[index], "reason": reason }));
                ("unavailable".to_string(), None, RATE, 1)
            }
        };
        names.push(name);
        sources.push(Source {
            track: Track::create(&path, rate, channels).map_err(|e| format!("{}: {e}", path.display()))?,
            watch: Watch::new(SILENT_AFTER),
            _stream: stream,
        });
    }
    drop(sound);
    if sources.iter().all(|source| source._stream.is_none()) {
        return Err("no source could be opened: nothing to record".into());
    }
    let stamp = |time: chrono::DateTime<chrono::Local>| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    write_session(&folder, &stamp(started), None, &names[0], &names[1]).map_err(|e| e.to_string())?;
    say(json!({ "event": "started", "folder": folder, "sources": { "me": names[0], "others": names[1] } }));

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        ctrlc::set_handler(move || stop.store(true, Ordering::SeqCst)).map_err(|e| e.to_string())?;
    }
    {
        let stop = stop.clone();
        std::thread::spawn(move || {
            for line in std::io::stdin().lock().lines().map_while(Result::ok) {
                if line.trim() == "stop" {
                    stop.store(true, Ordering::SeqCst);
                }
            }
        });
    }

    let clock = Instant::now();
    let mut checked = clock;
    while !stop.load(Ordering::SeqCst) {
        match heard.recv_timeout(Duration::from_millis(200)) {
            Ok((index, samples)) => sources[index].track.push(&samples).map_err(|e| e.to_string())?,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(Duration::from_millis(200)),
        }
        if checked.elapsed() >= Duration::from_secs(1) {
            let since = checked.elapsed().as_secs_f64();
            checked = Instant::now();
            let due = clock.elapsed().as_secs_f64();
            for (index, source) in sources.iter_mut().enumerate() {
                // A source that fell silent on the system's side delivers nothing at all:
                // zeros keep its file on the session's clock.
                if (source.track.written() as f64) < (due - LATE) * f64::from(RATE) {
                    source
                        .track
                        .pad_to(((due - LATE / 3.0) * f64::from(RATE)) as u64)
                        .map_err(|e| e.to_string())?;
                }
                match source.watch.after(since, source.track.loudest()) {
                    Heard::Silent => say(json!({ "event": "silent", "source": SOURCES[index] })),
                    Heard::Again => say(json!({ "event": "heard", "source": SOURCES[index] })),
                    Heard::Nothing => {}
                }
            }
        }
    }

    // The streams are closed first, then what they had already delivered is written.
    for source in &mut sources {
        source._stream = None;
    }
    while let Ok((index, samples)) = heard.try_recv() {
        sources[index].track.push(&samples).map_err(|e| e.to_string())?;
    }
    let longest = sources.iter().map(|source| source.track.written()).max().unwrap_or(0);
    for source in sources {
        let mut track = source.track;
        track.pad_to(longest).and_then(|()| track.finish()).map_err(|e| e.to_string())?;
    }
    write_session(&folder, &stamp(started), Some(&stamp(chrono::Local::now())), &names[0], &names[1]).map_err(|e| e.to_string())?;
    say(json!({ "event": "stopped", "folder": folder, "seconds": longest as f64 / f64::from(RATE) }));
    Ok(())
}

/// Opens a source and hands what it delivers to `sound`. An output is opened as an input:
/// what it delivers is what it plays.
fn open(device: &Device, playback: bool, index: usize, sound: Sender<(usize, Vec<f32>)>) -> Result<(Stream, u32, u16), String> {
    let config = if playback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|error| format!("no default configuration ({error})"))?;
    let (rate, channels) = (config.sample_rate(), config.channels());
    let stream = match config.sample_format() {
        SampleFormat::I8 => listen::<i8>(device, config.into(), index, sound),
        SampleFormat::I16 => listen::<i16>(device, config.into(), index, sound),
        SampleFormat::I32 => listen::<i32>(device, config.into(), index, sound),
        SampleFormat::U8 => listen::<u8>(device, config.into(), index, sound),
        SampleFormat::U16 => listen::<u16>(device, config.into(), index, sound),
        SampleFormat::F32 => listen::<f32>(device, config.into(), index, sound),
        SampleFormat::F64 => listen::<f64>(device, config.into(), index, sound),
        other => Err(format!("sample format {other} not handled")),
    }?;
    stream.play().map_err(|error| format!("the stream could not start ({error})"))?;
    Ok((stream, rate, channels))
}

fn listen<T>(device: &Device, config: cpal::StreamConfig, index: usize, sound: Sender<(usize, Vec<f32>)>) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let name = SOURCES[index];
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &_| {
                let _ = sound.send((index, data.iter().map(|&sample| f32::from_sample(sample)).collect()));
            },
            move |error: cpal::Error| drop(writeln!(std::io::stderr(), "summit-capture: [{name}] {error}")),
            None,
        )
        .map_err(|error| format!("the stream could not be opened ({error})"))
}
