//! What `capture` promises, with no audio device in sight: a session's folder as the
//! `session-audio/v1` contract describes it (contracts/session-audio/v1).
//!
//! A [`Track`] takes sound as a source delivers it — any rate, any number of channels, resting
//! wherever the hardware rests — and appends it to a file that is one channel, 16-bit,
//! 16,000 samples a second, centred, and on disk as soon as it is heard.

pub mod v2;

use std::{
    fs::{self, File},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde_json::json;

/// Samples per second of every audio file of a session.
pub const RATE: u32 = 16_000;
/// The contract's name, as `session.json` carries it.
pub const CONTRACT: &str = "session-audio/v1";
/// Below this share of full scale, a source is silent.
pub const SILENCE: f32 = 0.001;

/// The sources, as what the program says names them: the microphone, then what the machine plays.
pub const SOURCES: [&str; 2] = ["me", "others"];

/// What the program says while it records (`contracts/recording/v1`): one line of its output
/// each. Both forms of a session say the same lines, and say them only through these.
pub enum Event<'a> {
    /// A source — by its place in [`SOURCES`] — that could not be opened, and why.
    Unavailable {
        source: usize,
        reason: &'a str,
    },
    /// The recording runs: its folder, and the name the system gave each source, or `unavailable`.
    Started {
        folder: &'a Path,
        sources: [&'a str; 2],
    },
    Silent {
        source: usize,
    },
    Heard {
        source: usize,
    },
    /// How loud a source is now, from 0 to 1, for a meter.
    Level {
        source: usize,
        level: f32,
    },
    /// The recording is whole: its folder, and its length in seconds.
    Stopped {
        folder: &'a Path,
        seconds: f64,
    },
}

impl Event<'_> {
    /// The line, as a JSON object.
    pub fn line(&self) -> serde_json::Value {
        match *self {
            Event::Unavailable { source, reason } => json!({ "event": "unavailable", "source": SOURCES[source], "reason": reason }),
            Event::Started {
                folder,
                sources: [me, others],
            } => {
                json!({ "event": "started", "folder": folder, "sources": { "me": me, "others": others } })
            }
            Event::Silent { source } => json!({ "event": "silent", "source": SOURCES[source] }),
            Event::Heard { source } => json!({ "event": "heard", "source": SOURCES[source] }),
            // Rounded to a hundredth: a meter shows no more, and a line stays short.
            Event::Level { source, level } => {
                json!({ "event": "level", "source": SOURCES[source], "level": (f64::from(level.clamp(0.0, 1.0)) * 100.0).round() / 100.0 })
            }
            Event::Stopped { folder, seconds } => json!({ "event": "stopped", "folder": folder, "seconds": seconds }),
        }
    }
}

