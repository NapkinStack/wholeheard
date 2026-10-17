//! `summit-capture` — records a conversation into a session's folder, with no window.
//!
//!     summit-capture record <sessions-folder>     until Ctrl-C, a TERM signal, or "stop" on its input
//!     summit-capture record <sessions-folder> --as v2     the same, each source as delivered
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
use summit_capture::{Event, Heard, RATE, SOURCES, Track, Watch, new_folder, v2, write_session};

/// A source is told silent after this long without a sound.
const SILENT_AFTER: f64 = 10.0;
/// A source may run this far behind the session's clock before zeros are written for it.
const LATE: f64 = 1.5;

/// Between two points of a track's clock, at most this long.
const POINT_EVERY: f64 = 5.0;
/// A piece of sound heard this far, in seconds, from where its track's last point puts it
/// gets a point of its own: the source stopped a moment, too short for zeros to be written.
/// Well under one period of a system's sound — a lost period, 21 ms here, is what an
/// overrun looks like — and well over what the instants move by themselves: 0.1 ms, measured.
const OFF: f64 = 0.005;

/// The two sources as `session-audio/v2` names their tracks; what the program says names
/// them as `recording/v1` does, `SOURCES`.
const TRACKS: [&str; 2] = ["local", "remote"];

/// What a source just delivered: which source, when it was heard — in seconds since the
/// program took its clock — and the sound, interleaved.
type Sound = (usize, f64, Vec<f32>);

/// Says an event to whoever started the program (`contracts/recording/v1`). Nobody may be
/// listening any more — a shell that crashed, a pipe that closed: the recording goes on.
fn say(event: Event) {
    let _ = writeln!(std::io::stdout().lock(), "{}", event.line());
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["record", sessions] => record(Path::new(sessions)),
        ["record", sessions, "--as", "v2"] => record_v2(Path::new(sessions)),
        ["devices"] => devices(),
        _ => Err("usage: summit-capture record <sessions-folder> [--as v2] | summit-capture devices".into()),
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
        let mut line = json!({ "device": named(&device), "input": device.supports_input(), "output": device.supports_output() });
        let used = named_as_used(&host, &device);
        if used != line["device"] {
            line["stands_for"] = used.into();
        }
        println!("{line}");
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

/// The name of the device a source records. A desktop's default device is a name for whichever
/// one it stands for at that moment: that one is named, read from PipeWire's metadata, so that
/// the user can see it is not the microphone their call uses. Unread, the default's own name.
fn named_as_used(host: &Host, device: &Device) -> String {
    let key = match device.id().as_ref().map(|id| id.id()) {
        Ok("input_default") => "default.audio.source",
        Ok("output_default" | "sink_default") => "default.audio.sink",
        _ => return named(device),
    };
    let standing = || {
        let printed = std::process::Command::new("pw-metadata")
            .args(["-n", "default", "0", key])
            .output()
            .ok()?;
        let node = summit_capture::standing_for(&String::from_utf8_lossy(&printed.stdout), key)?;
        host.devices().ok()?.find(|other| other.id().is_ok_and(|id| id.id() == node))
    };
    standing().map_or_else(|| named(device), |stood_for| named(&stood_for))
}

/// A source being recorded: its file, and the stream that feeds it while it lives.
struct Source {
    track: Track,
    watch: Watch,
    _stream: Option<Stream>,
}

fn record(sessions: &Path) -> Result<(), String> {
    let host = host();
    let (sound, heard) = mpsc::channel::<Sound>();
    let started = chrono::Local::now();
    let zero = Instant::now();
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
                open(&device, playback, index, zero, sound.clone())
                    .map(|(stream, rate, channels)| (named_as_used(&host, &device), stream, rate, channels))
            });
        let (name, stream, rate, channels) = match opened {
            Ok((name, stream, rate, channels)) => (name, Some(stream), rate, channels),
            Err(reason) => {
                say(Event::Unavailable {
                    source: index,
                    reason: &reason,
                });
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
    say(Event::Started {
        folder: &folder,
        sources: [&names[0], &names[1]],
    });

    let stop = stop_asked()?;

    let clock = Instant::now();
    let mut checked = clock;
    while !stop.load(Ordering::SeqCst) {
        match heard.recv_timeout(Duration::from_millis(200)) {
            Ok((index, _, samples)) => sources[index].track.push(&samples).map_err(|e| e.to_string())?,
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
                    Heard::Silent => say(Event::Silent { source: index }),
                    Heard::Again => say(Event::Heard { source: index }),
                    Heard::Nothing => {}
                }
            }
        }
    }

    // The streams are closed first, then what they had already delivered is written.
    for source in &mut sources {
        source._stream = None;
    }
    while let Ok((index, _, samples)) = heard.try_recv() {
        sources[index].track.push(&samples).map_err(|e| e.to_string())?;
    }
    let longest = sources.iter().map(|source| source.track.written()).max().unwrap_or(0);
    for source in sources {
        let mut track = source.track;
        track.pad_to(longest).and_then(|()| track.finish()).map_err(|e| e.to_string())?;
    }
    write_session(&folder, &stamp(started), Some(&stamp(chrono::Local::now())), &names[0], &names[1]).map_err(|e| e.to_string())?;
    say(Event::Stopped {
        folder: &folder,
        seconds: longest as f64 / f64::from(RATE),
    });
    Ok(())
}

