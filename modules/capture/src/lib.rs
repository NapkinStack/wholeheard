//! What `capture` promises, with no audio device in sight: a session's folder as the
//! `session-audio/v1` contract describes it (contracts/session-audio/v1).
//!
//! A [`Track`] takes sound as a source delivers it — any rate, any number of channels, resting
//! wherever the hardware rests — and appends it to a file that is one channel, 16-bit,
//! 16,000 samples a second, centred, and on disk as soon as it is heard.

use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    path::Path,
};

/// Samples per second of every audio file of a session.
pub const RATE: u32 = 16_000;
/// Below this share of full scale, a source is silent.
pub const SILENCE: f32 = 0.001;

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
