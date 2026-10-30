//! `wholeheard-capture` — records a conversation into a session's folder, with no window.
//!
//!     wholeheard-capture record <sessions-folder>     each source as delivered, until Ctrl-C, a TERM
//!                                                 signal, or "stop" on its input
//!     wholeheard-capture record <sessions-folder> --as v1     the same, in the first form
//!     wholeheard-capture devices                      what the system offers
//!
//! It says what happens as one JSON object per line on its output: `started`, `unavailable`,
//! `silent`, `heard`, `stopped`. Whoever started it reads them; nothing else is printed there.
//!
//! The sound is on disk as it is heard: killed, it leaves two files that read, and a
//! `session.json` without `stopped`.
//!
//! What it does is logged — never a sound, nor a session's path — in the user's state folder (`log`).

use std::{
    io::{BufRead, Write},
    path::Path,
    process::ExitCode,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    time::{Duration, Instant},
};

use cpal::{
    Device, FromSample, Host, Sample, SampleFormat, SizedSample, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use serde_json::json;
use tracing::{error, info, warn};
use wholeheard_capture::{Event, Heard, RATE, SOURCES, Track, Watch, call, call::Me, log, new_folder, v2, write_session};

#[cfg(target_os = "linux")]
mod passive;

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
pub(crate) type Sound = (usize, f64, Vec<f32>);
/// A source's form: frames a second, and channels.
type Form = (u32, u16);
/// Said of a microphone not opened, for it is a headset's and nothing has taken it (ADR-0015).
const WAITS: &str = "the default input is a Bluetooth headset's: the microphone waits for a call to take it";
/// Said of a headset's microphone a call holds and does not run yet.
const WAITS_NOT_RUNNING: &str = "the call holds a Bluetooth headset's microphone, not running yet: the microphone waits for it";
/// How long a recording's start waits for a source that runs to negotiate.
const FORMED_AT_THE_START: Duration = Duration::from_secs(1);
/// A source recorded from another device now: when, on the session's clock, and its name — none
/// when the system names none.
type Moved = (usize, f64, Option<String>);
/// The same, as the stream's callback tells it: whether a device is left, not yet its name.
type Move = (usize, f64, bool);

/// Says an event to whoever started the program (`contracts/recording/v1`). Nobody may be
/// listening any more — a shell that crashed, a pipe that closed: the recording goes on.
fn say(event: Event) {
    let _ = writeln!(std::io::stdout().lock(), "{}", event.line());
    // Logged too, but the levels: ten a second say nothing a reader after needs.
    let session = |folder: &Path| {
        folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    match event {
        Event::Unavailable { source, reason } => warn!(source = SOURCES[source], reason, "capture.unavailable"),
        Event::Started {
            folder,
            sources: [me, others],
        } => info!(session = %session(folder), me, others, "capture.started"),
        Event::Silent { source, nothing } => info!(source = SOURCES[source], nothing, "capture.silent"),
        Event::Heard { source } => info!(source = SOURCES[source], "capture.heard"),
        Event::Stopped { folder, seconds } => info!(session = %session(folder), seconds, "capture.stopped"),
        Event::Device { source, device } => info!(source = SOURCES[source], device, "capture.device"),
        Event::Level { .. } => {}
    }
}

fn main() -> ExitCode {
    let _run = log::start("capture");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["record", sessions] | ["record", sessions, "--as", "v2"] => record_v2(Path::new(sessions)),
        ["record", sessions, "--as", "v1"] => record(Path::new(sessions)),
        ["devices"] => devices(),
        _ => Err("usage: wholeheard-capture record <sessions-folder> [--as v1|v2] | wholeheard-capture devices".into()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            error!(detail = %reason, "capture.failed");
            eprintln!("wholeheard-capture: {reason}");
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
    let name = device
        .description()
        .map(|description| description.name().to_string())
        .or_else(|_| device.id().map(|id| id.to_string()))
        .unwrap_or_else(|_| "unnamed".into());
    // A headset's node is named for its address: never in a log or a session.
    call::without_address(&name)
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
        let node = wholeheard_capture::standing_for(&metadata(key)?, key)?;
        host.devices().ok()?.find(|other| other.id().is_ok_and(|id| id.id() == node))
    };
    standing().map_or_else(|| named(device), |stood_for| named(&stood_for))
}

/// The output the remote side is moved to, while a call plays on one other than the default
/// (D41): what the remote side is said to record, whatever the default becomes meanwhile.
static FOLLOWED: Mutex<Option<String>> = Mutex::new(None);

/// The device the system records a source from now, by the name it gives it.
fn now_named(index: usize) -> Option<String> {
    if index == 1
        && let Some(followed) = FOLLOWED.lock().unwrap().clone()
    {
        return Some(followed);
    }
    let host = host();
    let device = if index == 0 {
        host.default_input_device()
    } else {
        host.default_output_device()
    }?;
    Some(named_as_used(&host, &device))
}

/// Names each move, one after the other on a thread of its own, so that they are said in the
/// order the system made them — a name may take `pw-metadata` up to a second. It ends when no
/// stream is left to tell one, and the follower of a call (`follow`) has ended.
fn naming(moves: Receiver<Move>) -> (Receiver<Moved>, Sender<Moved>) {
    let (tell, told) = mpsc::channel();
    let follower = tell.clone();
    let run = tracing::Span::current();
    std::thread::spawn(move || {
        let _run = run.enter();
        for (index, at, left) in moves {
            if tell.send((index, at, left.then(|| now_named(index)).flatten())).is_err() {
                break;
            }
        }
    });
    (told, follower)
}

/// Follows a call to the output it plays on (D41, ADR-0011), once a second until `stop`: the
/// remote stream moved there by its `target.object`, and back to the default when the call
/// leaves it. Each move is told as the system's are, from `zero`. A graph that does not read, or
/// a stream that cannot be moved, leaves the default, said once. Each reading also says what the
/// local side should record, to `me`: nothing, when the graph does not read (ADR-0015).
fn follow(tell: Sender<Moved>, me: Option<Sender<Me>>, zero: Instant, stop: Arc<AtomicBool>) {
    let run = tracing::Span::current();
    std::thread::spawn(move || {
        let _run = run.enter();
        let ours = u64::from(std::process::id());
        let mut followed: Option<(u64, String)> = None;
        // What feeds each stream, as last read in the graph: the same on two readings in a row, it is
        // what the stream records, said even when the stream itself told no move, or was named by
        // a default not yet updated (#352) — and said again over a later stale name; a repeat of
        // the name last said is dropped (`said_moved`).
        let mut fed: Option<[Option<String>; 2]> = None;
        let mut warned = false;
        let mut refused = |detail: String| {
            if !std::mem::replace(&mut warned, true) {
                warn!(detail = %detail, "capture.not_followed");
            }
        };
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(1));
            let Some(dump) = graph() else {
                refused("the graph could not be read".into());
                continue;
            };
            let now = call::linked(&dump, ours);
            if let Some(before) = &fed {
                for index in 0..2 {
                    if now[index].is_some() && now[index] == before[index] {
                        let _ = tell.send((index, zero.elapsed().as_secs_f64(), now[index].clone()));
                    }
                }
            }
            fed = Some(now);
            if let Some(me) = &me
                && let Ok(wanted) = call::me(&dump, ours)
            {
                let _ = me.send(wanted);
            }
            let wanted = match call::output(&dump, ours) {
                Ok(wanted) => wanted,
                Err(detail) => {
                    refused(detail);
                    continue;
                }
            };
            if wanted.as_ref().map(|output| &output.name) == followed.as_ref().map(|(_, name)| name) {
                continue;
            }
            let Some(stream) = call::remote_stream(&dump, ours).or(followed.as_ref().map(|(stream, _)| *stream)) else {
                refused("the remote stream is not in the graph".into());
                continue;
            };
            if !targeted(stream, wanted.as_ref().map(|output| output.name.as_str())) {
                refused("the remote stream could not be moved".into());
                continue;
            }
            *FOLLOWED.lock().unwrap() = wanted.as_ref().map(|output| output.description.clone());
            followed = wanted.map(|output| (stream, output.name));
            if tell.send((1, zero.elapsed().as_secs_f64(), now_named(1))).is_err() {
                break;
            }
        }
        // The stream goes with the recording: its target is never left for another node to take.
        if let Some((stream, _)) = followed {
            targeted(stream, None);
        }
        *FOLLOWED.lock().unwrap() = None;
    });
}