/// True once the user asked to stop: an interrupt, a termination signal, or `stop` on the input.
fn stop_asked() -> Result<Arc<AtomicBool>, String> {
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
    Ok(stop)
}

/// A track being recorded as `session-audio/v2` describes it, and where its clock stands.
struct Delivered {
    track: v2::Track,
    watch: Watch,
    /// The last point said of this track: a frame, and when it was heard.
    point: Option<(u64, f64)>,
    /// The next sound owes a point: it is the first, or the first after zeros written here.
    owed: bool,
    stream: Option<Stream>,
}

impl Delivered {
    /// Writes what the source delivered, and says where it is on the clock when that is owed:
    /// the first piece, the first after zeros of ours, every few seconds — and whenever a
    /// piece is not where the last point puts it, so that a source that stopped for a moment
    /// is never placed by a point from before it stopped.
    fn take(&mut self, name: &str, at: f64, samples: &[f32], clock: &mut v2::Clock) -> std::io::Result<()> {
        let frame = self.track.frames();
        let due = match self.point {
            Some((from, then)) => {
                let expected = then + (frame - from) as f64 / f64::from(self.track.rate());
                at - then >= POINT_EVERY || (at - expected).abs() > OFF
            }
            None => true,
        };
        if self.owed || due {
            clock.point(name, frame, at)?;
            self.point = Some((frame, at));
            self.owed = false;
        }
        self.track.push(samples)
    }
}

