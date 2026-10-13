//! The step, written once for every engine (ADR-0003): it reads a session's sound as
//! `contracts/session-audio/v1` describes it, hands an engine one span at a time, and writes
//! what was heard. Stopped anywhere, it is run again and goes on from where it was.

use std::{fs, io, path::Path};

use crate::{Making, Segment, Speaker, Transcript};

/// Samples per second of a session's audio files: the contract's, and nothing else is read.
pub const RATE: usize = 16_000;
/// Below this share of full scale a span is silence, as the audio contract writes it: an engine
/// asked to hear silence finds words in it.
const SILENCE: f32 = 0.001;
/// A segment that ends this close to the end of a span may be a sentence cut in two.
const EDGE: f64 = 1.0;

/// What hears: one question, whatever answers it. Times are seconds from the span's start.
pub trait Engine {
    /// Its name and its model, as the transcript's header says them.
    fn name(&self) -> (String, String);
    /// A span of one speaker's sound — one channel, [`RATE`] samples a second — heard in a
    /// language, or the engine's guess, with the context the user attached.
    fn hear(&mut self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> io::Result<Vec<Segment>>;
}

fn refused<T>(why: String) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, why))
}

/// The samples of one audio file of a session. A file that is not what the contract
/// describes is said and not converted; its header's lengths are not trusted — a recording
/// in progress, or ended by a crash, reads to its end.
pub fn sound(path: &Path) -> io::Result<Vec<f32>> {
    let bytes = fs::read(path)?;
    let field = |at: usize, size: usize| {
        bytes
            .get(at..at + size)
            .map(|b| b.iter().rev().fold(0u32, |n, &byte| n << 8 | u32::from(byte)))
    };
    let is = |at: usize, text: &[u8]| bytes.get(at..at + text.len()) == Some(text);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    if !(is(0, b"RIFF") && is(8, b"WAVEfmt ") && is(36, b"data")) {
        return refused(format!("{name} is not an audio file of a session"));
    }
    match (field(20, 2), field(22, 2), field(24, 4), field(34, 2)) {
        (Some(1), Some(1), Some(16_000), Some(16)) => {}
        (_, channels, rate, bits) => {
            return refused(format!(
                "{name} is not what a session holds — one channel, 16000 Hz, 16-bit: it has {} channel(s), {} Hz, {} bits",
                channels.unwrap_or(0),
                rate.unwrap_or(0),
                bits.unwrap_or(0)
            ));
        }
    }
    Ok(bytes[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
        .collect())
}

/// Hears a session — its two files, `me` then `others` — span by span, and writes its
/// transcript. `seconds` is the length of a span.
pub fn transcribe(folder: &Path, engine: &mut dyn Engine, language: Option<&str>, seconds: usize) -> io::Result<()> {
    let context = match fs::read(folder.join("context.txt")).map(String::from_utf8) {
        Ok(Ok(text)) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
        Ok(Err(_)) => return refused("context.txt is not text: it must be UTF-8".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let (name, model) = engine.name();
    let making = Making {
        engine: name,
        model,
        language: language.map(Into::into),
        context,
    };
    let mut transcript = Transcript::open(folder, &making)?;
    if transcript.is_complete() {
        return Ok(());
    }
    let mut longest = 0.0f64;
    for (speaker, file) in [(Speaker::Me, "me.wav"), (Speaker::Others, "others.wav")] {
        let heard = sound(&folder.join(file))?;
        let whole = heard.len() as f64 / RATE as f64;
        longest = longest.max(whole);
        let mut from = (transcript.covered(speaker) * RATE as f64).round() as usize;
        while from < heard.len() {
            let to = (from + seconds.max(1) * RATE).min(heard.len());
            let (start, mut until) = (from as f64 / RATE as f64, to as f64 / RATE as f64);
            let silent = heard[from..to].iter().all(|sample| sample.abs() < SILENCE);
            let mut segments = if silent {
                Vec::new()
            } else {
                engine.hear(&heard[from..to], language, making.context.as_deref())?
            };
            segments
                .iter_mut()
                .for_each(|segment| (segment.start, segment.end) = (segment.start + start, (segment.end + start).min(until)));
            segments.retain(|segment| segment.start < until);
            // A last segment that touches the end of the span may be half a sentence: it is
            // heard again with what follows — unless it opens the span, which would never end.
            if to < heard.len() && segments.last().is_some_and(|last| last.end > until - EDGE && last.start > start) {
                until = segments.pop().map_or(until, |last| last.start);
            }
            transcript.heard(speaker, &segments, until)?;
            from = ((until * RATE as f64).round() as usize).max(from + 1);
        }
    }
    transcript.complete(longest)
}
