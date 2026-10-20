//! A meeting as the window plays it (ADR-0009): one sound, each track placed where the
//! session's clock puts it, its channels averaged, the tracks added — a WAV of one channel of
//! 16 bits at the highest of the tracks' rates, made as it is asked for, by any range, and kept
//! nowhere. The tracks are only read (`session-audio`: found by their chunks, their lengths
//! ignored, read to the end); one of a lower rate is brought up with rubato, as the transcription
//! brings its tracks down.

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use rubato::{Fft, FixedSync, Resampler, WindowFunction, audioadapter_buffers::direct::InterleavedSlice};

/// The mix's header: RIFF, `fmt `, `data`.
const HEADER: u64 = 44;

/// A track as its own header says it is.
struct Track {
    path: PathBuf,
    /// Where its audio starts in the file, and how many whole frames follow, to the file's end.
    data: u64,
    frames: u64,
    rate: u32,
    channels: usize,
    /// Bytes a sample: 2, 3 or 4 for PCM; 4 for float.
    width: usize,
    float: bool,
    /// The mix's frame its frame 0 falls on: before the session's start, below zero.
    at: i64,
}

pub struct Mix {
    rate: u32,
    frames: u64,
    tracks: Vec<Track>,
}

impl Mix {
    /// The mix of these tracks, each with where its first frame is on the session's clock, in
    /// seconds; none when not one of them reads as a track.
    pub fn of(tracks: &[(PathBuf, f64)]) -> Option<Mix> {
        let read: Vec<(Track, f64)> = tracks.iter().filter_map(|(path, starts)| Some((track(path)?, *starts))).collect();
        let rate = read.iter().map(|(track, _)| track.rate).max()?;
        let tracks: Vec<Track> = read
            .into_iter()
            .map(|(track, starts)| Track {
                at: (starts * f64::from(rate)).round() as i64,
                ..track
            })
            .collect();
        let end = tracks
            .iter()
            .map(|track| track.at + (track.frames as f64 * f64::from(rate) / f64::from(track.rate)).round() as i64)
            .max()?;
        Some(Mix {
            rate,
            frames: end.max(0) as u64,
            tracks,
        })
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Its length in bytes, header included.
    pub fn size(&self) -> u64 {
        HEADER + 2 * self.frames
    }

    /// `length` bytes of the mix from byte `from`, as a file of it would hold them.
    pub fn read(&self, from: u64, length: u64) -> io::Result<Vec<u8>> {
        let to = (from + length).min(self.size());
        let mut bytes = Vec::with_capacity(to.saturating_sub(from) as usize);
        if from < HEADER {
            bytes.extend_from_slice(&self.header()[from as usize..to.min(HEADER) as usize]);
        }
        if to > HEADER {
            let start = from.max(HEADER) - HEADER;
            let (first, last) = (start / 2, (to - HEADER).div_ceil(2));
            let mut samples = Vec::with_capacity(2 * (last - first) as usize);
            for sample in self.render(first, (last - first) as usize)? {
                samples.extend_from_slice(&((sample.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
            }
            let skip = (start - 2 * first) as usize;
            bytes.extend_from_slice(&samples[skip..skip + (to - HEADER - start) as usize]);
        }
        Ok(bytes)
    }

    fn header(&self) -> [u8; HEADER as usize] {
        let data = u32::try_from(2 * self.frames).unwrap_or(u32::MAX);
        let mut header = [0u8; HEADER as usize];
        header[0..4].copy_from_slice(b"RIFF");
        header[4..8].copy_from_slice(&data.saturating_add(36).to_le_bytes());
        header[8..16].copy_from_slice(b"WAVEfmt ");
        header[16..20].copy_from_slice(&16u32.to_le_bytes());
        header[20..22].copy_from_slice(&1u16.to_le_bytes());
        header[22..24].copy_from_slice(&1u16.to_le_bytes());
        header[24..28].copy_from_slice(&self.rate.to_le_bytes());
        header[28..32].copy_from_slice(&(2 * self.rate).to_le_bytes());
        header[32..34].copy_from_slice(&2u16.to_le_bytes());
        header[34..36].copy_from_slice(&16u16.to_le_bytes());
        header[36..40].copy_from_slice(b"data");
        header[40..44].copy_from_slice(&data.to_le_bytes());
        header
    }

    /// `count` frames of the mix from `first`, the tracks added.
    fn render(&self, first: u64, count: usize) -> io::Result<Vec<f32>> {
        let mut sum = vec![0.0f32; count];
        for track in &self.tracks {
            if track.rate == self.rate {
                for (into, sample) in sum.iter_mut().zip(track.mono(first as i64 - track.at, count)?) {
                    *into += sample;
                }
            } else {
                track.raised(self.rate, first as i64, &mut sum)?;
            }
        }
        Ok(sum)
    }
}

impl Track {
    /// `count` frames from frame `from`, each one channel: the average of its channels; zero
    /// where the track has none.
    fn mono(&self, from: i64, count: usize) -> io::Result<Vec<f32>> {
        let mut mono = vec![0.0f32; count];
        let (start, end) = (from.max(0), (from + count as i64).min(self.frames as i64));
        if start >= end {
            return Ok(mono);
        }
        let block = self.channels * self.width;
        let mut bytes = vec![0u8; (end - start) as usize * block];
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.data + start as u64 * block as u64))?;
        file.read_exact(&mut bytes)?;
        for (frame, into) in bytes.chunks_exact(block).zip(&mut mono[(start - from) as usize..]) {
            let total: f32 = frame.chunks_exact(self.width).map(|sample| self.sample(sample)).sum();
            *into = total / self.channels as f32;
        }
        Ok(mono)
    }

    fn sample(&self, bytes: &[u8]) -> f32 {
        match (self.float, self.width) {
            (true, _) => f32::from_le_bytes(bytes.try_into().unwrap_or_default()),
            (false, 2) => f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32_768.0,
            (false, 3) => (i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) >> 8) as f32 / 8_388_608.0,
            _ => i32::from_le_bytes(bytes.try_into().unwrap_or_default()) as f32 / 2_147_483_648.0,
        }
    }

    /// Adds this track, of a lower rate, brought up to `rate`, into `sum`: the mix's frames from
    /// `first`. The resampler always starts on a frame of the track that falls on a frame of the
    /// mix, so a range of the mix is made of the same frames, whoever asks for it and from where.
    fn raised(&self, rate: u32, first: i64, sum: &mut [f32]) -> io::Result<()> {
        let failed = |error: &dyn std::fmt::Display| io::Error::other(format!("a track could not be brought to {rate} Hz: {error}"));
        let shared = gcd(self.rate, rate);
        let (taken, given) = (i64::from(self.rate / shared), i64::from(rate / shared));
        let chunk = 1024;
        let mut resampler = Fft::<f32>::new_custom(
            self.rate as usize,
            rate as usize,
            chunk as usize,
            1,
            1,
            WindowFunction::BlackmanHarris2,
            FixedSync::Input,
        )
        .map_err(|e| failed(&e))?;
        let delay = resampler.output_delay() as i64;
        // Two blocks before the first frame asked, for the resampler to settle.
        let wanted = ((first - self.at) as f64 * taken as f64 / given as f64).floor() as i64;
        let start = (wanted - 2 * chunk).div_euclid(taken) * taken;
        let from = self.at + start / taken * given;
        let (mut made, mut read) = (Vec::new(), start);
        let mut out = vec![0.0f32; resampler.output_frames_max()];
        while (made.len() as i64) < delay + first + sum.len() as i64 - from {
            let piece = self.mono(read, chunk as usize)?;
            read += chunk;
            let source = InterleavedSlice::new(&piece, 1, piece.len()).map_err(|e| failed(&e))?;
            let room = out.len();
            let mut into = InterleavedSlice::new_mut(&mut out, 1, room).map_err(|e| failed(&e))?;
            let (_, written) = resampler.process_into_buffer(&source, &mut into, None).map_err(|e| failed(&e))?;
            made.extend_from_slice(&out[..written]);
        }
        let skip = (delay + first - from) as usize;
        for (into, sample) in sum.iter_mut().zip(&made[skip..]) {
            *into += sample;
        }
        Ok(())
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A track as its header says it is, or nothing when it does not read as one.
fn track(path: &Path) -> Option<Track> {
    let mut file = File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut head = vec![0u8; 64 * 1024];
    let read = file.read(&mut head).ok()?;
    let head = &head[..read];
    if head.get(0..4)? != b"RIFF" || head.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut at, mut format) = (12usize, None);
    while at + 8 <= head.len() {
        let length = u32::from_le_bytes(head[at + 4..at + 8].try_into().ok()?) as usize;
        match &head[at..at + 4] {
            b"fmt " => {
                let fmt = head.get(at + 8..at + 24)?;
                // WAVE_FORMAT_EXTENSIBLE says its own in the first two bytes of its sub-format.
                let tag = match u16::from_le_bytes([fmt[0], fmt[1]]) {
                    0xFFFE => u16::from_le_bytes(head.get(at + 32..at + 34)?.try_into().ok()?),
                    tag => tag,
                };
                let channels = usize::from(u16::from_le_bytes([fmt[2], fmt[3]]));
                let rate = u32::from_le_bytes(fmt[4..8].try_into().ok()?);
                let width = usize::from(u16::from_le_bytes([fmt[14], fmt[15]])) / 8;
                let float = match (tag, width) {
                    (1, 2..=4) => false,
                    (3, 4) => true,
                    _ => return None,
                };
                format = Some((channels, rate, width, float));
            }
            b"data" => {
                let (channels, rate, width, float) = format?;
                if channels == 0 || rate == 0 {
                    return None;
                }
                let data = (at + 8) as u64;
                return Some(Track {
                    path: path.to_path_buf(),
                    data,
                    frames: size.saturating_sub(data) / (channels * width) as u64,
                    rate,
                    channels,
                    width,
                    float,
                    at: 0,
                });
            }
            _ => {}
        }
        at = at.checked_add(8 + length + length % 2)?;
    }
    None
}
