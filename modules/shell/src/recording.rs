//! A recording, supervised: the program that records is started, listened to as
//! `contracts/recording/v1` describes it, and told to stop. The shell records nothing itself —
//! and a recording goes on if the shell does not.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};

use serde_json::Value;
use tracing::{error, info, warn};

/// One of a session's two sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Me,
    Others,
}

/// What a recording made known, in the order it happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Said {
    /// A source could not be opened: its file will be silence, the other is still recorded.
    Unavailable(Source, String),
    /// The recording runs, in this session's folder, from this microphone and this output — as
    /// the system names each, or nothing when it did not say which one.
    Started(PathBuf, Option<String>, Option<String>),
    /// A source has given no sound for a while.
    Silent(Source),
    /// The microphone has sent nothing at all for a while, exact zeros — muted on the device
    /// itself, or not ready — which no quiet room gives (#361). An output sends exact zeros
    /// whenever nothing plays: the meeting's side stays `Silent`.
    Nothing,
    /// A source that was silent gives sound again.
    Heard(Source),
    /// How loud a source is now, from 0 to 1, for a meter.
    Level(Source, f32),
    /// A source is recorded from another device now: its name, as `Started` names one (D31).
    Moved(Source, Option<String>),
    /// The system names no device for a source: nothing of it is recorded until one comes.
    Lost(Source),
    /// The recording was stopped: its folder, and its length in seconds.
    Stopped(PathBuf, f64),
    /// The program ended without having said it stopped: its own sentence, when it gave one.
    Failed(String),
}

/// A line of the program's output as the contract describes it — or nothing: a line that is
/// no event, or an event this version does not know, is passed over.
pub fn read(line: &str) -> Option<Said> {
    let event: Value = serde_json::from_str(line).ok()?;
    let source = || match event["source"].as_str() {
        Some("me") => Some(Source::Me),
        Some("others") => Some(Source::Others),
        _ => None,
    };
    let folder = || event["folder"].as_str().map(PathBuf::from);
    match event["event"].as_str()? {
        "unavailable" => Some(Said::Unavailable(
            source()?,
            event["reason"].as_str().unwrap_or_default().to_string(),
        )),
        "started" => Some(Said::Started(
            folder()?,
            event["sources"]["me"].as_str().and_then(device),
            event["sources"]["others"].as_str().and_then(device),
        )),
        "silent" => match source()? {
            Source::Me if event["nothing"] == true => Some(Said::Nothing),
            source => Some(Said::Silent(source)),
        },
        "heard" => Some(Said::Heard(source()?)),
        // Another device, or none: a name, or null — anything else is no event (recording/v1).
        "device" => match &event["device"] {
            serde_json::Value::Null if event.get("device").is_some() => Some(Said::Lost(source()?)),
            serde_json::Value::String(name) => Some(Said::Moved(source()?, device(name))),
            _ => None,
        },
        "level" => Some(Said::Level(source()?, event["level"].as_f64()?.clamp(0.0, 1.0) as f32)),
        "stopped" => Some(Said::Stopped(folder()?, event["seconds"].as_f64().unwrap_or(0.0))),
        _ => None,
    }
}

/// What each side is recorded from now, as the recording said it: the microphone, then the
/// output — none when it named none, or has none (D33).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Devices([Option<String>; 2]);

impl Devices {
    pub fn heard(&mut self, said: &Said) {
        let side = |source: &Source| usize::from(*source == Source::Others);
        match said {
            Said::Started(_, me, others) => self.0 = [me.clone(), others.clone()],
            Said::Moved(source, name) => self.0[side(source)] = name.clone(),
            Said::Lost(source) | Said::Unavailable(source, _) => self.0[side(source)] = None,
            _ => {}
        }
    }

    pub fn names(&self) -> [Option<String>; 2] {
        self.0.clone()
    }
}

/// Above this, a level is sound: about -40 dB of the loudest a side can be, under a voice and
/// over the nothing of a side at rest (`recording/v1` says 0 is silence, and no more). The silence
/// a user is told of stays capture's own measure (`silent`).
const ABOVE_SILENCE: f32 = 0.33;

/// What the small window says of a side being heard (D49).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    /// Nothing heard yet of what it is recorded from now.
    #[default]
    Listening,
    Heard,
    /// Capture says it has given no sound for a while.
    Silent,
    /// Capture says the microphone sends nothing at all (`Said::Nothing`).
    Nothing,
    /// No device: nothing of it is recorded now, so there is nothing to check.
    Waiting,
}

/// Each side's check, the microphone then the output, as the recording said them.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Checks {
    now: [Check; 2],
    /// A silence capture said: it says it once per silence (`recording/v1`), and neither a device
    /// changed nor a side back from waiting makes it say it again.
    silence: [Option<Check>; 2],
}

