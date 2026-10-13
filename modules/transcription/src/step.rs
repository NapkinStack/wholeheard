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
/// A segment that ends this close to the end of a span — in hundredths of a second — may be
/// a sentence cut in two.
const EDGE: usize = 100;

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
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let bytes = fs::read(path).or_else(|error| refused(format!("{name} cannot be read ({error})")))?;
    let field = |at: usize, size: usize| bytes[at..at + size].iter().rev().fold(0u32, |n, &byte| n << 8 | u32::from(byte));
    let is = |at: usize, text: &[u8]| bytes.get(at..at + text.len()) == Some(text);
    if bytes.len() < 44 || !(is(0, b"RIFF") && is(8, b"WAVEfmt ") && is(36, b"data")) {
        return refused(format!("{name} is not an audio file of a session"));
    }
    let (channels, rate, bits) = (field(22, 2), field(24, 4), field(34, 2));
    if (field(20, 2), channels, rate, bits) != (1, 1, 16_000, 16) {
        let held = format!("it has {channels} channel(s), {rate} Hz, {bits} bits");
        return refused(format!(
            "{name} is not what a session holds — one channel, 16000 Hz, 16-bit: {held}"
        ));
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
    // Everything is read and judged first: a run that refuses hears nothing and leaves no transcript.
    let session = fs::read(folder.join("session.json")).or_else(|error| refused(format!("session.json cannot be read ({error})")))?;
    let session: serde_json::Value = serde_json::from_slice(&session).unwrap_or_default();
    if session["contract"] != "session-audio/v1" {
        return refused("this folder is not a session this version reads: session.json does not name session-audio/v1".into());
    }
    let context = match fs::read(folder.join("context.txt")).map(String::from_utf8) {
        Ok(Ok(text)) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
        Ok(Err(_)) => return refused("context.txt is not text: it must be UTF-8".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return refused(format!("context.txt cannot be read ({error})")),
    };
    let sounds = [
        (Speaker::Me, sound(&folder.join("me.wav"))?),
        (Speaker::Others, sound(&folder.join("others.wav"))?),
    ];
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
    // Every place in the session is counted in whole hundredths of a second, as the transcript
    // writes them: a run taken up starts exactly where the run before decided to stop.
    const HUNDREDTH: usize = RATE / 100;
    let span = seconds.clamp(1, 86_400) * 100;
    let stopped = session["stopped"].is_string();
    for (speaker, heard) in &sounds {
        // A file still being written is heard up to its last whole hundredth.
        let end = (heard.len() + if stopped { HUNDREDTH / 2 } else { 0 }) / HUNDREDTH;
        let mut from = (transcript.covered(*speaker) * 100.0).round() as usize;
        while from < end {
            let until = (from + span).min(end);
            let last = until == end;
            let sound = &heard[from * HUNDREDTH..if last { heard.len() } else { until * HUNDREDTH }];
            let silent = sound.iter().all(|sample| sample.abs() < SILENCE);
            let answer = if silent {
                None
            } else {
                Some(engine.hear(sound, language, making.context.as_deref())?)
            };
            // A time is the hundredth it falls on, or the next; one that is no time is the span's edge.
            let within = |seconds: f64| from + (seconds * 100.0 - 1e-6).ceil().clamp(0.0, (until - from) as f64) as usize;
            let mut segments: Vec<(usize, usize, String)> = answer
                .into_iter()
                .flatten()
                .map(|segment| (within(segment.start), within(segment.end), segment.text))
                .filter(|(start, ..)| *start < until)
                .collect();
            segments.sort_by_key(|(start, ..)| *start);
            // A last segment that touches the end of what was heard may be half a sentence. In the middle of a file it
            // is heard again with what follows, when that gains half a span: the step never crawls. At the end of a
            // file still being written it waits for the rest; at the end of one that was stopped there is no rest.
            // (One that fits in a single hundredth is whole: heard again, it would be lost.)
            let touching = segments
                .last()
                .filter(|(start, end, _)| end + EDGE > until && end > start)
                .map(|(start, ..)| *start);
            let again = touching.filter(|start| if last { !stopped } else { (start - from) * 2 >= until - from });
            let covered = again.inspect(|_| drop(segments.pop())).unwrap_or(until);
            let segments: Vec<Segment> = segments
                .into_iter()
                .map(|(start, end, text)| (start as f64 / 100.0, end.max(start) as f64 / 100.0, text))
                .map(|(start, end, text)| Segment { start, end, text })
                .collect();
            transcript.heard(*speaker, &segments, covered as f64 / 100.0)?;
            from = if again.is_some() && last { end } else { covered };
        }
    }
    // A recording still going on, or ended by a crash, has no last word yet: the step is run again.
    if !stopped {
        return Ok(());
    }
    let longest = sounds.iter().map(|(_, heard)| heard.len()).max().unwrap_or(0);
    transcript.complete(longest as f64 / RATE as f64)
}