/// Sets the stream's `target.object` to `output` in PipeWire's default metadata — or, `None`,
/// clears it, and the stream follows the default again. True once `pw-metadata` said so.
fn targeted(stream: u64, output: Option<&str>) -> bool {
    use std::process::{Command, Stdio};
    let stream = stream.to_string();
    let mut asked = Command::new("pw-metadata");
    match output {
        Some(output) => asked.args(["-n", "default", &stream, "target.object", output]),
        None => asked.args(["-n", "default", "-d", &stream, "target.object"]),
    };
    asked.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // A second at most, as `metadata` waits: the stop waits on this thread.
    let Ok(mut asked) = asked.spawn() else { return false };
    let given = Instant::now();
    loop {
        match asked.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if given.elapsed() < Duration::from_secs(1) => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = asked.kill();
                let _ = asked.wait();
                return false;
            }
        }
    }
}

/// PipeWire's graph as `pw-dump` prints it — or nothing, if it has not answered within two
/// seconds. Read as it is printed: the graph is larger than a pipe holds.
fn graph() -> Option<String> {
    use std::process::{Command, Stdio};
    let mut dumped = Command::new("pw-dump")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = dumped.stdout.take()?;
    let (give, given) = mpsc::channel();
    std::thread::spawn(move || {
        let mut printed = String::new();
        let _ = give.send(std::io::Read::read_to_string(&mut out, &mut printed).map(|_| printed));
    });
    match given.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(printed)) => {
            let _ = dumped.wait();
            Some(printed)
        }
        _ => {
            let _ = dumped.kill();
            let _ = dumped.wait();
            None
        }
    }
}

