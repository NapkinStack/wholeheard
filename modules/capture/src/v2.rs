//! A session's folder as `session-audio/v2` describes it (contracts/session-audio/v2): each
//! source as it was delivered, and the session's clock beside.
//!
//! A [`Track`] writes what a source delivers and changes nothing in it — its rate, its
//! channels, where it rests.

use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
    path::Path,
};

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
    /// The start of a frame the system delivered in two pieces, kept for its end.
    cut: Vec<f32>,
    /// Where each channel rests, to measure sound from there: (last sample, last measure).
    rest: Vec<Option<(f32, f32)>>,
    keep: f32,
    loudest: f32,
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
            cut: Vec::new(),
            rest: vec![None; usize::from(channels)],
            // One pole at 20 Hz: what a source rests at is not sound, a voice is.
            keep: (1.0 - std::f64::consts::TAU * 20.0 / f64::from(rate)).max(0.0) as f32,
            loudest: 0.0,
        })
    }

    /// Appends what the source just delivered, interleaved, as it is; it is on disk when
    /// this returns. A frame delivered in two pieces is written once whole. A sample beyond
    /// full scale is written at full scale, one that is no number as zero.
    pub fn push(&mut self, interleaved: &[f32]) -> io::Result<()> {
        let channels = usize::from(self.channels);
        let mut sound = std::mem::take(&mut self.cut);
        sound.extend_from_slice(interleaved);
        self.cut = sound.split_off(sound.len() - sound.len() % channels);
        let mut bytes = Vec::with_capacity(sound.len() * BYTES);
        for (at, &sample) in sound.iter().enumerate() {
            // What is no number is no sound either: written as zero, and not measured.
            if sample.is_nan() {
                bytes.extend_from_slice(&[0; BYTES]);
                continue;
            }
            self.measure(at % channels, sample.clamp(-1.0, 1.0));
            let value = (sample * FULL_SCALE).round().clamp(-FULL_SCALE, FULL_SCALE - 1.0) as i32;
            bytes.extend_from_slice(&value.to_le_bytes()[..BYTES]);
        }
        // Refused, a delivery leaves nothing: not its frames, not the piece of one it held.
        self.write(&bytes).inspect_err(|_| self.cut.clear())?;
        self.frames += (sound.len() / channels) as u64;
        Ok(())
    }

    /// Writes whole or not at all: a write the system cut leaves the track as it was, so
    /// that what follows a failure is still frames.
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
        }
    }

    /// Writes zeros until the track holds `frames`, for a source that delivered nothing, and
    /// returns the frames written that way: the clock has to say them. Never shortens.
    pub fn silence_to(&mut self, frames: u64) -> io::Result<Option<(u64, u64)>> {
        if frames <= self.frames {
            return Ok(None);
        }
        let from = self.frames;
        // The source stopped: the start of a frame it never ended is not the start of the next.
        self.cut.clear();
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

    /// The loudest sound delivered since the last call, as a share of full scale, measured
    /// from where the source rests.
    pub fn loudest(&mut self) -> f32 {
        std::mem::take(&mut self.loudest)
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
