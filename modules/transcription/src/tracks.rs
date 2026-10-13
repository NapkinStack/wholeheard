//! What an engine hears of a session recorded as `contracts/session-audio/v2` describes it:
//! each track brought to one channel, [`RATE`] samples a second, and placed on the session's
//! clock by every point that clock holds — nothing else done to what a source delivered.
//!
//! Nothing is written: the tracks are the recording, and this is a way of listening to them
//! (ADR-0004). Where a source rests is left where it is: whether a span is silence is a
//! measure the step makes, not a correction made here. Where a source delivered nothing —
//! zeros the clock says are the capture's, an instant the clock leaves empty — what is heard
//! goes in a line from where the source left to where it came back: a fall to zero and back
//! is a step the source never made. A track is read as it comes — it is never held whole as
//! delivered.

use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::Path,
};

use rubato::{Fft, FixedSync, Resampler, WindowFunction, audioadapter_buffers::direct::InterleavedSlice};

use crate::step::RATE;

/// Frames a second no source of a meeting delivers fewer or more of: a header that says
/// otherwise is not believed, and not given the memory it would take.
const RATES: std::ops::RangeInclusive<u32> = 1_000..=1_000_000;
/// Frames the resampler is given at a time.
const BLOCK: usize = 4096;
/// A frame heard this far, in seconds, from where the points before it put it starts again
/// from its own point: the capture's own threshold for saying a source stopped a moment.
const OFF: f64 = 0.005;
/// No session is this long, in seconds: an instant beyond it is not one a clock holds.
const LONGEST: f64 = 43_200.0;

fn refused<T>(why: String) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, why))
}

/// What `clock.jsonl` says of one track.
#[derive(Debug, Default, PartialEq)]
pub struct Clock {
    /// A frame, and the instant it was heard, in seconds on the session's clock — in the
    /// order of the frames.
    pub points: Vec<(u64, f64)>,
    /// Frames the source did not deliver, from and up to: zeros of the capture's.
    pub silences: Vec<(u64, u64)>,
}

/// The lines `clock.jsonl` holds for a track. A line cut by a crash, that is no JSON, or
/// that holds no instant of a session, is passed over.
pub fn clock(folder: &Path, track: &str) -> Clock {
    let lines = fs::read(folder.join("clock.jsonl")).unwrap_or_default();
    let mut clock = Clock::default();
    for line in String::from_utf8_lossy(&lines).trim_start_matches('\u{feff}').lines() {
        let line = serde_json::from_str::<serde_json::Value>(line).unwrap_or_default();
        let (frame, at, silence) = (line["sample"].as_u64(), line["at"].as_f64(), &line["silence"]);
        match (frame, at, silence[0].as_u64(), silence[1].as_u64()) {
            _ if line["track"] != track => {}
            (Some(frame), Some(at), ..) if at.abs() <= LONGEST => clock.points.push((frame, at)),
            (.., Some(from), Some(to)) if from < to => clock.silences.push((from, to)),
            _ => {}
        }
    }
    clock.points.sort_by_key(|(frame, _)| *frame);
    clock.points.dedup_by_key(|(frame, _)| *frame);
    clock.silences.sort_unstable();
    clock
}

/// A track as an engine hears it, placed by its clock: the sound after a source stopped a
/// moment where it was heard, what was heard before the session started left out. Without a
/// point it starts with the session. A file that is not a track is said and not converted;
/// its header's lengths are not trusted.
pub fn heard(path: &Path, clock: &Clock) -> io::Result<Vec<f32>> {
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
    // A clock made elsewhere than by `clock` is read in the order of its frames all the same.
    let (mut points, mut silences) = (clock.points.clone(), clock.silences.clone());
    points.sort_by_key(|(frame, _)| *frame);
    points.dedup_by_key(|(frame, _)| *frame);
    silences.sort_unstable();
    let frame = vec![0u8; 3 * channels];
    let mut track = Frames {
        file: &mut file,
        frame,
        silences: &silences,
        read: 0,
        last: None,
        line: (0, 0.0, 0.0),
        after: None,
    };

    // Each run is brought to the engine's rate by itself: the frames either side of a stop
    // were not heard one after the other, and are not taken for neighbours.
    let starts = runs(&points, rate, u64::MAX);
    let (mut sounds, mut frames) = (Vec::new(), 0u64);
    for (index, (from, _)) in starts.iter().enumerate() {
        let until = starts.get(index + 1).map_or(u64::MAX, |(next, _)| *next);
        let (sound, held, ended) = resampled(&mut track, rate, until - from).map_err(unread)?;
        sounds.push(sound);
        frames += held;
        if ended {
            break;
        }
    }
    let mut out: Vec<f32> = Vec::new();
    // A run too short to give the engine a sample places nothing, and leaves no instant empty.
    let placed = runs(&points, rate, frames.max(1));
    for (&(_, at), sound) in placed.iter().zip(&sounds).filter(|(_, sound)| !sound.is_empty()) {
        // Where this run starts in what the engine hears; what was heard before the session
        // started is left out, and a run the clock puts before the end of the one before
        // takes its place from there.
        let start = (at * RATE as f64).round() as i64;
        let sound = &sound[(-start).clamp(0, sound.len() as i64) as usize..];
        let start = start.max(0) as usize;
        // An instant the clock leaves empty: a line from the sound before it to the sound after.
        let to = sound.first().or(out.last()).copied().unwrap_or(0.0);
        let (from, gap) = (out.last().copied().unwrap_or(to), start.saturating_sub(out.len()));
        out.truncate(start);
        out.extend((1..=gap).map(|step| from + (to - from) * step as f32 / (gap + 1) as f32));
        out.extend_from_slice(sound);
    }
    Ok(out)
}

