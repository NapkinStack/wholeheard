//! A session's folder as `session-audio/v2` describes it (contracts/session-audio/v2): each
//! source as it was delivered, and the session's clock beside.
//!
//! A [`Track`] writes what a source delivers and changes nothing in it — its rate, its
//! channels, where it rests. A [`Clock`] says where the frames of each track are in time.

use std::{
    fs::{self, File},
    io::{self, Seek, SeekFrom, Write},
    path::Path,
};

/// The contract's name, as `session.json` carries it.
pub const CONTRACT: &str = "session-audio/v2";

/// A header that says 24 bits needs the extensible form: 68 bytes before the first sample.
const HEADER: usize = 68;
/// A length that says "not known yet": players then read to the end of the file.
const UNKNOWN: u32 = u32::MAX;
const BYTES: usize = 3;
/// Full scale as every reader of 24 bits counts it.
const FULL_SCALE: f32 = 8_388_608.0;

/// One source of a session, growing.
pub struct Track {
    file: File,
    frames: u64,
    rate: u32,
    channels: u16,
    /// Where each channel rests, to measure sound from there: (last sample, last measure).
    rest: Vec<Option<(f32, f32)>>,
    keep: f32,
    loudest: f32,
    /// A sample other than an exact zero came since the last call (#361).
    stirred: bool,
    /// The same, for the meter: read ten times a second, apart from the silence's watch.
    meter: f32,
}