impl Checks {
    pub fn heard(&mut self, said: &Said) {
        let side = |source: &Source| usize::from(*source == Source::Others);
        let (at, check) = match said {
            Said::Unavailable(source, _) | Said::Lost(source) => (side(source), Check::Waiting),
            Said::Moved(source, _) => (side(source), self.silence[side(source)].unwrap_or(Check::Listening)),
            // The zeros a recording keeps in the place of a side with no device are remembered
            // for when it has one, and no news until then.
            Said::Silent(source) => {
                self.silence[side(source)] = Some(Check::Silent);
                if self.now[side(source)] == Check::Waiting {
                    return;
                }
                (side(source), Check::Silent)
            }
            Said::Nothing => {
                self.silence[0] = Some(Check::Nothing);
                if self.now[0] == Check::Waiting {
                    return;
                }
                (0, Check::Nothing)
            }
            Said::Heard(source) => {
                self.silence[side(source)] = None;
                if self.now[side(source)] == Check::Waiting {
                    return;
                }
                (side(source), Check::Heard)
            }
            Said::Level(source, level) if *level > ABOVE_SILENCE && self.now[side(source)] == Check::Listening => {
                (side(source), Check::Heard)
            }
            _ => return,
        };
        self.now[at] = check;
    }

    pub fn now(&self) -> [Check; 2] {
        self.now
    }
}

/// A device's name made fit to show: on one line, its spaces collapsed, and short. A name
/// that says nothing of which device it is gives none: a desktop's default named for itself,
/// the capture's own words for a device it could not open or name.
fn device(name: &str) -> Option<String> {
    let words: Vec<&str> = name
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|word| !word.is_empty())
        .collect();
    let name = words.join(" ");
    if ["", "unavailable", "unnamed", "default_input"].contains(&name.as_str()) {
        return None;
    }
    Some(match name.char_indices().nth(80) {
        Some((cut, _)) => format!("{}…", &name[..cut]),
        None => name,
    })
}

/// The lines of a stream, whatever bytes they hold: what is not text is read with its
/// unreadable bytes replaced, never taken for the stream's end — a reader that let go there
/// would close the stream under a program that is still recording.
fn lines(stream: impl Read) -> impl Iterator<Item = String> {
    let lines = BufReader::new(stream).split(b'\n').map_while(Result::ok);
    lines.map(|line| String::from_utf8_lossy(&line).trim_end_matches('\r').to_string())
}

/// A recording in progress: the program, and what it says. Dropped, it stops nothing: the
/// program records on, and is stopped by a signal.
pub struct Recording {
    program: Child,
    input: ChildStdin,
}

impl Recording {
    /// Starts the program that records, into a new session of this folder of sessions. What
    /// it says comes apart from the hold on it, so that one may listen while another stops
    /// it: it ends on `Stopped` or `Failed`, then closes.
    pub fn start(program: &Path, sessions: &Path) -> io::Result<(Self, Receiver<Said>)> {
        let (trace, making) = crate::log::making();
        let _making = making.enter();
        info!("shell.recording");
        let mut program = Command::new(program)
            .arg("record")
            .arg(sessions)
            .env("WHOLEHEARD_TRACE", &trace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .inspect_err(|error| error!(detail = %error, "shell.recording_failed"))?;
        let (input, output, errors) = (program.stdin.take(), program.stdout.take(), program.stderr.take());
        let (Some(input), Some(output), Some(errors)) = (input, output, errors) else {
            return Err(io::Error::other("the program that records could not be listened to"));
        };
        // Its error stream is read as it comes, so that it never fills: the last sentence is
        // what it says of a failure.
        let complaint = thread::spawn(move || lines(errors).filter(|line| !line.trim().is_empty()).last().unwrap_or_default());
        let (tell, said) = mpsc::channel();
        let (making, sessions) = (making.clone(), sessions.to_path_buf());
        thread::spawn(move || {
            let _making = making.enter();
            // Read to its end, so that the program is never held up. "stopped" is the last word:
            // whoever listens hears the end there, though the program may take its time to go.
            let mut tell = Some(tell);
            for event in lines(output).filter_map(|line| read(&line)) {
                let last = matches!(event, Said::Stopped(..));
                match &event {
                    Said::Started(folder, ..) => info!(session = %crate::transcription::named(folder), "shell.recording_started"),
                    Said::Unavailable(source, why) => warn!(source = ?source, detail = %why, "shell.recording_unavailable"),
                    Said::Moved(source, _) => info!(source = ?source, "shell.recording_moved"),
                    Said::Lost(source) => warn!(source = ?source, "shell.recording_lost"),
                    Said::Stopped(_, seconds) => info!(seconds, "shell.recording_stopped"),
                    _ => {}
                }
                if let Some(listener) = if last { tell.take() } else { tell.clone() } {
                    let _ = listener.send(event);
                }
            }
            let Some(tell) = tell else { return };
            // Its output is closed and it never said "stopped": it failed, and its error stream
            // holds why once it has ended.
            let why = complaint.join().unwrap_or_default().trim().to_string();
            error!(detail = %crate::log::without(&why, Some(&sessions)), "shell.recording_failed");
            let _ = tell.send(Said::Failed(why));
        });
        Ok((Self { program, input }, said))
    }

    /// Tells the program to stop: it writes its files whole, says `stopped`, and ends.
    pub fn stop(&mut self) -> io::Result<()> {
        self.input.write_all(b"stop\n")?;
        self.input.flush()
    }

    /// The program's number for the system: what stops a recording whose shell is gone.
    pub fn id(&self) -> u32 {
        self.program.id()
    }

    /// Waits for the program to end, so that nothing of it is left in the system's table of
    /// programs: for whoever heard its output close.
    pub fn wait(mut self) {
        let _ = self.program.wait();
    }

    /// Whether the program has ended.
    pub fn ended(&mut self) -> bool {
        !matches!(self.program.try_wait(), Ok(None))
    }
}