/// Records each source as the system delivers it, with the session's clock beside (ADR-0004).
fn record_v2(sessions: &Path) -> Result<(), String> {
    let host = host();
    let (sound, heard) = mpsc::channel::<Sound>();
    let started = chrono::Local::now();
    let zero = Instant::now();
    let folder = new_folder(sessions, &started.format("%Y-%m-%d_%H-%M").to_string()).map_err(|e| e.to_string())?;

    // A source that cannot be opened is said and has no track; the other is still recorded.
    let wanted = [(host.default_input_device(), false), (host.default_output_device(), true)];
    let mut names = Vec::new();
    let mut sources = Vec::new();
    for (index, (device, playback)) in wanted.into_iter().enumerate() {
        let opened = device
            .ok_or_else(|| "the system names no default device".to_string())
            .and_then(|device| {
                open(&device, playback, index, zero, sound.clone())
                    .map(|(stream, rate, channels)| (named_as_used(&host, &device), stream, rate, channels))
            });
        match opened {
            Ok((name, stream, rate, channels)) => {
                let path = folder.join(format!("{}.wav", TRACKS[index]));
                names.push(name);
                sources.push(Some(Delivered {
                    track: v2::Track::create(&path, rate, channels).map_err(|e| format!("{}: {e}", path.display()))?,
                    watch: Watch::new(SILENT_AFTER),
                    point: None,
                    owed: true,
                    stream: Some(stream),
                }));
            }
            Err(reason) => {
                say(Event::Unavailable {
                    source: index,
                    reason: &reason,
                });
                names.push("unavailable".to_string());
                sources.push(None);
            }
        }
    }
    drop(sound);
    if sources.iter().all(Option::is_none) {
        return Err("no source could be opened: nothing to record".into());
    }
    let mut clock = v2::Clock::create(&folder.join("clock.jsonl")).map_err(|e| e.to_string())?;
    let held: Vec<(&str, &str)> = (0..2)
        .filter(|&index| sources[index].is_some())
        .map(|index| (TRACKS[index], names[index].as_str()))
        .collect();
    let stamp = |time: chrono::DateTime<chrono::Local>| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    v2::write_session(&folder, &stamp(started), None, &held).map_err(|e| e.to_string())?;
    say(Event::Started {
        folder: &folder,
        sources: [&names[0], &names[1]],
    });

    let stop = stop_asked()?;
    let mut checked = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        match heard.recv_timeout(Duration::from_millis(200)) {
            Ok((index, at, samples)) => {
                if let Some(source) = &mut sources[index] {
                    source.take(TRACKS[index], at, &samples, &mut clock).map_err(|e| e.to_string())?;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(Duration::from_millis(200)),
        }
        if checked.elapsed() >= Duration::from_secs(1) {
            let since = checked.elapsed().as_secs_f64();
            checked = Instant::now();
            let now = zero.elapsed().as_secs_f64();
            for (index, source) in sources.iter_mut().enumerate() {
                let Some(source) = source else { continue };
                // Where the track should be by now, counted from its last point — not from
                // the start: a source's own rate, over hours, is not the one it names.
                let (rate, (frame, at)) = (f64::from(source.track.rate()), source.point.unwrap_or((0, 0.0)));
                let due = frame as f64 + (now - at) * rate;
                if (source.track.frames() as f64) < due - LATE * rate {
                    // Said before it is written: killed between the two, the clock names zeros
                    // the track does not hold yet — never the track zeros nobody named.
                    let zeros = (source.track.frames(), (due - LATE / 3.0 * rate) as u64);
                    clock.silence(TRACKS[index], zeros).map_err(|e| e.to_string())?;
                    source.track.silence_to(zeros.1).map_err(|e| e.to_string())?;
                    source.owed = true;
                }
                match source.watch.after(since, source.track.loudest()) {
                    Heard::Silent => say(Event::Silent { source: index }),
                    Heard::Again => say(Event::Heard { source: index }),
                    Heard::Nothing => {}
                }
            }
        }
    }

    // The streams are closed first, then what they had already delivered is written. A track
    // ends where its source ended: nothing brings two tracks to one length, the clock places them.
    for source in sources.iter_mut().flatten() {
        source.stream = None;
    }
    while let Ok((index, at, samples)) = heard.try_recv() {
        if let Some(source) = &mut sources[index] {
            source.take(TRACKS[index], at, &samples, &mut clock).map_err(|e| e.to_string())?;
        }
    }
    let mut seconds = 0.0f64;
    for source in sources.into_iter().flatten() {
        let rate = f64::from(source.track.rate());
        seconds = seconds.max(source.track.finish().map_err(|e| e.to_string())? as f64 / rate);
    }
    v2::write_session(&folder, &stamp(started), Some(&stamp(chrono::Local::now())), &held).map_err(|e| e.to_string())?;
    say(Event::Stopped { folder: &folder, seconds });
    Ok(())
}

/// Opens a source and hands what it delivers to `sound`, each piece with when it was heard,
/// counted from `zero`. An output is opened as an input: what it delivers is what it plays.
fn open(device: &Device, playback: bool, index: usize, zero: Instant, sound: Sender<Sound>) -> Result<(Stream, u32, u16), String> {
    let config = if playback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|error| format!("no default configuration ({error})"))?;
    let (rate, channels) = (config.sample_rate(), config.channels());
    let stream = match config.sample_format() {
        SampleFormat::I8 => listen::<i8>(device, config.into(), index, zero, sound),
        SampleFormat::I16 => listen::<i16>(device, config.into(), index, zero, sound),
        SampleFormat::I32 => listen::<i32>(device, config.into(), index, zero, sound),
        SampleFormat::U8 => listen::<u8>(device, config.into(), index, zero, sound),
        SampleFormat::U16 => listen::<u16>(device, config.into(), index, zero, sound),
        SampleFormat::F32 => listen::<f32>(device, config.into(), index, zero, sound),
        SampleFormat::F64 => listen::<f64>(device, config.into(), index, zero, sound),
        other => Err(format!("sample format {other} not handled")),
    }?;
    stream.play().map_err(|error| format!("the stream could not start ({error})"))?;
    Ok((stream, rate, channels))
}

fn listen<T>(device: &Device, config: cpal::StreamConfig, index: usize, zero: Instant, sound: Sender<Sound>) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let name = SOURCES[index];
    // The stream's own time, tied once to this program's clock: every later piece is placed
    // by the instant the system handed it over, which does not wait for this program to be
    // scheduled. The delay the system says a device adds is left out: measured here, it
    // moved by whole periods from one piece to the next, and was not known for the first.
    let mut tied: Option<(cpal::StreamInstant, f64)> = None;
    device
        .build_input_stream(
            config,
            move |data: &[T], info: &cpal::InputCallbackInfo| {
                let stamp = info.timestamp();
                let (first, then) = *tied.get_or_insert_with(|| (stamp.callback, zero.elapsed().as_secs_f64()));
                let since = (stamp.callback.as_nanos() as i128 - first.as_nanos() as i128) as f64 / 1e9;
                let _ = sound.send((index, then + since, data.iter().map(|&sample| f32::from_sample(sample)).collect()));
            },
            move |error: cpal::Error| drop(writeln!(std::io::stderr(), "summit-capture: [{name}] {error}")),
            None,
        )
        .map_err(|error| format!("the stream could not be opened ({error})"))
}