impl Track {
    /// Creates the file with a header a reader can already use.
    pub fn create(path: &Path, rate: u32, channels: u16) -> io::Result<Self> {
        let (rate, channels) = (rate.max(1), channels.max(1));
        if u16::try_from(usize::from(channels) * BYTES).is_err() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "more channels than a track can hold"));
        }
        let mut file = File::create(path)?;
        file.write_all(&header(rate, channels, UNKNOWN, UNKNOWN))?;
        Ok(Self {
            file,
            frames: 0,
            rate,
            channels,
            rest: vec![None; usize::from(channels)],
            // One pole at 20 Hz: what a source rests at is not sound, a voice is.
            keep: (1.0 - std::f64::consts::TAU * 20.0 / f64::from(rate)).max(0.0) as f32,
            loudest: 0.0,
            stirred: false,
            meter: 0.0,
        })
    }

    /// Appends what the source just delivered — whole frames, interleaved — as it is; it is
    /// on disk when this returns. A delivery that is not whole frames is refused and nothing
    /// of it is written: which channel a sample belongs to is never guessed. A sample beyond
    /// full scale is written at full scale, one that is no number as zero.
    pub fn push(&mut self, interleaved: &[f32]) -> io::Result<()> {
        let channels = usize::from(self.channels);
        if !interleaved.len().is_multiple_of(channels) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "a delivery that is not whole frames"));
        }
        let mut bytes = Vec::with_capacity(interleaved.len() * BYTES);
        for (at, &sample) in interleaved.iter().enumerate() {
            // What is no number is no sound either: written as zero, and not measured.
            if sample.is_nan() {
                bytes.extend_from_slice(&[0; BYTES]);
                continue;
            }
            self.stirred |= sample != 0.0;
            self.measure(at % channels, sample.clamp(-1.0, 1.0));
            let value = (sample * FULL_SCALE).round().clamp(-FULL_SCALE, FULL_SCALE - 1.0) as i32;
            bytes.extend_from_slice(&value.to_le_bytes()[..BYTES]);
        }
        self.write(&bytes)?;
        self.frames += (interleaved.len() / channels) as u64;
        Ok(())
    }

    /// Writes whole or not at all: what the system refused part-way is taken back, so that
    /// the track holds only what a delivery that succeeded put there.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes).inspect_err(|_| drop(self.rewind()))
    }

    /// Brings the file back to the frames the track counts.
    fn rewind(&mut self) -> io::Result<()> {
        let whole = HEADER as u64 + self.frames * u64::from(self.channels) * BYTES as u64;
        self.file.set_len(whole)?;
        self.file.seek(SeekFrom::Start(whole)).map(drop)
    }

    /// The track holds the sound as delivered; how loud it is, is measured from where the
    /// source rests — a microphone resting at a third of full scale is not a sound.
    fn measure(&mut self, channel: usize, sample: f32) {
        let (last_in, last_out) = self.rest[channel].unwrap_or((sample, 0.0));
        let out = sample - last_in + self.keep * last_out;
        if out.is_finite() {
            self.rest[channel] = Some((sample, out));
            self.loudest = self.loudest.max(out.abs());
            self.meter = self.meter.max(out.abs());
        }
    }

    /// Writes zeros until the track holds `frames`, for a source that delivered nothing, and
    /// returns the frames written that way: the clock has to say them. Never shortens.
    pub fn silence_to(&mut self, frames: u64) -> io::Result<Option<(u64, u64)>> {
        if frames <= self.frames {
            return Ok(None);
        }
        let from = self.frames;
        // A second at a time: a long gap is not held in memory. Refused part-way, none of it
        // stays: zeros of ours that no one was told of would pass for the room's.
        while self.frames < frames {
            let more = (frames - self.frames).min(u64::from(self.rate));
            if let Err(error) = self.write(&vec![0u8; more as usize * usize::from(self.channels) * BYTES]) {
                self.frames = from;
                let _ = self.write(&[]).and_then(|()| self.rewind());
                return Err(error);
            }
            self.frames += more;
        }
        Ok(Some((from, frames)))
    }

    /// Frames in the file: one sample per channel each.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Frames a second, as the source said them.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Channels of a frame, as the source said them.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// The loudest sound delivered since the last call, as a share of full scale, measured
    /// from where the source rests.
    pub fn loudest(&mut self) -> f32 {
        std::mem::take(&mut self.loudest)
    }

    /// Whether anything but an exact zero came since the last call: no microphone at rest gives
    /// exact zeros, a device that delivers nothing does (#361).
    pub fn stirred(&mut self) -> bool {
        std::mem::take(&mut self.stirred)
    }

    /// How loud it was since the last call, for a meter: the loudest sound on a scale of
    /// decibels, from 0 at -60 dB of full scale or below to 1 at full scale.
    pub fn level(&mut self) -> f32 {
        crate::level(std::mem::take(&mut self.meter))
    }

    /// Writes the true lengths in the header — the only bytes of a track ever written twice.
    /// A track never finished, or longer than a header can say, still reads: see `UNKNOWN`.
    pub fn finish(mut self) -> io::Result<u64> {
        let data = u32::try_from(self.frames * u64::from(self.channels) * BYTES as u64)
            .ok()
            .filter(|data| *data <= UNKNOWN - HEADER as u32);
        if let Some(data) = data {
            self.file.seek(SeekFrom::Start(0))?;
            self.file
                .write_all(&header(self.rate, self.channels, data + HEADER as u32 - 8, data))?;
        }
        self.file.sync_all()?;
        Ok(self.frames)
    }
}

