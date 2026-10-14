//! The step, written once for every engine (ADR-0003): it reads a session's sound as
//! `contracts/session-audio/v1` or `v2` describes it, hands an engine one span at a time, and
//! writes what was heard. Stopped anywhere, it is run again and goes on from where it was.

use std::{fs, io, path::Path};

use crate::{Making, Segment, Speaker, Transcript, tracks};

/// Samples per second an engine hears: the first form's own rate, and what a track of the
/// second is brought to.
pub const RATE: usize = 16_000;
/// Below this share of full scale a span is silence: an engine asked to hear silence finds
/// words in it.
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

/// What a session's folder holds: whether it was stopped, its context, its two sounds.
/// The last says whether its sound is as its sources delivered it — the second form — or
/// centred, as the first form writes it.
pub type Session = (bool, Option<String>, [(Speaker, Vec<f32>); 2], bool);

/// The hundredths of a second of a track as delivered that hold sound. A source rests where
/// it rests, and what it did not deliver is heard as a line: where the track rests is taken
/// out of the measure — never out of the sound — as the capture does for its own watch, by
/// one pole at 20 Hz run from the track's first sample to its last. It starts from where the
/// track's first tenth of a second rests, and never again: a span is measured the same
/// wherever a run takes the session up.
fn sounding(heard: &[f32]) -> Vec<bool> {
    let keep = 1.0 - std::f32::consts::TAU * 20.0 / RATE as f32;
    let opening = &heard[..heard.len().min(RATE / 10)];
    let (mut last_in, mut last_out) = (opening.iter().sum::<f32>() / opening.len().max(1) as f32, 0.0f32);
    let mut moves = |&sample: &f32| {
        (last_in, last_out) = (sample, sample - last_in + keep * last_out);
        last_out.abs() >= SILENCE
    };
    // Every sample is measured, sound or not: the pole is run through them all.
    let hundredth = |samples: &[f32]| samples.iter().map(&mut moves).filter(|sound| *sound).count() > 0;
    heard.chunks(RATE / 100).map(hundredth).collect()
}

/// Reads a session's folder and judges it — or says which file is wrong, and what: a caller
/// asks this before it loads an engine.
pub fn session(folder: &Path) -> io::Result<Session> {
    read(folder).map(|(_, session)| session)
}

/// The same, with the form the session was recorded in — the audio contract `session.json`
/// names: a caller that reads both says which one it read.
pub fn read(folder: &Path) -> io::Result<(&'static str, Session)> {
    let session = fs::read(folder.join("session.json")).or_else(|error| refused(format!("session.json cannot be read ({error})")))?;
    let session: serde_json::Value = serde_json::from_slice(&session).unwrap_or_default();
    let form = match session["contract"].as_str() {
        Some("session-audio/v1") => "session-audio/v1",
        Some("session-audio/v2") => "session-audio/v2",
        _ => {
            return refused(
                "this folder is not a session this version reads: session.json names neither session-audio/v1 nor session-audio/v2".into(),
            );
        }
    };
    let context = match fs::read(folder.join("context.txt")).map(String::from_utf8) {
        Ok(Ok(text)) => Some(text.trim().to_string()).filter(|text| !text.is_empty()),
        Ok(Err(_)) => return refused("context.txt is not text: it must be UTF-8".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return refused(format!("context.txt cannot be read ({error})")),
    };
    let sounds = if form == "session-audio/v2" {
        // Each source as it was delivered, on the session's clock. A track the session does
        // not name is a side that said nothing; one it names and does not hold is said.
        let named = |name: &str| session["tracks"][name].is_object();
        if !named("local") && !named("remote") {
            return refused("session.json names no track this version reads: neither local nor remote".into());
        }
        let track = |name: &str| match named(name) {
            true => tracks::heard(&folder.join(format!("{name}.wav")), &tracks::clock(folder, name)),
            false => Ok(Vec::new()),
        };
        [(Speaker::Me, track("local")?), (Speaker::Others, track("remote")?)]
    } else {
        [
            (Speaker::Me, sound(&folder.join("me.wav"))?),
            (Speaker::Others, sound(&folder.join("others.wav"))?),
        ]
    };
    Ok((form, (session["stopped"].is_string(), context, sounds, form == "session-audio/v2")))
}

/// Hears a session — its two files, `me` then `others` — span by span, and writes its
/// transcript. `seconds` is the length of a span.
pub fn transcribe(folder: &Path, engine: &mut dyn Engine, language: Option<&str>, seconds: usize) -> io::Result<()> {
    // Everything is read and judged first: a run that refuses hears nothing and leaves no transcript.
    hear(folder, session(folder)?, engine, language, seconds)
}

/// The same, of a session already read: a caller that judged the session before loading its
/// engine does not read an hour of sound twice.
pub fn hear(folder: &Path, session: Session, engine: &mut dyn Engine, language: Option<&str>, seconds: usize) -> io::Result<()> {
    let (stopped, context, sounds, as_delivered) = session;
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
    for (speaker, heard) in &sounds {
        let sounding = as_delivered.then(|| sounding(heard));
        // A file still being written is heard up to its last whole hundredth; one that was stopped, to the nearest.
        let end = (heard.len() + if stopped { HUNDREDTH / 2 } else { 0 }) / HUNDREDTH;
        let mut from = (transcript.covered(*speaker) * 100.0).round() as usize;
        while from < end {
            let until = (from + span).min(end);
            let last = until == end;
            let sound = &heard[from * HUNDREDTH..(until * HUNDREDTH).min(heard.len())];
            // The first form is centred: silence is within a thousandth of zero, as it always was.
            let silent = match &sounding {
                Some(sounding) => !sounding[from.min(sounding.len())..until.min(sounding.len())].contains(&true),
                None => sound.iter().all(|sample| sample.abs() < SILENCE),
            };
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
                .collect();
            segments.sort_by_key(|(start, ..)| *start);
            // One that starts in the span's last hundredth is heard with what follows, from the hundredth before.
            let late = segments.last().is_some_and(|(start, ..)| *start == until) && !(last && stopped);
            segments.retain(|(start, ..)| !late || *start < until);
            // A last segment that touches the end of what was heard may be half a sentence. In the middle of a file it
            // is heard again with what follows, when that gains half a span: the step never crawls. At the end of a
            // file still being written it waits for the rest; at the end of one that was stopped there is no rest.
            let touching = segments
                .last()
                .filter(|(start, end, _)| end + EDGE > until && end > start)
                .map(|(start, ..)| *start);
            let again = touching.filter(|start| if last { !stopped } else { (start - from) * 2 >= until - from });
            let covered = again.inspect(|_| drop(segments.pop())).unwrap_or(until - usize::from(late));
            let segments: Vec<Segment> = segments
                .into_iter()
                .map(|(start, end, text)| (start as f64 / 100.0, end.max(start) as f64 / 100.0, text))
                .map(|(start, end, text)| Segment { start, end, text })
                .collect();
            transcript.heard(*speaker, &segments, covered as f64 / 100.0)?;
            from = if last && (late || again.is_some()) { end } else { covered };
        }
    }
    // A recording still going on, or ended by a crash, has no last word yet: the step is run again.
    if !stopped {
        return Ok(());
    }
    let longest = sounds.iter().map(|(_, heard)| heard.len()).max().unwrap_or(0);
    // The length as the last `covered` says it: counted the same way, to the nearest hundredth.
    transcript.complete(((longest + HUNDREDTH / 2) / HUNDREDTH) as f64 / 100.0)
}