/// Says each source moved to a device other than the one it was last said on. `closing`: the
/// streams are gone, and every move still being named is waited for, so none is lost at the stop.
fn said_moved(
    moved: &Receiver<Moved>,
    closing: bool,
    last: &mut [Option<String>; 2],
    mut kept: impl FnMut(usize, f64, Option<&str>) -> Result<(), String>,
) -> Result<(), String> {
    let next = || if closing { moved.recv().ok() } else { moved.try_recv().ok() };
    while let Some((index, at, name)) = next() {
        if name == last[index] {
            continue;
        }
        say(Event::Device {
            source: index,
            device: name.as_deref(),
        });
        kept(index, at, name.as_deref())?;
        last[index] = name;
    }
    Ok(())
}

/// What `pw-metadata` prints of one key of PipeWire's default metadata — or nothing, if it is
/// not there or has not answered within a second: a recording does not wait on a name.
fn metadata(key: &str) -> Option<String> {
    use std::process::{Command, Stdio};
    let mut asked = Command::new("pw-metadata")
        .args(["-n", "default", "0", key])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let given = Instant::now();
    while asked.try_wait().ok()?.is_none() {
        if given.elapsed() > Duration::from_secs(1) {
            let _ = asked.kill();
            let _ = asked.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut printed = String::new();
    std::io::Read::read_to_string(&mut asked.stdout.take()?, &mut printed).ok()?;
    Some(printed)
}

/// What the local side should record now. Where PipeWire is not, the default, as ever. Where the
/// graph does not read, the default input's name alone decides: a headset's is not opened.
fn me_now(host: &Host) -> Me {
    if host.id().name() != "PipeWire" {
        return Me::Default;
    }
    let read = graph().and_then(|dump| call::me(&dump, u64::from(std::process::id())).ok());
    read.unwrap_or_else(unread_me)
}

/// The same, with no graph: the default input's name alone.
fn unread_me() -> Me {
    let key = "default.audio.source";
    let default = metadata(key).and_then(|printed| wholeheard_capture::standing_for(&printed, key));
    call::me_unread(default.as_deref())
}

/// Whether the default input must not be opened, as far as can be read: unread, it is not.
fn default_is_a_headset(host: &Host) -> bool {
    host.id().name() == "PipeWire"
        && graph()
            .and_then(|dump| call::default_waits(&dump).ok())
            .unwrap_or_else(|| unread_me() == Me::Wait)
}

/// What feeds the second form's tracks.
enum Feed {
    Cpal {
        #[expect(dead_code, reason = "held for what it does: dropped, it closes")]
        stream: Stream,
        /// Set before it closes: what the stream says of its end is no move of the system's.
        closing: Arc<AtomicBool>,
    },
    #[cfg(target_os = "linux")]
    Passive(passive::Passive),
}

impl Feed {
    /// False once it failed and will deliver nothing more; a cpal stream goes on and is moved.
    fn alive(&self) -> bool {
        match self {
            Feed::Cpal { .. } => true,
            #[cfg(target_os = "linux")]
            Feed::Passive(stream) => stream.alive(),
        }
    }

    /// What a passive stream negotiated, if it has yet. A cpal stream says its own when opened.
    fn form(&self) -> Option<Form> {
        match self {
            Feed::Cpal { .. } => None,
            #[cfg(target_os = "linux")]
            Feed::Passive(stream) => stream.form(),
        }
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        if let Feed::Cpal { closing, .. } = self {
            closing.store(true, Ordering::SeqCst);
        }
    }
}

/// What the recording needs to open the local side, on whichever thread does it.
#[derive(Clone)]
struct Opening {
    zero: Instant,
    sound: Sender<Sound>,
    moved: Sender<Move>,
}

/// Opens what the local side should record — in `form`, the track's, when it has one — with its name,
/// and the form it delivers, none yet on a source a passive stream waits for. A cpal stream takes
/// some seconds at most to answer when the system does not: at the start, or on a thread of its own
/// (`Following`), never in the loop that writes the sound.
fn open_me(host: &Host, wanted: &Me, form: Option<Form>, opening: &Opening) -> Result<Opened, String> {
    let cpal_on = |device: &Device, name: Option<String>| {
        let closing = Arc::new(AtomicBool::new(false));
        let (stream, rate, channels) = open(
            device,
            false,
            0,
            opening.zero,
            form,
            opening.sound.clone(),
            (opening.moved.clone(), closing.clone()),
        )?;
        Ok((
            Feed::Cpal { stream, closing },
            name.unwrap_or_else(|| named_as_used(host, device)),
            Some((rate, channels)),
        ))
    };
    match wanted {
        Me::Wait => Err(WAITS.to_string()),
        Me::Default => cpal_on(&host.default_input_device().ok_or("the system names no default device")?, None),
        #[cfg(target_os = "linux")]
        Me::Hold(call::Microphone {
            name,
            description,
            passive: Some(passive),
        }) => {
            let stream = passive::Passive::open(name, passive, form, (0, opening.zero), opening.sound.clone());
            Ok((Feed::Passive(stream), description.clone(), form))
        }
        Me::Hold(microphone) => {
            // A new host: its devices are what the graph holds now.
            let device = host_devices(&microphone.name).ok_or("the source is no longer there")?;
            cpal_on(&device, Some(microphone.description.clone()))
        }
    }
}

/// At the start, where a track's form is wanted now: a source that runs negotiates at once, and one
/// that does not has no track until it does: `Following` takes it from there.
fn formed_at_the_start(opened: Opened, stop: &AtomicBool) -> Result<(Feed, String, Form), String> {
    let (feed, name, form) = opened;
    let asked = Instant::now();
    loop {
        if let Some(form) = form.or_else(|| feed.form()) {
            return Ok((feed, name, form));
        }
        if !feed.alive() || stop.load(Ordering::SeqCst) || asked.elapsed() >= FORMED_AT_THE_START {
            return Err(WAITS_NOT_RUNNING.to_string());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The device of a node, by its name.
fn host_devices(node: &str) -> Option<Device> {
    host().devices().ok()?.find(|device| device.id().is_ok_and(|id| id.id() == node))
}

/// What opening the local side gives: what feeds it, its name, and its form when it has one yet.
type Opened = (Feed, String, Option<Form>);

/// A local stream being opened, on a thread of its own. Dropped before it answers, what that thread
/// opens is closed there.
struct Attaching {
    want: Me,
    given: Receiver<Result<Opened, String>>,
}

/// Who follows the local side, once it runs: what it is on, and what it takes to open another. The
/// recording's loop only polls it: nothing here waits on the system.
struct Following {
    on: Me,
    /// A device was last said of the local side, not `null`.
    saying: bool,
    attaching: Option<Attaching>,
    /// A passive stream with no track, its source not run yet: a track's form is what it waits for.
    unformed: Option<Opened>,
    /// What failed, how many times, and when it may be tried again.
    failed: Option<(Me, u32, Instant)>,
    refused: bool,
    opening: Opening,
    tell: Sender<Moved>,
}

impl Following {
    /// Does what the graph calls for, and never stops the recording for it: a source that cannot
    /// be opened is logged once, and tried again less and less often.
    fn heed(&mut self, want: Me, source: &mut Option<Delivered>) {
        if let Some(attaching) = &self.attaching {
            if attaching.want == want {
                return;
            }
            self.attaching = None;
        }
        if self.feed(source).is_some_and(|feed| !feed.alive()) {
            let on = std::mem::replace(&mut self.on, Me::Wait);
            self.close(source);
            self.gone();
            self.failure(on, "the stream failed");
        }
        // No stream on the default: it could not be opened at the start, and is not asked again.
        let alive = self.feed(source).map_or(self.on == Me::Default, Feed::alive);
        let step = call::step(&self.on, alive, &want);
        if step == call::Step::Keep {
            return;
        }
        if step == call::Step::Take
            && let Some((failed, _, again)) = &self.failed
            && *failed == want
            && Instant::now() < *again
        {
            return;
        }
        // Closed first, always: it lets go before anything else is opened.
        self.close(source);
        self.on = Me::Wait;
        if step == call::Step::Let {
            info!("capture.me_waits");
            self.gone();
            return;
        }
        let form = source.as_ref().map(|source| (source.track.rate(), source.track.channels()));
        let (give, given) = mpsc::channel();
        let (asked, opening, run) = (want.clone(), self.opening.clone(), tracing::Span::current());
        std::thread::spawn(move || {
            let _run = run.enter();
            let _ = give.send(open_me(&host(), &asked, form, &opening));
        });
        self.attaching = Some(Attaching { want, given });
    }

    /// Takes the answer of what was being opened, and the form of what waited for one, if they came.
    fn settle(&mut self, source: &mut Option<Delivered>, held: &mut Vec<v2::Held>, clock: &mut v2::Clock, folder: &Path) {
        let answer = self.attaching.as_ref().and_then(|attaching| match attaching.given.try_recv() {
            Ok(answer) => Some(answer),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("what opens it ended".to_string())),
        });
        if let Some(answer) = answer
            && let Some(Attaching { want, .. }) = self.attaching.take()
        {
            match answer {
                Ok((feed, name, form)) => {
                    self.on = want;
                    if let Some(source) = source.as_mut() {
                        // Under a track, in its form: on it from now, whatever the source does.
                        source.stream = Some(feed);
                        self.taken(name);
                    } else {
                        self.unformed = Some((feed, name, form));
                    }
                }
                Err(detail) => self.failure(want, &detail),
            }
        }
        let form = self.unformed.as_ref().and_then(|(feed, _, form)| form.or_else(|| feed.form()));
        let Some(form) = form else { return };
        let Some((feed, name, _)) = self.unformed.take() else { return };
        info!(source = SOURCES[0], device = %name, rate = form.0, channels = form.1, "capture.opened");
        let at = self.opening.zero.elapsed().as_secs_f64();
        match Delivered::after_waiting(folder, form, at, clock) {
            Ok(mut track) => {
                track.stream = Some(feed);
                *source = Some(track);
                held.push(v2::Held::new(TRACKS[0], &name));
                self.taken(name);
            }
            Err(error) => {
                let on = std::mem::replace(&mut self.on, Me::Wait);
                self.failure(on, &error.to_string());
            }
        }
    }

    /// What feeds the local side, track or no track.
    fn feed<'a>(&'a self, source: &'a Option<Delivered>) -> Option<&'a Feed> {
        let unformed = self.unformed.as_ref().map(|(feed, ..)| feed);
        unformed.or_else(|| source.as_ref()?.stream.as_ref())
    }

    fn taken(&mut self, name: String) {
        self.failed = None;
        self.refused = false;
        self.saying = true;
        let _ = self.tell.send((0, self.opening.zero.elapsed().as_secs_f64(), Some(name)));
    }

    fn close(&mut self, source: &mut Option<Delivered>) {
        self.unformed = None;
        if let Some(source) = source.as_mut() {
            source.stream = None;
        }
    }

    /// `what` could not be opened, or did not last: logged once, and tried again later.
    fn failure(&mut self, what: Me, detail: &str) {
        let tries = self
            .failed
            .as_ref()
            .filter(|(failed, ..)| *failed == what)
            .map_or(0, |(_, tries, _)| *tries)
            + 1;
        self.failed = Some((what, tries, Instant::now() + call::retry_after(tries)));
        self.gone();
        if !std::mem::replace(&mut self.refused, true) {
            warn!(detail = %call::without_address(detail), "capture.me_not_attached");
        }
    }

    /// The local side has no device now: said, if one was.
    fn gone(&mut self) {
        if std::mem::replace(&mut self.saying, false) {
            let _ = self.tell.send((0, self.opening.zero.elapsed().as_secs_f64(), None));
        }
    }
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
    let (moved, moves) = mpsc::channel::<Move>();
    let (moves, follower) = naming(moves);
    let started = chrono::Local::now();
    let zero = Instant::now();
    let folder = new_folder(sessions, &started.format("%Y-%m-%d_%H-%M").to_string()).map_err(|e| e.to_string())?;

    // "me" is the default microphone; "others" is what the default output plays. Each is
    // opened on its own: a source that cannot be opened is written as silence and said, the
    // other is still recorded.
    // A headset's virtual input is never opened actively (ADR-0015): this form does not follow a
    // call's microphone, and records none there.
    let headset = default_is_a_headset(&host);
    let wanted = [
        (host.default_input_device().filter(|_| !headset), false),
        (host.default_output_device(), true),
    ];
    let mut names = Vec::new();
    let mut sources = Vec::new();
    for (index, (device, playback)) in wanted.into_iter().enumerate() {
        let path = folder.join(format!("{}.wav", SOURCES[index]));
        let opened = device
            .ok_or_else(|| {
                if index == 0 && headset {
                    WAITS
                } else {
                    "the system names no default device"
                }
                .to_string()
            })
            .and_then(|device| {
                open(&device, playback, index, zero, None, sound.clone(), (moved.clone(), Arc::default()))
                    .map(|(stream, rate, channels)| (named_as_used(&host, &device), stream, rate, channels))
            });
        let (name, stream, rate, channels) = match opened {
            Ok((name, stream, rate, channels)) => {
                info!(source = SOURCES[index], device = %name, rate, channels, "capture.opened");
                (name, Some(stream), rate, channels)
            }
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
            // The file's name, never the session's path: a sentence that fails is logged (ADR-0007).
            track: Track::create(&path, rate, channels).map_err(|e| format!("{}.wav: {e}", SOURCES[index]))?,
            watch: Watch::new(SILENT_AFTER),
            _stream: stream,
        });
    }
    drop((sound, moved));
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
    // A call heard on another output than the default is followed there, where PipeWire is.
    if host.id().name() == "PipeWire" {
        follow(follower, None, zero, stop.clone());
    } else {
        drop(follower);
    }

    let clock = Instant::now();
    let mut checked = clock;
    let mut metered = clock;
    let mut last = [Some(names[0].clone()), Some(names[1].clone())];
    while !stop.load(Ordering::SeqCst) {
        said_moved(&moves, false, &mut last, |_, _, _| Ok(()))?;
        match heard.recv_timeout(Duration::from_millis(50)) {
            Ok((index, _, samples)) => sources[index].track.push(&samples).map_err(|e| e.to_string())?,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(Duration::from_millis(200)),
        }
        // A meter's levels, ten times a second.
        if metered.elapsed() >= Duration::from_millis(100) {
            metered = Instant::now();
            for (index, source) in sources.iter_mut().enumerate() {
                // A source that could not be opened is not being recorded: it has no level to
                // say, as in v2 (recording/v1).
                if source._stream.is_none() {
                    continue;
                }
                say(Event::Level {
                    source: index,
                    level: source.track.level(),
                });
            }
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
                match source.watch.after_stirred(since, source.track.loudest(), source.track.stirred()) {
                    Heard::Silent => say(Event::Silent {
                        source: index,
                        nothing: source.watch.nothing(),
                    }),
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
    // A move the system made in the last moment is said too.
    said_moved(&moves, true, &mut last, |_, _, _| Ok(()))?;
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
    /// Zeros are being written for it: logged once a gap, the first from the start included.
    mute: bool,
    stream: Option<Feed>,
}

impl Delivered {
    /// The local track of a microphone that waited: made when it can be, in the form the source
    /// negotiated, and on the clock by zeros that are said (session-audio/v2).
    fn after_waiting(folder: &Path, (rate, channels): Form, at: f64, clock: &mut v2::Clock) -> std::io::Result<Delivered> {
        let mut track = v2::Track::create(&folder.join(format!("{}.wav", TRACKS[0])), rate, channels)?;
        let zeros = (0, (at * f64::from(track.rate())) as u64);
        clock.silence(TRACKS[0], zeros)?;
        track.silence_to(zeros.1)?;
        Ok(Delivered {
            track,
            watch: Watch::new(SILENT_AFTER),
            point: None,
            owed: true,
            // Those zeros are not a source gone mute.
            mute: true,
            stream: None,
        })
    }

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
        self.mute = false;
        self.track.push(samples)
    }
}

/// Records each source as the system delivers it, with the session's clock beside (ADR-0004).
fn record_v2(sessions: &Path) -> Result<(), String> {
    let host = host();
    let (sound, heard) = mpsc::channel::<Sound>();
    let (moved, moves) = mpsc::channel::<Move>();
    let (moves, follower) = naming(moves);
    let started = chrono::Local::now();
    let zero = Instant::now();
    let folder = new_folder(sessions, &started.format("%Y-%m-%d_%H-%M").to_string()).map_err(|e| e.to_string())?;

    // A source that cannot be opened is said and has no track; the other is still recorded. The
    // local side is what the graph says (ADR-0015): it may wait, and gets its track when it is taken.
    let stop = stop_asked()?;
    let opening = Opening { zero, sound, moved };
    let me_first = me_now(&host);
    let mut me_opened = false;
    let mut names = Vec::new();
    let mut sources = Vec::new();
    for index in 0..2 {
        let opened = if index == 0 {
            open_me(&host, &me_first, None, &opening)
                .and_then(|opened| formed_at_the_start(opened, &stop))
                .map(|(feed, name, (rate, channels))| (name, feed, rate, channels))
        } else {
            host.default_output_device()
                .ok_or_else(|| "the system names no default device".to_string())
                .and_then(|device| {
                    let closing = Arc::new(AtomicBool::new(false));
                    open(
                        &device,
                        true,
                        index,
                        zero,
                        None,
                        opening.sound.clone(),
                        (opening.moved.clone(), closing.clone()),
                    )
                    .map(|(stream, rate, channels)| (named_as_used(&host, &device), Feed::Cpal { stream, closing }, rate, channels))
                })
        };
        match opened {
            Ok((name, stream, rate, channels)) => {
                info!(source = SOURCES[index], device = %name, rate, channels, "capture.opened");
                me_opened |= index == 0;
                let path = folder.join(format!("{}.wav", TRACKS[index]));
                names.push(name);
                sources.push(Some(Delivered {
                    track: v2::Track::create(&path, rate, channels).map_err(|e| format!("{}.wav: {e}", TRACKS[index]))?,
                    watch: Watch::new(SILENT_AFTER),
                    point: None,
                    owed: true,
                    mute: false,
                    stream: Some(stream),
                }));
            }
            Err(reason) => {
                say(Event::Unavailable {
                    source: index,
                    reason: &call::without_address(&reason),
                });
                names.push("unavailable".to_string());
                sources.push(None);
            }
        }
    }
    if sources.iter().all(Option::is_none) {
        return Err("no source could be opened: nothing to record".into());
    }
    let mut clock = v2::Clock::create(&folder.join("clock.jsonl")).map_err(|e| e.to_string())?;
    let mut held: Vec<v2::Held> = (0..2)
        .filter(|&index| sources[index].is_some())
        .map(|index| v2::Held::new(TRACKS[index], &names[index]))
        .collect();
    let stamp = |time: chrono::DateTime<chrono::Local>| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    v2::write_session(&folder, &stamp(started), None, &held).map_err(|e| e.to_string())?;
    say(Event::Started {
        folder: &folder,
        sources: [&names[0], &names[1]],
    });

    let (wants, wanted_me) = mpsc::channel::<Me>();
    let mut following = Following {
        // A default that could not be opened stays as it was; anything else is asked again.
        on: if me_opened || me_first == Me::Default { me_first } else { Me::Wait },
        saying: me_opened,
        attaching: None,
        unformed: None,
        failed: None,
        refused: false,
        opening,
        tell: follower.clone(),
    };
    // A call heard on another output than the default is followed there, where PipeWire is; the
    // microphone it holds, too.
    if host.id().name() == "PipeWire" {
        follow(follower, Some(wants), zero, stop.clone());
    } else {
        drop((follower, wants));
    }
    let mut checked = Instant::now();
    let mut metered = checked;
    let mut last = [Some(names[0].clone()), Some(names[1].clone())];
    while !stop.load(Ordering::SeqCst) {
        // The session keeps each device a track was recorded from after its first.
        said_moved(&moves, false, &mut last, |index, at, name| {
            let Some(track) = held.iter_mut().find(|held| held.track == TRACKS[index]) else {
                return Ok(());
            };
            track.moved(at, name);
            v2::write_session(&folder, &stamp(started), None, &held).map_err(|e| e.to_string())
        })?;
        while let Ok(want) = wanted_me.try_recv() {
            following.heed(want, &mut sources[0]);
        }
        following.settle(&mut sources[0], &mut held, &mut clock, &folder);
        match heard.recv_timeout(Duration::from_millis(50)) {
            Ok((index, at, samples)) => {
                if let Some(source) = &mut sources[index] {
                    source.take(TRACKS[index], at, &samples, &mut clock).map_err(|e| e.to_string())?;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(Duration::from_millis(200)),
        }
        // A meter's levels, ten times a second.
        if metered.elapsed() >= Duration::from_millis(100) {
            metered = Instant::now();
            for (index, source) in sources.iter_mut().enumerate() {
                // A source that could not be opened has no level to say.
                let Some(source) = source else { continue };
                say(Event::Level {
                    source: index,
                    level: source.track.level(),
                });
            }
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
                    // Once a gap: zeros follow each second while the source stays mute.
                    if !source.mute {
                        warn!(track = TRACKS[index], at = zeros.0 as f64 / rate, "capture.zeros");
                        source.mute = true;
                    }
                    clock.silence(TRACKS[index], zeros).map_err(|e| e.to_string())?;
                    source.track.silence_to(zeros.1).map_err(|e| e.to_string())?;
                    source.owed = true;
                }
                match source.watch.after_stirred(since, source.track.loudest(), source.track.stirred()) {
                    Heard::Silent => say(Event::Silent {
                        source: index,
                        nothing: source.watch.nothing(),
                    }),
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
    // What it took to open another goes too: the moves' channel ends with it.
    drop(following);
    // A move the system made in the last moment is said, and kept, too.
    said_moved(&moves, true, &mut last, |index, at, name| {
        if let Some(track) = held.iter_mut().find(|held| held.track == TRACKS[index]) {
            track.moved(at, name);
        }
        Ok(())
    })?;
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
fn open(
    device: &Device,
    playback: bool,
    index: usize,
    zero: Instant,
    form: Option<Form>,
    sound: Sender<Sound>,
    moved: (Sender<Move>, Arc<AtomicBool>),
) -> Result<(Stream, u32, u16), String> {
    let config = if playback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|error| format!("no default configuration ({error})"))?;
    // A track that already has a form keeps it: the system converts, as when a device changes.
    let (rate, channels) = form.unwrap_or((config.sample_rate(), config.channels()));
    let format = config.sample_format();
    let config = cpal::StreamConfig {
        sample_rate: rate,
        channels,
        ..config.into()
    };
    let stream = match format {
        SampleFormat::I8 => listen::<i8>(device, config, index, zero, sound, moved),
        SampleFormat::I16 => listen::<i16>(device, config, index, zero, sound, moved),
        SampleFormat::I32 => listen::<i32>(device, config, index, zero, sound, moved),
        SampleFormat::U8 => listen::<u8>(device, config, index, zero, sound, moved),
        SampleFormat::U16 => listen::<u16>(device, config, index, zero, sound, moved),
        SampleFormat::F32 => listen::<f32>(device, config, index, zero, sound, moved),
        SampleFormat::F64 => listen::<f64>(device, config, index, zero, sound, moved),
        other => Err(format!("sample format {other} not handled")),
    }?;
    stream.play().map_err(|error| format!("the stream could not start ({error})"))?;
    Ok((stream, rate, channels))
}

fn listen<T>(
    device: &Device,
    config: cpal::StreamConfig,
    index: usize,
    zero: Instant,
    sound: Sender<Sound>,
    (moved, closing): (Sender<Move>, Arc<AtomicBool>),
) -> Result<Stream, String>
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
    // The system's thread logs under this run.
    let run = tracing::Span::current();
    device
        .build_input_stream(
            config,
            move |data: &[T], info: &cpal::InputCallbackInfo| {
                let stamp = info.timestamp();
                let (first, then) = *tied.get_or_insert_with(|| (stamp.callback, zero.elapsed().as_secs_f64()));
                let since = (stamp.callback.as_nanos() as i128 - first.as_nanos() as i128) as f64 / 1e9;
                let _ = sound.send((index, then + since, data.iter().map(|&sample| f32::from_sample(sample)).collect()));
            },
            move |error: cpal::Error| {
                // The system moved the stream to another device, or has none for it now: the
                // track goes on, and the device is named — off this thread, which the system's
                // sound waits on.
                if closing.load(Ordering::SeqCst) {
                    return;
                }
                let Some(left) = wholeheard_capture::moved(error.kind()) else {
                    let error = call::without_address(&error.to_string());
                    run.in_scope(|| warn!(source = name, detail = %error, "capture.stream_error"));
                    drop(writeln!(std::io::stderr(), "wholeheard-capture: [{name}] {error}"));
                    return;
                };
                let _ = moved.send((index, zero.elapsed().as_secs_f64(), left));
            },
            None,
        )
        .map_err(|error| format!("the stream could not be opened ({error})"))
}
