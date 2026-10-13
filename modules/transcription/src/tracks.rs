//! What an engine hears of a session recorded as `contracts/session-audio/v2` describes it:
//! each track brought to one channel, [`RATE`] samples a second, on the session's clock.
//!
//! Nothing is written: the tracks are the recording, and this is a way of listening to them
//! (ADR-0004). A track is read as it comes — an hour of it is never held as delivered.

use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::Path,
};

use crate::step::RATE;

const TAPS: usize = 97;

fn refused<T>(why: String) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, why))
}

/// The first point `clock.jsonl` holds for a track: a frame, and the instant it was heard,
/// in seconds on the session's clock. A line cut by a crash, or that is no JSON, is passed over.
pub fn first_point(folder: &Path, track: &str) -> Option<(u64, f64)> {
    let clock = fs::read(folder.join("clock.jsonl")).unwrap_or_default();
    String::from_utf8_lossy(&clock)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["track"] == track)
        .find_map(|line| Some((line["sample"].as_u64()?, line["at"].as_f64()?)))
}

/// A track as an engine hears it, placed by a point of its clock: the silence before its
/// first frame comes first — or that much of the track is left out, when it started before
/// the session did. Without a point it starts with the session. A file that is not a track
/// is said and not converted; its header's lengths are not trusted.
pub fn heard(path: &Path, point: Option<(u64, f64)>) -> io::Result<Vec<f32>> {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let file = File::open(path).or_else(|error| refused(format!("{name} cannot be read ({error})")))?;
    let mut file = BufReader::with_capacity(1 << 16, file);
    let (rate, channels) = format(&mut file, &name)?;

    let step = rate as f64 / RATE as f64;
    let taps = low_pass(rate as f64);
    let (mut history, mut cursor) = (vec![0.0f32; TAPS], 0);
    let (mut seen, mut next, mut before) = (0u64, 0.0f64, 0.0f32);
    // Where frame 0 is on the session's clock; an instant no session holds is not believed.
    let starts = point.map_or(0.0, |(frame, at)| at - frame as f64 / f64::from(rate));
    let lead = (starts.clamp(-86_400.0, 86_400.0) * RATE as f64).round() as i64;
    let mut out = vec![0.0f32; lead.max(0) as usize];
    let mut skip = (-lead).max(0) as u64;

    let mut frame = vec![0u8; 3 * channels];
    while read_whole(&mut file, &mut frame)? {
        let mixed = frame
            .as_chunks::<3>()
            .0
            .iter()
            .map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0)
            .sum::<f32>()
            / channels as f32;
        // What would fold back is removed before samples are dropped.
        let filtered = if step > 1.0 {
            history[cursor] = mixed;
            cursor = (cursor + 1) % TAPS;
            let (recent, older) = history.split_at(cursor);
            older.iter().chain(recent).zip(&taps).map(|(sample, tap)| sample * tap).sum()
        } else {
            mixed
        };
        // Output samples fall between two filtered ones: the line between them is read.
        while next <= seen as f64 {
            let along = (next - (seen as f64 - 1.0)) as f32;
            if skip > 0 {
                skip -= 1;
            } else {
                out.push(before + along * (filtered - before));
            }
            next += step;
        }
        before = filtered;
        seen += 1;
    }
    Ok(out)
}

/// Reads up to the audio, and gives the rate and the channels the track's own header says.
fn format(file: &mut impl Read, name: &str) -> io::Result<(u32, usize)> {
    let not_a_track = || refused(format!("{name} is not a track of a session"));
    let mut start = [0u8; 12];
    if !read_whole(file, &mut start)? || &start[0..4] != b"RIFF" || &start[8..12] != b"WAVE" {
        return not_a_track();
    }
    let mut said = None;
    loop {
        let mut head = [0u8; 8];
        if !read_whole(file, &mut head)? {
            return not_a_track();
        }
        let size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if &head[0..4] == b"data" {
            break;
        }
        // A chunk is padded to an even length; none before the audio is long.
        let mut chunk = vec![0u8; (size + size % 2).min(1 << 16)];
        if size > chunk.len() || !read_whole(file, &mut chunk)? {
            return not_a_track();
        }
        if &head[0..4] == b"fmt " && size >= 16 {
            let field = |at: usize, len: usize| chunk[at..at + len].iter().rev().fold(0u32, |n, &b| n << 8 | u32::from(b));
            // PCM, said plainly or — as 24 bits ask — in the extensible form.
            let pcm = field(0, 2) == 1 || (field(0, 2) == 0xFFFE && size >= 26 && field(24, 2) == 1);
            said = Some((pcm, field(2, 2) as usize, field(4, 4), field(14, 2)));
        }
    }
    match said {
        Some((true, channels, rate, 24)) if channels > 0 && rate > 0 => Ok((rate, channels)),
        Some((_, channels, rate, bits)) => refused(format!(
            "{name} is not what a session holds — PCM in 24 bits: it has {channels} channel(s), {rate} Hz, {bits} bits"
        )),
        None => not_a_track(),
    }
}

/// Fills `buffer`, or says the file ended before it could: a frame cut by a crash is not sound.
fn read_whole(file: &mut impl Read, buffer: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read(&mut buffer[filled..]) {
            Ok(0) => return Ok(false),
            Ok(more) => filled += more,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

/// A windowed-sinc low-pass just under half of [`RATE`].
fn low_pass(rate: f64) -> Vec<f32> {
    let cutoff = 0.45 * RATE as f64 / rate;
    let middle = (TAPS - 1) as f64 / 2.0;
    let mut taps: Vec<f64> = (0..TAPS)
        .map(|n| {
            let x = n as f64 - middle;
            let sinc = if x == 0.0 {
                2.0 * cutoff
            } else {
                (std::f64::consts::TAU * cutoff * x).sin() / (std::f64::consts::PI * x)
            };
            sinc * (0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / (TAPS - 1) as f64).cos())
        })
        .collect();
    let sum: f64 = taps.iter().sum();
    taps.iter_mut().for_each(|tap| *tap /= sum);
    taps.into_iter().map(|tap| tap as f32).collect()
}
