//! What an engine hears of a session recorded as `contracts/session-audio/v2` describes it:
//! each track brought to one channel, [`RATE`] samples a second, around its rest.
//!
//! Nothing is written: the tracks are the recording, and this is a way of listening to them
//! (ADR-0004). A track is read as it comes — it is never held whole as delivered.

use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::Path,
};

use rubato::{Fft, FixedSync, Indexing, Resampler, WindowFunction, audioadapter_buffers::direct::InterleavedSlice};

use crate::step::RATE;

/// Frames a second no source of a meeting delivers fewer or more of: a header that says
/// otherwise is not believed, and not given the memory it would take.
const RATES: std::ops::RangeInclusive<u32> = 1_000..=1_000_000;
/// Frames the resampler is given at a time.
const BLOCK: usize = 4096;
fn refused<T>(why: String) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, why))
}

/// A track as an engine hears it, in the track's own time. A file that is not a track is
/// said and not converted; its header's lengths are not trusted.
pub fn heard(path: &Path) -> io::Result<Vec<f32>> {
    let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    // Whatever the system says of the file is said with the file's name; what this says of it stands.
    let unread = |error: io::Error| match error.kind() {
        io::ErrorKind::InvalidData => error,
        _ => io::Error::new(io::ErrorKind::InvalidData, format!("{name} cannot be read ({error})")),
    };
    if !fs::metadata(path).map_err(unread)?.is_file() {
        return refused(format!("{name} is not a file"));
    }
    let mut file = BufReader::with_capacity(1 << 16, File::open(path).map_err(unread)?);
    let (rate, channels) = format(&mut file, &name).map_err(unread)?;
    resampled(&mut file, rate, channels).map(|(sound, _)| sound).map_err(unread)
}

/// The whole track as one channel at [`RATE`], around its rest, in the track's own time —
/// and how many frames the track holds.
fn resampled(file: &mut impl Read, rate: u32, channels: usize) -> io::Result<(Vec<f32>, u64)> {
    let failed = |error: &dyn std::fmt::Display| io::Error::other(format!("its sound could not be brought to {RATE} Hz: {error}"));
    // One long block: the longer it is, the closer to half the engine's rate a voice is kept whole.
    let mut resampler = Fft::<f32>::new_custom(rate as usize, RATE, BLOCK, 1, 1, WindowFunction::BlackmanHarris2, FixedSync::Input)
        .map_err(|e| failed(&e))?;
    let delay = resampler.output_delay();
    let mut piece = vec![0.0f32; resampler.input_frames_max()];
    let mut made = vec![0.0f32; resampler.output_frames_max()];
    let (mut flat, mut frames, mut ended) = (Vec::new(), 0u64, false);
    // Where the source rests when the track opens — the middle value of its first block, so
    // that a voice already speaking is not taken for the rest: taken out before anything
    // else, and a track does not open on a step as loud as its rest.
    let mut rest = None;
    let mut frame = vec![0u8; 3 * channels];
    // The track, then silence until what the resampler still holds of it has come out.
    while !ended || flat.len() < delay + (frames as f64 * RATE as f64 / f64::from(rate)).ceil() as usize {
        let wanted = resampler.input_frames_next();
        let mut held = 0;
        while !ended && held < wanted {
            if !read_whole(file, &mut frame)? {
                ended = true;
                break;
            }
            let mixed: f32 = frame
                .as_chunks::<3>()
                .0
                .iter()
                .map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0)
                .sum();
            piece[held] = mixed / channels as f32;
            held += 1;
        }
        let rest = *rest.get_or_insert_with(|| {
            let mut first = piece[..held].to_vec();
            first.sort_by(f32::total_cmp);
            first.get(held / 2).copied().unwrap_or(0.0)
        });
        piece[..held].iter_mut().for_each(|sample| *sample -= rest);
        frames += held as u64;
        let (given, taken) = (piece.len(), made.len());
        let from = InterleavedSlice::new(&piece, 1, given).map_err(|e| failed(&e))?;
        let mut into = InterleavedSlice::new_mut(&mut made, 1, taken).map_err(|e| failed(&e))?;
        let (_, written) = resampler
            .process_into_buffer(&from, &mut into, Some(&Indexing::new().partial_len(held)))
            .map_err(|e| failed(&e))?;
        flat.extend_from_slice(&made[..written]);
    }
    let length = (frames as f64 * RATE as f64 / f64::from(rate)).round() as usize;
    let mut flat: Vec<f32> = flat.into_iter().skip(delay).take(length).collect();
    // Around its rest: a source may rest away from zero, and silence there is silence — and
    // a rest moves, slowly. One pole at 20 Hz, starting from the first sample itself. A rest
    // that steps — a device that wakes — is heard as the click it is.
    let keep = 1.0 - std::f32::consts::TAU * 20.0 / RATE as f32;
    let (mut last_in, mut last_out) = (flat.first().copied().unwrap_or(0.0), 0.0f32);
    for sample in &mut flat {
        let out = *sample - last_in + keep * last_out;
        (last_in, last_out) = (*sample, out);
        *sample = out;
    }
    Ok((flat, frames))
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
        let size = u64::from(u32::from_le_bytes([head[4], head[5], head[6], head[7]]));
        if &head[0..4] == b"data" {
            break;
        }
        // A chunk is padded to an even length. Only the format's is kept; any other is passed over.
        let mut chunk = Vec::new();
        let kept = if &head[0..4] == b"fmt " { size.min(64) } else { 0 };
        let read = file.by_ref().take(kept).read_to_end(&mut chunk)? as u64
            + io::copy(&mut file.by_ref().take(size + size % 2 - kept), &mut io::sink())?;
        if read < size {
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
        Some((true, channels, rate, 24)) if channels > 0 && RATES.contains(&rate) => Ok((rate, channels)),
        Some((pcm, channels, rate, bits)) => refused(format!(
            "{name} is not what a session holds — PCM in 24 bits: it has {channels} channel(s), {rate} Hz, {bits} bits{}",
            if pcm { "" } else { ", and is not PCM" }
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