/// A peak, as a share of full scale, on a meter's scale: decibels from -60 (0) to 0 (1).
pub fn level(peak: f32) -> f32 {
    if peak <= 0.001 {
        return 0.0;
    }
    ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

const HEADER: u64 = 44;
/// A length that says "not known yet": players then read to the end of the file.
const UNKNOWN: u32 = u32::MAX;
const TAPS: usize = 97;
/// The opening of a source is brought in over this long: a microphone opens on a click.
const FADE_SECONDS: f64 = 0.1;

/// One audio file of a session, growing.
pub struct Track {
    file: File,
    written: u64,
    channels: usize,
    step: f64,
    taps: Vec<f32>,
    history: Vec<f32>,
    cursor: usize,
    seen: u64,
    next: f64,
    before: f32,
    rest: Option<(f32, f32)>,
    keep: f32,
    fade: u64,
    loudest: f32,
    /// The same, for the meter: read ten times a second, apart from the silence's watch.
    meter: f32,
}

impl Track {
    /// Creates the file with a header a reader can already use.
    pub fn create(path: &Path, rate: u32, channels: u16) -> io::Result<Self> {
        let mut file = File::create(path)?;
        file.write_all(&header(UNKNOWN, UNKNOWN))?;
        let rate = f64::from(rate.max(1));
        Ok(Self {
            file,
            written: 0,
            channels: usize::from(channels.max(1)),
            step: rate / f64::from(RATE),
            taps: low_pass(rate),
            history: vec![0.0; TAPS],
            cursor: 0,
            seen: 0,
            next: 0.0,
            before: 0.0,
            rest: None,
            // One pole at 20 Hz: what a source rests at is taken out, a voice is not.
            keep: (1.0 - std::f64::consts::TAU * 20.0 / rate) as f32,
            fade: (FADE_SECONDS * f64::from(RATE)) as u64,
            loudest: 0.0,
            meter: 0.0,
        })
    }

    /// Appends what a source just delivered, interleaved; it is on disk when this returns.
    pub fn push(&mut self, interleaved: &[f32]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(interleaved.len() / self.channels * 2 / self.step.max(1.0) as usize + 8);
        for frame in interleaved.chunks_exact(self.channels) {
            let mixed = frame.iter().sum::<f32>() / self.channels as f32;
            let centred = self.centre(mixed);
            let filtered = self.filter(centred);
            // Output samples fall between two filtered ones: the line between them is read.
            while self.next <= self.seen as f64 {
                let along = (self.next - (self.seen as f64 - 1.0)) as f32;
                let mut value = self.before + along * (filtered - self.before);
                let at = self.written + (bytes.len() / 2) as u64;
                if at < self.fade {
                    value *= at as f32 / self.fade as f32;
                }
                self.loudest = self.loudest.max(value.abs());
                self.meter = self.meter.max(value.abs());
                bytes.extend_from_slice(&((value.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16).to_le_bytes());
                self.next += self.step;
            }
            self.before = filtered;
            self.seen += 1;
        }
        self.file.write_all(&bytes)?;
        self.written += (bytes.len() / 2) as u64;
        Ok(())
    }

    /// Writes zeros until the file holds `samples`: a source that delivers nothing stays on
    /// the session's clock. Never shortens.
    pub fn pad_to(&mut self, samples: u64) -> io::Result<()> {
        if samples > self.written {
            self.file.write_all(&vec![0u8; ((samples - self.written) * 2) as usize])?;
            self.written = samples;
        }
        Ok(())
    }

    /// Samples in the file.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// The loudest sample written since the last call, as a share of full scale.
    pub fn loudest(&mut self) -> f32 {
        std::mem::take(&mut self.loudest)
    }

    /// How loud it was since the last call, for a meter: the loudest sound on a scale of
    /// decibels, from 0 at -60 dB of full scale or below to 1 at full scale.
    pub fn level(&mut self) -> f32 {
        crate::level(std::mem::take(&mut self.meter))
    }

    /// Writes the true lengths in the header. A file never finished still reads: see `UNKNOWN`.
    pub fn finish(mut self) -> io::Result<u64> {
        let data = u32::try_from(self.written * 2).unwrap_or(UNKNOWN);
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header(data.saturating_add(36), data))?;
        self.file.sync_all()?;
        Ok(self.written)
    }

    fn centre(&mut self, sample: f32) -> f32 {
        // Starting from the first sample itself: a source resting far from zero would
        // otherwise open on a step as loud as its offset.
        let (last_in, last_out) = self.rest.unwrap_or((sample, 0.0));
        let out = sample - last_in + self.keep * last_out;
        self.rest = Some((sample, out));
        out
    }

    fn filter(&mut self, sample: f32) -> f32 {
        if self.step <= 1.0 {
            return sample;
        }
        self.history[self.cursor] = sample;
        self.cursor = (self.cursor + 1) % TAPS;
        let (recent, older) = self.history.split_at(self.cursor);
        older.iter().chain(recent).zip(&self.taps).map(|(sample, tap)| sample * tap).sum()
    }
}

/// A windowed-sinc low-pass just under half of [`RATE`]: what would fold back is removed
/// before samples are dropped.
fn low_pass(rate: f64) -> Vec<f32> {
    let cutoff = 0.45 * f64::from(RATE) / rate;
    let middle = (TAPS - 1) as f64 / 2.0;
    let mut taps: Vec<f64> = (0..TAPS)
        .map(|n| {
            let x = n as f64 - middle;
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (std::f64::consts::TAU * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            let window = 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / (TAPS - 1) as f64).cos();
            sinc * window
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    taps.iter_mut().for_each(|tap| *tap /= sum);
    taps.into_iter().map(|tap| tap as f32).collect()
}

fn header(riff: u32, data: u32) -> [u8; HEADER as usize] {
    let mut bytes = [0u8; HEADER as usize];
    bytes[0..4].copy_from_slice(b"RIFF");
    bytes[4..8].copy_from_slice(&riff.to_le_bytes());
    bytes[8..16].copy_from_slice(b"WAVEfmt ");
    bytes[16..20].copy_from_slice(&16u32.to_le_bytes());
    bytes[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
    bytes[22..24].copy_from_slice(&1u16.to_le_bytes()); // one channel
    bytes[24..28].copy_from_slice(&RATE.to_le_bytes());
    bytes[28..32].copy_from_slice(&(RATE * 2).to_le_bytes());
    bytes[32..34].copy_from_slice(&2u16.to_le_bytes());
    bytes[34..36].copy_from_slice(&16u16.to_le_bytes());
    bytes[36..40].copy_from_slice(b"data");
    bytes[40..44].copy_from_slice(&data.to_le_bytes());
    bytes
}

/// Says when a source has been silent for too long, and when it is heard again — once each.
pub struct Watch {
    quiet_for: f64,
    limit: f64,
    told: bool,
}

/// What a [`Watch`] has to say after a stretch of sound.
#[derive(Debug, PartialEq, Eq)]
pub enum Heard {
    Nothing,
    Silent,
    Again,
}

impl Watch {
    pub fn new(limit_seconds: f64) -> Self {
        Self {
            quiet_for: 0.0,
            limit: limit_seconds,
            told: false,
        }
    }

    /// `loudest` is the loudest sample of the last `seconds`.
    pub fn after(&mut self, seconds: f64, loudest: f32) -> Heard {
        if loudest > SILENCE {
            self.quiet_for = 0.0;
            return if std::mem::take(&mut self.told) {
                Heard::Again
            } else {
                Heard::Nothing
            };
        }
        self.quiet_for += seconds;
        if self.quiet_for >= self.limit && !self.told {
            self.told = true;
            return Heard::Silent;
        }
        Heard::Nothing
    }
}

/// Creates the session's folder, named by the local day and hour — `2026-10-13_14-05`; a
/// second session in the same minute takes `-2`.
pub fn new_folder(sessions: &Path, day_and_hour: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(sessions)?;
    for attempt in 1..1000 {
        let name = if attempt == 1 {
            day_and_hour.to_string()
        } else {
            format!("{day_and_hour}-{attempt}")
        };
        let folder = sessions.join(name);
        match fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "too many sessions in the same minute"))
}

/// Writes `session.json`, whole or not at all. `stopped` absent means: still recording, or
/// ended by a crash.
pub fn write_session(folder: &Path, started: &str, stopped: Option<&str>, me: &str, others: &str) -> io::Result<()> {
    let mut session = serde_json::json!({
        "contract": CONTRACT,
        "started": started,
        "sources": { "me": { "device": me }, "others": { "device": others } },
    });
    if let Some(stopped) = stopped {
        session["stopped"] = stopped.into();
    }
    let draft = folder.join("session.json.part");
    fs::write(&draft, serde_json::to_vec_pretty(&session)?)?;
    fs::rename(draft, folder.join("session.json"))
}

/// The node a desktop's default device stands for, as `pw-metadata -n default 0 <key>` prints
/// it — `key` being `default.audio.source` for the microphone, `default.audio.sink` for what
/// plays — or nothing when the output does not say.
pub fn standing_for(metadata: &str, key: &str) -> Option<String> {
    let line = metadata.lines().find(|line| line.contains(&format!("key:'{key}'")))?;
    let value = line.split("value:'").nth(1)?.split("' type:").next()?;
    let value: serde_json::Value = serde_json::from_str(value).ok()?;
    Some(value["name"].as_str()?.to_string())
}