fn header(rate: u32, channels: u16, riff: u32, data: u32) -> [u8; HEADER] {
    let block = channels * BYTES as u16;
    // Which loudspeaker each channel is: said for one and for two, left open beyond.
    let places: u32 = match channels {
        1 => 0x4,
        2 => 0x3,
        _ => 0,
    };
    let mut bytes = [0u8; HEADER];
    bytes[0..4].copy_from_slice(b"RIFF");
    bytes[4..8].copy_from_slice(&riff.to_le_bytes());
    bytes[8..16].copy_from_slice(b"WAVEfmt ");
    bytes[16..20].copy_from_slice(&40u32.to_le_bytes());
    bytes[20..22].copy_from_slice(&0xFFFEu16.to_le_bytes()); // extensible
    bytes[22..24].copy_from_slice(&channels.to_le_bytes());
    bytes[24..28].copy_from_slice(&rate.to_le_bytes());
    bytes[28..32].copy_from_slice(&(rate.saturating_mul(u32::from(block))).to_le_bytes());
    bytes[32..34].copy_from_slice(&block.to_le_bytes());
    bytes[34..36].copy_from_slice(&24u16.to_le_bytes());
    bytes[36..38].copy_from_slice(&22u16.to_le_bytes());
    bytes[38..40].copy_from_slice(&24u16.to_le_bytes()); // bits that hold sound
    bytes[40..44].copy_from_slice(&places.to_le_bytes());
    // PCM, as the extensible form names it.
    bytes[44..60].copy_from_slice(&[1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
    bytes[60..64].copy_from_slice(b"data");
    bytes[64..68].copy_from_slice(&data.to_le_bytes());
    bytes
}

/// The session's clock, growing: one line per thing known, on disk when it is said.
pub struct Clock {
    file: File,
    written: u64,
}

impl Clock {
    pub fn create(path: &Path) -> io::Result<Self> {
        File::create(path).map(|file| Self { file, written: 0 })
    }

    /// The frame `sample` of `track` was heard `at` seconds after the session started.
    pub fn point(&mut self, track: &str, sample: u64, at: f64) -> io::Result<()> {
        // A ten-thousandth of a second: finer than a sample at 8 kHz, and no false digits.
        let at = (at * 10_000.0).round() / 10_000.0;
        if !at.is_finite() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "an instant that is no number"));
        }
        self.say(serde_json::json!({ "track": track, "sample": sample, "at": at }))
    }

    /// The frames `from` up to, not including, `to` are zeros the capture wrote.
    pub fn silence(&mut self, track: &str, (from, to): (u64, u64)) -> io::Result<()> {
        self.say(serde_json::json!({ "track": track, "silence": [from, to] }))
    }

    /// Whole or not at all: the start of a line the system refused is taken back, so that
    /// the next line is not glued to it and lost with it.
    fn say(&mut self, line: serde_json::Value) -> io::Result<()> {
        let line = format!("{line}\n");
        if let Err(error) = self.file.write_all(line.as_bytes()) {
            let _ = self
                .file
                .set_len(self.written)
                .and_then(|()| self.file.seek(SeekFrom::Start(self.written)));
            return Err(error);
        }
        self.written += line.len() as u64;
        Ok(())
    }
}

/// Writes `session.json`, whole or not at all: each track the session holds, and the name
/// the system gave its source. `stopped` absent means: still recording, or ended by a crash.
/// A track the session holds: the device it opened on, and each it was recorded from after,
/// with when on the session's clock.
pub struct Held {
    pub track: String,
    pub device: String,
    /// No name: the system named no device for it from then on.
    pub changed: Vec<(f64, Option<String>)>,
}

impl Held {
    pub fn new(track: &str, device: &str) -> Self {
        Held {
            track: track.into(),
            device: device.into(),
            changed: Vec::new(),
        }
    }

    /// The track is recorded from `name` from `at` on. The device it opened on, said as it opens
    /// — a microphone the track waited for — is no change.
    pub fn moved(&mut self, at: f64, name: Option<&str>) {
        if !(self.changed.is_empty() && name == Some(self.device.as_str())) {
            self.changed.push((at, name.map(str::to_string)));
        }
    }
}

pub fn write_session(folder: &Path, started: &str, stopped: Option<&str>, tracks: &[Held]) -> io::Result<()> {
    let tracks: serde_json::Map<String, serde_json::Value> = tracks
        .iter()
        .map(|held| {
            let mut track = serde_json::json!({ "device": held.device });
            if !held.changed.is_empty() {
                let changed = held
                    .changed
                    .iter()
                    .map(|(at, device)| serde_json::json!({ "at": at, "device": device }));
                track["changed"] = changed.collect();
            }
            (held.track.clone(), track)
        })
        .collect();
    let mut session = serde_json::json!({ "contract": CONTRACT, "started": started, "tracks": tracks });
    if let Some(stopped) = stopped {
        session["stopped"] = stopped.into();
    }
    let draft = folder.join("session.json.part");
    fs::write(&draft, serde_json::to_vec_pretty(&session)?)?;
    fs::rename(draft, folder.join("session.json"))
}