/// Each run of frames the clock places by itself — where it starts in the track, and when —
/// from the points for frames before `held`: one for a frame the track does not hold is not used.
fn runs(points: &[(u64, f64)], rate: u32, held: u64) -> Vec<(u64, f64)> {
    let mut runs: Vec<(u64, f64)> = Vec::new();
    for &(frame, at) in points.iter().filter(|(frame, _)| *frame < held) {
        match runs.last() {
            // The first point places the track's first frame too, at the track's own rate.
            None => runs.push((0, at - frame as f64 / f64::from(rate))),
            Some(&(from, then)) if (at - then - (frame - from) as f64 / f64::from(rate)).abs() > OFF => runs.push((frame, at)),
            Some(_) => {}
        }
    }
    if runs.is_empty() {
        runs.push((0, 0.0));
    }
    runs
}

/// A track's frames as one channel, in their order — the zeros the capture wrote where its
/// source delivered nothing given as a line from where the source left to where it came back.
struct Frames<'a, R> {
    file: &'a mut R,
    frame: Vec<u8>,
    silences: &'a [(u64, u64)],
    read: u64,
    /// The last value given, the zeros still to give as a line — how many, towards what, by
    /// what step — and the frame read past them.
    last: Option<f32>,
    line: (u64, f32, f32),
    after: Option<f32>,
}

impl<R: Read> Frames<'_, R> {
    fn next(&mut self) -> io::Result<Option<f32>> {
        let (mut zeros, mut to) = (0u64, None);
        if self.line.0 > 0 {
            self.line.0 -= 1;
            self.last = Some(self.line.1 - self.line.2 * (self.line.0 + 1) as f32);
            return Ok(self.last);
        }
        if self.after.is_some() {
            self.last = self.after.take();
            return Ok(self.last);
        }
        while read_whole(self.file, &mut self.frame)? {
            let at = self.read;
            self.read += 1;
            while self.silences.first().is_some_and(|(_, to)| *to <= at) {
                self.silences = &self.silences[1..];
            }
            // The capture's zeros are zeros: a frame the clock names that holds a sound is a sound.
            if self.silences.first().is_some_and(|(from, _)| *from <= at) && self.frame.iter().all(|byte| *byte == 0) {
                zeros += 1;
                continue;
            }
            let channels = self.frame.as_chunks::<3>().0;
            let mixed: f32 = channels
                .iter()
                .map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0)
                .sum();
            to = Some(mixed / channels.len() as f32);
            break;
        }
        if zeros == 0 {
            self.last = to.or(self.last);
            return Ok(to);
        }
        // A track that opens on the capture's zeros stays where its source then starts; one
        // that ends on them, where its source left.
        let towards = to.or(self.last).unwrap_or(0.0);
        self.line = (zeros, towards, (towards - self.last.unwrap_or(towards)) / (zeros + 1) as f32);
        self.after = to;
        self.next()
    }
}

/// The next frames of a track, `most` of them at most, as one channel at [`RATE`], as they
/// were delivered, in the track's own time — how many frames that was, and whether the track ended.
fn resampled<R: Read>(track: &mut Frames<R>, rate: u32, most: u64) -> io::Result<(Vec<f32>, u64, bool)> {
    let failed = |error: &dyn std::fmt::Display| io::Error::other(format!("its sound could not be brought to {RATE} Hz: {error}"));
    // One long block: the longer it is, the closer to half the engine's rate a voice is kept whole.
    let mut resampler = Fft::<f32>::new_custom(rate as usize, RATE, BLOCK, 1, 1, WindowFunction::BlackmanHarris2, FixedSync::Input)
        .map_err(|e| failed(&e))?;
    let delay = resampler.output_delay();
    let mut piece = vec![0.0f32; resampler.input_frames_max()];
    let mut made = vec![0.0f32; resampler.output_frames_max()];
    let (mut flat, mut frames, mut ended) = (Vec::new(), 0u64, false);
    // Before their first frame and after their last, these frames are taken to stay where
    // they are — not to fall to zero: a source that rests away from zero never made that
    // step, and it is not heard. Nothing of the track itself is moved: what is taken out is put back.
    let (mut first, mut last) = (None, 0.0f32);
    while !(ended || frames == most) || flat.len() < delay + (frames as f64 * RATE as f64 / f64::from(rate)).ceil() as usize {
        let mut held = 0;
        while !ended && held < piece.len() && frames + (held as u64) < most {
            match track.next()? {
                Some(sample) => (piece[held], held) = (sample, held + 1),
                None => ended = true,
            }
        }
        let first = *first.get_or_insert(if held > 0 { piece[0] } else { 0.0 });
        last = if held > 0 { piece[held - 1] } else { last };
        piece[..held].iter_mut().for_each(|sample| *sample -= first);
        piece[held..].fill(last - first);
        frames += held as u64;
        let (given, taken) = (piece.len(), made.len());
        let from = InterleavedSlice::new(&piece, 1, given).map_err(|e| failed(&e))?;
        let mut into = InterleavedSlice::new_mut(&mut made, 1, taken).map_err(|e| failed(&e))?;
        let (_, written) = resampler.process_into_buffer(&from, &mut into, None).map_err(|e| failed(&e))?;
        flat.extend_from_slice(&made[..written]);
    }
    let length = (frames as f64 * RATE as f64 / f64::from(rate)).round() as usize;
    let first = first.unwrap_or(0.0);
    Ok((
        flat.into_iter().skip(delay).take(length).map(|sample| sample + first).collect(),
        frames,
        ended,
    ))
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
