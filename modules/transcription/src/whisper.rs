//! The engine every system can run (ADR-0003): Whisper, through whisper.cpp, linked into
//! this program. The model is a file the user has; nothing is fetched here.

use std::{io, ops::Range, path::Path};

use whisper_rs::{DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    Segment,
    step::{Engine, RATE},
};

/// Where a segment of the engine is cut: from one word's time to the next's, longer than this,
/// in hundredths of a second. Whisper starts a segment where it starts a window, not where a
/// turn starts: cut where the words stop, a turn starts where it was said (cycle 02, D10). A
/// word's time is where it starts: half a second cut a sentence after each long word, one
/// second missed the turns; 0.8 holds the order and leaves a word alone twice a session.
const PAUSE: i64 = 80;

/// Sound is measured a tenth of a second at a time, in samples.
const TENTH: usize = RATE / 10;
/// How far below a speaker's own level a tenth of a second is still their voice, in decibels.
/// The speaker's level is that of the words the engine wrote: the loudest twentieth of their
/// tenths. On the bench a television in the room sits 15 to 20 dB below the speaker's voice.
const UNDER: f64 = 12.0;
/// The least a stretch at the speaker's level, with no word written, lasts to be heard again:
/// in tenths of a second. Between two words a voice rests a few tenths; a turn lasts seconds.
const UNWRITTEN: usize = 15;
/// The most a stretch is joined across where a word was written, in tenths of a second: a
/// stray word the engine wrote alone in a turn it otherwise jumped over, its margins with it,
/// is heard again with the turn. Kept written, it cut a name out of the turn (D10).
const STRAY: usize = 15;
/// How deep a stretch heard again is looked at again. Heard alone, Whisper may jump once more,
/// from where the stretch starts: a turn of 38 words gave its first three, and the rest heard
/// alone was written (D10).
const AGAIN: usize = 2;
/// A loop: a run of this many words written [`TIMES`] times or more in what a span gave.
/// Whisper hears each thirty seconds after the text of the thirty before; where a track falls
/// silent it may write that text again and again, and lose what follows (D10).
const RUN: usize = 4;
/// How often a run is written to be a loop. Twice, a speaker may say it: a decision, then
/// its summary.
const TIMES: usize = 3;

pub struct Whisper {
    engine: WhisperContext,
    model: String,
    threads: i32,
    /// Whether each word has its time, from the model's alignment heads.
    words: bool,
}

/// The alignment heads whisper.cpp holds for a model, known by what its file's header says:
/// its words, its layers heard and written, its pitch bands. Without them a word's time is the
/// engine's guess — the turns out of order as they were. Two models share one header, 1 and
/// 2 of large: neither is told, so neither is timed by word.
fn heads(model: &Path) -> Option<DtwModelPreset> {
    let mut header = [0u8; 48];
    std::io::Read::read_exact(&mut std::fs::File::open(model).ok()?, &mut header).ok()?;
    let field = |n: usize| i32::from_le_bytes(header[4 * n..4 * n + 4].try_into().unwrap_or_default());
    if field(0) as u32 != 0x6767_6d6c {
        return None;
    }
    // After the magic: the words; the sound side's context, width, heads, layers; the text side's the same; the bands.
    Some(match (field(1), field(5), field(9), field(10)) {
        (51864, 4, 4, 80) => DtwModelPreset::TinyEn,
        (51865, 4, 4, 80) => DtwModelPreset::Tiny,
        (51864, 6, 6, 80) => DtwModelPreset::BaseEn,
        (51865, 6, 6, 80) => DtwModelPreset::Base,
        (51864, 12, 12, 80) => DtwModelPreset::SmallEn,
        (51865, 12, 12, 80) => DtwModelPreset::Small,
        (51864, 24, 24, 80) => DtwModelPreset::MediumEn,
        (51865, 24, 24, 80) => DtwModelPreset::Medium,
        (51866, 32, 32, 128) => DtwModelPreset::LargeV3,
        (51866, 32, 4, 128) => DtwModelPreset::LargeV3Turbo,
        _ => return None,
    })
}

/// A segment cut where its words stop for longer than [`PAUSE`]: each piece starts at its
/// first word's time. A word is a token that starts with a space and holds a letter or a
/// digit: a mark of punctuation stays with the word before it, wherever it was heard.
fn cut(tokens: &[(i64, Vec<u8>)]) -> Vec<Segment> {
    let mut pieces: Vec<(i64, i64, Vec<u8>)> = Vec::new();
    for (at, bytes) in tokens {
        let word = bytes.first() == Some(&b' ') && bytes[1..].iter().any(|byte| byte.is_ascii_alphanumeric() || !byte.is_ascii());
        match pieces.last_mut() {
            Some((_, end, text)) if !(word && at - *end > PAUSE) => {
                *end = (*end).max(*at);
                text.extend(bytes);
            }
            _ => pieces.push((*at, *at, bytes.clone())),
        }
    }
    pieces
        .into_iter()
        .map(|(start, end, text)| Segment {
            start: start as f64 / 100.0,
            end: end as f64 / 100.0,
            text: String::from_utf8_lossy(&text).trim().to_string(),
        })
        .filter(|segment| !segment.text.is_empty())
        .collect()
}

/// Pieces added to what a span gave so far: a piece that does not start after the one before
/// is part of it. The engine may write a sentence twice, every word at one instant, and what
/// follows at that instant too: kept apart, the transcript would keep the first only — two
/// segments of one speaker never share a start.
fn joined(segments: &mut Vec<Segment>, pieces: Vec<Segment>) {
    for piece in pieces {
        match segments.last_mut() {
            Some(last) if piece.start <= last.start => {
                last.text = format!("{} {}", last.text, piece.text);
                last.end = last.end.max(piece.end);
            }
            _ => segments.push(piece),
        }
    }
}

/// Where a span holds a voice at its speaker's level and the engine wrote no word: the ranges
/// of samples to hear again, alone. Whisper sometimes jumps from one sentence to a later one,
/// over a whole turn whose sound is intact; heard alone, the turn is written (cycle 02, D10).
/// A written segment covers from a little before its first word to a little after the start
/// of its last: a word's time is where it starts. Where nothing was written, the speaker's
/// level is not known, and nothing is heard again: the level of a whole span, where someone
/// speaks a little, is that of the room.
fn unwritten(sound: &[f32], segments: &[Segment]) -> Vec<Range<usize>> {
    let level: Vec<f64> = sound
        .chunks(TENTH)
        .map(|tenth| {
            10.0 * (tenth.iter().map(|sample| f64::from(*sample).powi(2)).sum::<f64>() / tenth.len() as f64)
                .max(1e-12)
                .log10()
        })
        .collect();
    let mut written = vec![false; level.len()];
    for segment in segments {
        let from = ((segment.start - 0.3).max(0.0) * 10.0) as usize;
        let until = (((segment.end + 0.6).max(0.0) * 10.0) as usize).min(level.len());
        written.iter_mut().take(until).skip(from).for_each(|tenth| *tenth = true);
    }
    let mut words: Vec<f64> = level
        .iter()
        .zip(&written)
        .filter(|(_, written)| **written)
        .map(|(loud, _)| *loud)
        .collect();
    if words.is_empty() {
        return Vec::new();
    }
    words.sort_by(f64::total_cmp);
    let own = words[(words.len() * 95 / 100).min(words.len() - 1)];
    // Tenths at the speaker's level and not written, one stretch across rests of up to three
    // tenths, and across a stray word written alone in it, with its margins.
    let mut stretches: Vec<Range<usize>> = Vec::new();
    for (at, loud) in level.iter().enumerate() {
        if written[at] || *loud < own - UNDER {
            continue;
        }
        match stretches.last_mut() {
            Some(last) if at - last.end <= STRAY && written[last.end..at].iter().filter(|written| !**written).count() <= 3 => {
                last.end = at + 1
            }
            _ => stretches.push(at..at + 1),
        }
    }
    // Heard with three tenths on each side, the start of a word and the end of the last.
    stretches
        .into_iter()
        .filter(|stretch| stretch.len() >= UNWRITTEN)
        .map(|stretch| stretch.start.saturating_sub(3) * TENTH..((stretch.end + 3) * TENTH).min(sound.len()))
        .collect()
}

fn failed<T>(what: &str, error: impl std::fmt::Display) -> io::Result<T> {
    Err(io::Error::other(format!("{what} ({error})")))
}

/// Whether Whisper knows a language by this code — `fr`, `en`. Its word for "detect it" is not
/// one: leaving the language out says that.
pub fn knows(language: &str) -> bool {
    // whisper.cpp says what it does not know on the error stream: kept quiet, as in `load`.
    whisper_rs::install_logging_hooks();
    // A code, not a language's English name, which whisper.cpp takes too: one language, one word in a header.
    whisper_rs::get_lang_id(language).and_then(whisper_rs::get_lang_str) == Some(language)
}

impl Whisper {
    /// Loads a model in whisper.cpp's format. It is named in the transcript by its file's name.
    pub fn load(model: &Path) -> io::Result<Self> {
        // The file's name, never its path: a sentence that fails is logged (ADR-0007), and a
        // path names whose computer, or a session given by mistake.
        let named = model
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !model.is_file() || std::fs::File::open(model).is_err() {
            return failed("the model is not a file that can be read", &named);
        }
        // whisper.cpp writes its own account of the model on the error stream: kept quiet,
        // so that what this program says there is one sentence.
        whisper_rs::install_logging_hooks();
        let Some(path) = model.to_str() else {
            return failed("the model's path is not text", &named);
        };
        let mut settings = WhisperContextParameters::default();
        let preset = heads(model);
        let words = preset.is_some();
        if let Some(model_preset) = preset {
            settings.dtw_parameters(DtwParameters {
                mode: DtwMode::ModelPreset { model_preset },
                ..Default::default()
            });
        }
        let engine = match WhisperContext::new_with_params(path, settings) {
            Ok(engine) => engine,
            Err(error) => {
                return failed(
                    "the model could not be loaded: it is not a Whisper model in whisper.cpp's format",
                    error,
                );
            }
        };
        // One thread per physical core: one per logical core made the same run 2.7 times
        // longer on the laptop this was measured on (cycle 01, D4).
        let logical = std::thread::available_parallelism().map_or(2, |n| n.get());
        let model = model.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        Ok(Self {
            engine,
            model,
            threads: (logical / 2).max(1) as i32,
            words,
        })
    }

    /// Whether each word has its time, or each segment only: what is known of a model.
    pub fn times_words(&self) -> bool {
        self.words
    }
}

impl Engine for Whisper {
    fn name(&self) -> (String, String) {
        ("whisper.cpp".into(), self.model.clone())
    }

    fn hear(&mut self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> io::Result<Vec<Segment>> {
        listened(sound, &mut |stretch| self.heard(stretch, language, context))
    }
}

/// Where the engine wrote a loop: the segments that hold one of its runs, one after another,
/// with the pieces of its words beside them, and the samples from the end of the segment before
/// them to the start of the segment after, to hear again alone. Heard alone, a stretch is heard
/// after no text of the window before: on the bench, the two draws in eight that looped went from
/// 113 and 139 words wrong to 22 and 21, the others unchanged (D10).
fn looped(sound: &[f32], segments: &[Segment]) -> Vec<(Range<usize>, Range<usize>)> {
    let words: Vec<Vec<String>> = segments
        .iter()
        .map(|segment| {
            segment
                .text
                .split_whitespace()
                .map(|word| word.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
                .filter(|word| !word.is_empty())
                .collect()
        })
        .collect();
    let runs: Vec<Vec<String>> = words
        .iter()
        .map(|words| words.windows(RUN).map(|run| run.join(" ")).collect())
        .collect();
    let mut times: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for run in runs.iter().flatten() {
        *times.entry(run).or_default() += 1;
    }
    let mut looping: Vec<bool> = runs
        .iter()
        .map(|runs| runs.iter().any(|run| times[run.as_str()] >= TIMES))
        .collect();
    // The engine cuts a loop where its words' times stop: a piece of a few of its words, beside
    // it, is of the loop too — and a word that is none of them, said at its edge, is not.
    let looped: std::collections::HashSet<&str> = runs
        .iter()
        .zip(&looping)
        .filter(|(_, looping)| **looping)
        .flat_map(|(runs, _)| runs.iter().filter(|run| times[run.as_str()] >= TIMES))
        .flat_map(|run| run.split(' '))
        .collect();
    let piece = |at: usize| !words[at].is_empty() && words[at].iter().all(|word| looped.contains(word.as_str()));
    for at in 1..segments.len() {
        looping[at] = looping[at] || (looping[at - 1] && piece(at));
    }
    for at in (0..segments.len().saturating_sub(1)).rev() {
        looping[at] = looping[at] || (looping[at + 1] && piece(at));
    }
    let sample = |seconds: f64| ((seconds.max(0.0) * RATE as f64).round() as usize).min(sound.len());
    let mut loops = Vec::new();
    let mut at = 0;
    while at < segments.len() {
        if !looping[at] {
            at += 1;
            continue;
        }
        let first = at;
        while at < segments.len() && looping[at] {
            at += 1;
        }
        let from = first
            .checked_sub(1)
            .map_or(0.0, |before| segments[before].end.min(segments[first].start));
        let until = segments.get(at).map_or(sound.len(), |after| sample(after.start));
        loops.push((sample(from)..until, first..at));
    }
    loops
}

/// What a span gave, with each [`looped`] stretch heard again alone in place of the loop's
/// segments — `rounds` deep: what a stretch gives is looked at the same way. The segments
/// around a loop stay; what a stretch gives as marks alone (`...`) is not written: where a loop
/// was, the track is often silent.
fn unlooped(
    sound: &[f32],
    mut segments: Vec<Segment>,
    hear: &mut dyn FnMut(&[f32]) -> io::Result<Vec<Segment>>,
    rounds: usize,
) -> io::Result<Vec<Segment>> {
    if rounds == 0 {
        return Ok(segments);
    }
    let loops = looped(sound, &segments);
    if loops.is_empty() {
        return Ok(segments);
    }
    // By its place in the list: a one-word segment ends where it starts, and by time one at a
    // loop's edge would leave with it.
    let mut at = 0;
    segments.retain(|_| {
        at += 1;
        !loops.iter().any(|(_, written)| written.contains(&(at - 1)))
    });
    let seconds = |sample: usize| sample as f64 / RATE as f64;
    for (stretch, _) in loops {
        let offset = seconds(stretch.start);
        let mut again = hear(&sound[stretch.clone()])?;
        again.retain(|segment| segment.text.chars().any(char::is_alphanumeric));
        for segment in unlooped(&sound[stretch], again, hear, rounds - 1)? {
            segments.push(Segment {
                start: segment.start + offset,
                end: segment.end + offset,
                text: segment.text,
            });
        }
    }
    segments.sort_by(|one, other| one.start.total_cmp(&other.start));
    let mut ordered = Vec::new();
    joined(&mut ordered, segments);
    Ok(ordered)
}

/// What a span gives, heard by `hear`: a loop heard again first, then what the engine left
/// unwritten, as it is then — and in what a stretch heard again gives, the same.
fn listened(sound: &[f32], hear: &mut dyn FnMut(&[f32]) -> io::Result<Vec<Segment>>) -> io::Result<Vec<Segment>> {
    let segments = hear(sound)?;
    let segments = unlooped(sound, segments, hear, AGAIN)?;
    let mut heard = heard_again(
        sound,
        segments,
        &mut |stretch| {
            let segments = hear(stretch)?;
            unlooped(stretch, segments, hear, AGAIN)
        },
        AGAIN,
    )?;
    // What holds no letter nor digit — "..." on a track's silence — is no line (#355). Left out
    // last: until then it stands where the engine wrote, as the loops and the stretches read it.
    heard.retain(|segment| segment.text.chars().any(char::is_alphanumeric));
    Ok(heard)
}

/// What a span gave, with each [`unwritten`] stretch heard again alone and put back in its
/// place, in the order of the starts — `rounds` deep: what a stretch gives is looked at the
/// same way. A stray word inside a stretch leaves for what the stretch gives there.
fn heard_again(
    sound: &[f32],
    mut segments: Vec<Segment>,
    hear: &mut dyn FnMut(&[f32]) -> io::Result<Vec<Segment>>,
    rounds: usize,
) -> io::Result<Vec<Segment>> {
    if rounds == 0 {
        return Ok(segments);
    }
    let stretches = unwritten(sound, &segments);
    let seconds = |sample: usize| sample as f64 / RATE as f64;
    segments.retain(|segment| {
        !stretches
            .iter()
            .any(|stretch| segment.start >= seconds(stretch.start) && segment.end <= seconds(stretch.end))
    });
    for stretch in stretches {
        let offset = seconds(stretch.start);
        let again = hear(&sound[stretch.clone()])?;
        for segment in heard_again(&sound[stretch], again, hear, rounds - 1)? {
            segments.push(Segment {
                start: segment.start + offset,
                end: segment.end + offset,
                text: segment.text,
            });
        }
    }
    segments.sort_by(|one, other| one.start.total_cmp(&other.start));
    let mut ordered = Vec::new();
    joined(&mut ordered, segments);
    Ok(ordered)
}

impl Whisper {
    /// What the engine heard of a stretch of sound, as it gave it: seconds from its start.
    fn heard(&mut self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> io::Result<Vec<Segment>> {
        let mut state = self
            .engine
            .create_state()
            .or_else(|error| failed("the engine could not start", error))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(language);
        params.set_n_threads(self.threads);
        // Each span is heard for itself: the same span gives the same segments, whatever came before.
        params.set_no_context(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_token_timestamps(self.words);
        if let Some(context) = context {
            params.set_initial_prompt(context);
        }
        state
            .full(params, sound)
            .or_else(|error| failed("the engine could not hear this span", error))?;
        let mut segments: Vec<Segment> = Vec::new();
        if self.words {
            let ends = self.engine.token_eot();
            for segment in state.as_iter() {
                // A token's bytes may be half a letter: the text is read once the piece is whole.
                let tokens: Vec<(i64, Vec<u8>)> = (0..segment.n_tokens())
                    .filter_map(|index| segment.get_token(index))
                    .filter(|token| token.token_id() < ends)
                    .map(|token| {
                        let data = token.token_data();
                        let at = if data.t_dtw >= 0 { data.t_dtw } else { data.t0 };
                        (at, token.to_bytes().unwrap_or_default().to_vec())
                    })
                    .collect();
                joined(&mut segments, cut(&tokens));
            }
            return Ok(segments);
        }
        for segment in state.as_iter() {
            let text = segment
                .to_str_lossy()
                .or_else(|error| failed("the engine gave a text that cannot be read", error))?;
            segments.push(Segment {
                start: segment.start_timestamp() as f64 / 100.0,
                end: segment.end_timestamp() as f64 / 100.0,
                text: text.trim().to_string(),
            });
        }
        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(said: &[(i64, &str)]) -> Vec<(i64, Vec<u8>)> {
        said.iter().map(|(at, text)| (*at, text.as_bytes().to_vec())).collect()
    }

    #[test]
    fn a_segment_is_cut_where_its_words_stop_and_each_piece_starts_at_its_first_word() {
        let said = tokens(&[
            (130, " Parfait"),
            (160, "."),
            (190, " Marc"),
            (240, ","),
            (260, " tu"),
            (420, " Oui"),
            (450, ","),
            (470, " la"),
        ]);
        let cut = cut(&said);
        let pieces: Vec<(f64, f64, &str)> = cut.iter().map(|s| (s.start, s.end, s.text.as_str())).collect();
        assert_eq!(pieces, [(1.3, 2.6, "Parfait. Marc, tu"), (4.2, 4.7, "Oui, la")]);
    }

    #[test]
    fn a_mark_of_punctuation_heard_late_stays_with_the_word_before_it() {
        let cut = cut(&tokens(&[(100, " calendrier"), (190, " ?"), (300, " Oui")]));
        let texts: Vec<&str> = cut.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["calendrier ?", "Oui"]);
    }

    #[test]
    fn what_holds_no_word_is_no_line_of_the_transcript() {
        // The rehearsal of D23: "..." alone, on a track's silence, shown as a line (#355).
        let sound = voiced(10.0, 0.3, &[(3.0, 5.0)]);
        let mut first = true;
        let mut hear = |_: &[f32]| -> io::Result<Vec<Segment>> {
            Ok(match std::mem::replace(&mut first, false) {
                true => vec![
                    wrote(0.0, 0.0, "..."),
                    wrote(3.0, 5.0, "Oui."),
                    wrote(6.0, 6.5, "42"),
                    wrote(8.0, 8.0, "…"),
                ],
                false => vec![],
            })
        };
        let heard = listened(&sound, &mut hear).unwrap();
        let texts: Vec<&str> = heard.iter().map(|s| s.text.as_str()).collect();
        // A figure said is a word.
        assert_eq!(texts, ["Oui.", "42"]);
    }

    #[test]
    fn what_holds_no_word_still_stands_where_the_engine_wrote_while_the_span_is_read() {
        // Left out last: over a voice, "..." is what the engine wrote there, and that stretch is
        // not heard again, as before #355.
        let sound = voiced(10.0, 0.3, &[(3.0, 5.0), (7.0, 9.0)]);
        let mut heard_times = 0;
        let mut hear = |_: &[f32]| -> io::Result<Vec<Segment>> {
            heard_times += 1;
            Ok(vec![wrote(3.0, 5.0, "Oui."), wrote(7.0, 9.0, "...")])
        };
        let heard = listened(&sound, &mut hear).unwrap();
        let texts: Vec<&str> = heard.iter().map(|s| s.text.as_str()).collect();
        assert_eq!((texts, heard_times), (vec!["Oui."], 1));
    }

    #[test]
    fn segments_the_engine_wrote_at_one_instant_are_one_with_what_follows_them_there() {
        // A sentence written twice in two segments, every word at one time, then the sentence
        // said there in a third: one segment, none of its words lost to a start already taken.
        let twice = tokens(&[(17130, " on"), (17130, " a"), (17130, " un"), (17130, " guide")]);
        let said = tokens(&[(17130, " il"), (17150, " faudrait"), (17190, " aussi")]);
        let mut segments = Vec::new();
        for segment in [&twice, &twice, &said] {
            joined(&mut segments, cut(segment));
        }
        let texts: Vec<(f64, &str)> = segments.iter().map(|s| (s.start, s.text.as_str())).collect();
        assert_eq!(texts, [(171.3, "on a un guide on a un guide il faudrait aussi")]);
        assert_eq!(segments[0].end, 171.9);
        // One that starts later is a segment of its own.
        joined(&mut segments, cut(&tokens(&[(17500, " Oui")])));
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn a_pause_of_exactly_eight_tenths_of_a_second_does_not_cut() {
        assert_eq!(cut(&tokens(&[(100, " un"), (180, " deux")])).len(), 1);
        assert_eq!(cut(&tokens(&[(100, " un"), (181, " deux")])).len(), 2);
    }

    #[test]
    fn a_letter_split_across_two_tokens_is_read_whole() {
        // Far apart in time, the halves of a letter are still one word: neither starts with a space.
        let mut said = tokens(&[(100, " d")]);
        said.push((200, vec![0xc3]));
        said.push((300, vec![0xa9, b'j', b'a']));
        said.push((310, " là".as_bytes().to_vec()));
        said.push((400, " à".as_bytes().to_vec()));
        let texts: Vec<String> = cut(&said).into_iter().map(|s| s.text).collect();
        assert_eq!(texts, ["déja là", "à"]);
    }

    #[test]
    fn a_model_is_known_by_its_header_and_a_header_two_models_share_is_not() {
        let folder = tempfile::tempdir().unwrap();
        let header = |fields: [i32; 11]| {
            let mut bytes = 0x6767_6d6cu32.to_le_bytes().to_vec();
            bytes.extend(fields.iter().flat_map(|field| field.to_le_bytes()));
            let path = folder
                .path()
                .join(format!("{}.bin", fields.iter().map(i32::to_string).collect::<Vec<_>>().join("-")));
            std::fs::write(&path, bytes).unwrap();
            path
        };
        // The models whisper.cpp holds heads for, by what their headers say: words, layers heard, layers written, bands.
        let models = [
            ((51864, 4, 4, 80), "TinyEn"),
            ((51865, 4, 4, 80), "Tiny"),
            ((51864, 6, 6, 80), "BaseEn"),
            ((51865, 6, 6, 80), "Base"),
            ((51864, 12, 12, 80), "SmallEn"),
            ((51865, 12, 12, 80), "Small"),
            ((51864, 24, 24, 80), "MediumEn"),
            ((51865, 24, 24, 80), "Medium"),
            ((51866, 32, 32, 128), "LargeV3"),
            ((51866, 32, 4, 128), "LargeV3Turbo"),
        ];
        for ((words, heard, written, bands), preset) in models {
            let model = header([words, 1500, 1280, 20, heard, 448, 1280, 20, written, bands, 1]);
            assert_eq!(
                heads(&model).map(|found| format!("{found:?}")),
                Some(preset.to_string()),
                "{preset}"
            );
        }
        let large_v2 = header([51865, 1500, 1280, 20, 32, 448, 1280, 20, 32, 80, 1]);
        assert!(heads(&large_v2).is_none());
        // A header of the right fields after another magic is no model of whisper.cpp's.
        let mut other = std::fs::read(header([51866, 1500, 1280, 20, 32, 448, 1280, 20, 4, 128, 1])).unwrap();
        other[..4].copy_from_slice(b"GGUF");
        let other_magic = folder.path().join("other.bin");
        std::fs::write(&other_magic, other).unwrap();
        assert!(heads(&other_magic).is_none());
        let not_a_model = folder.path().join("short.bin");
        std::fs::write(&not_a_model, b"ggml").unwrap();
        assert!(heads(&not_a_model).is_none());
    }

    /// A sound of `seconds` with a voice — a tone at `level` of full scale — over each stretch given in seconds.
    fn voiced(seconds: f64, level: f32, stretches: &[(f64, f64)]) -> Vec<f32> {
        let mut sound = vec![0.0_f32; (seconds * RATE as f64) as usize];
        for (from, until) in stretches {
            let (from, until) = ((from * RATE as f64) as usize, (until * RATE as f64) as usize);
            for (at, sample) in sound.iter_mut().enumerate().take(until).skip(from) {
                *sample += level * (at as f32 * 0.07).sin();
            }
        }
        sound
    }

    /// A sound of `seconds` with a tone at each level over its stretch, in seconds.
    fn mixed(seconds: f64, tones: &[(f32, f64, f64)]) -> Vec<f32> {
        let mut sound = vec![0.0_f32; (seconds * RATE as f64) as usize];
        for (level, from, until) in tones {
            for (at, tone) in voiced(seconds, *level, &[(*from, *until)]).into_iter().enumerate() {
                sound[at] += tone;
            }
        }
        sound
    }

    fn said(start: f64, end: f64) -> Segment {
        Segment {
            start,
            end,
            text: "written".into(),
        }
    }

    fn wrote(start: f64, end: f64, text: &str) -> Segment {
        Segment {
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn a_turn_at_the_speakers_level_with_no_word_written_is_heard_again_with_a_margin() {
        let sound = voiced(60.0, 0.3, &[(2.0, 10.0), (20.0, 34.0), (40.0, 50.0)]);
        // The engine wrote the first and the last turn, and jumped over the second.
        let again = unwritten(&sound, &[said(2.0, 9.5), said(40.0, 49.5)]);
        assert_eq!(again, vec![197 * TENTH..343 * TENTH]);
    }

    #[test]
    fn what_was_written_is_not_heard_again() {
        let sound = voiced(60.0, 0.3, &[(2.0, 10.0), (20.0, 34.0)]);
        // The last word starts within six tenths of the end of the voice: its own length.
        assert!(unwritten(&sound, &[said(2.2, 9.5), said(20.0, 33.5)]).is_empty());
        // Its last word started 1.6 s before the voice ended: one second is left, too short to be a turn.
        assert!(unwritten(&sound, &[said(2.2, 9.5), said(20.0, 32.4)]).is_empty());
        assert!(unwritten(&[], &[]).is_empty());
    }

    #[test]
    fn a_sound_well_below_the_speakers_level_is_not_heard_again() {
        // The speaker at 0.3, a television at 0.0735 (12.2 dB under), never written.
        let sound = mixed(60.0, &[(0.3, 2.0, 30.0), (0.0735, 31.0, 59.0)]);
        assert!(unwritten(&sound, &[said(2.0, 29.5)]).is_empty());
        // At 11.8 dB under, it is taken for the speaker.
        let sound = mixed(60.0, &[(0.3, 2.0, 30.0), (0.077, 31.0, 59.0)]);
        assert_eq!(unwritten(&sound, &[said(2.0, 29.5)]), vec![307 * TENTH..593 * TENTH]);
    }

    #[test]
    fn where_the_engine_wrote_nothing_or_the_speaker_says_little_the_room_is_not_heard_again() {
        // A quiet room all along — a hum at 0.01 — and nothing written: no level to start from.
        let hum = voiced(300.0, 0.01, &[(0.0, 300.0)]);
        assert!(unwritten(&hum, &[]).is_empty());
        // One sentence of three seconds in five minutes of that room: the room is far under it.
        let mut sound = hum.clone();
        for (at, voice) in voiced(300.0, 0.3, &[(100.0, 103.0)]).into_iter().enumerate() {
            sound[at] += voice;
        }
        assert!(unwritten(&sound, &[said(100.0, 102.5)]).is_empty());
    }

    #[test]
    fn a_stretch_shorter_than_a_second_and_a_half_is_left_and_rests_between_words_do_not_split_one() {
        let sound = voiced(30.0, 0.3, &[(2.0, 10.0), (12.0, 13.4), (15.0, 15.8), (16.0, 16.8)]);
        // 1.4 s alone is left; two stretches of 0.8 s with a rest of 0.2 s are one of 1.8 s.
        let again = unwritten(&sound, &[said(2.0, 9.5)]);
        assert_eq!(again, vec![147 * TENTH..171 * TENTH]);
    }

    #[test]
    fn a_stretch_of_a_second_and_a_half_is_heard_again_and_a_rest_of_four_tenths_splits_one() {
        let written = [said(2.0, 9.5)];
        let sound = voiced(30.0, 0.3, &[(2.0, 10.0), (12.0, 13.5)]);
        assert_eq!(unwritten(&sound, &written), vec![117 * TENTH..138 * TENTH]);
        // Two stretches of 0.8 s: one across a rest of three tenths, two too short across four.
        let sound = voiced(30.0, 0.3, &[(2.0, 10.0), (15.0, 15.8), (16.1, 16.9)]);
        assert_eq!(unwritten(&sound, &written), vec![147 * TENTH..172 * TENTH]);
        let sound = voiced(30.0, 0.3, &[(2.0, 10.0), (15.0, 15.8), (16.2, 17.0)]);
        assert!(unwritten(&sound, &written).is_empty());
    }

    #[test]
    fn a_segment_covers_three_tenths_before_its_first_word_and_six_after_the_start_of_its_last() {
        let sound = voiced(40.0, 0.3, &[(17.0, 33.0)]);
        let again = unwritten(&sound, &[said(20.05, 30.05)]);
        assert_eq!(again, vec![167 * TENTH..200 * TENTH, 303 * TENTH..333 * TENTH]);
    }

    #[test]
    fn the_speakers_level_is_that_of_the_loudest_twentieth_of_the_tenths_written() {
        // Written: eight silent tenths of margin, ninety quiet, five at 0.1, five at 1.0. The
        // twentieth from the top is at 0.1: a voice 6 dB under it is theirs, 16 dB under is not.
        let sound = mixed(
            60.0,
            &[
                (0.01, 10.0, 19.0),
                (0.1, 19.0, 19.5),
                (1.0, 19.5, 20.0),
                (0.05, 30.0, 35.0),
                (0.016, 40.0, 45.0),
            ],
        );
        assert_eq!(unwritten(&sound, &[said(10.05, 19.95)]), vec![297 * TENTH..353 * TENTH]);
    }

    #[test]
    fn a_stray_word_written_alone_in_a_turn_jumped_over_is_heard_again_with_the_turn() {
        let sound = voiced(40.0, 0.3, &[(2.0, 10.0), (20.0, 30.0)]);
        // Its margins with it, fifteen tenths: one stretch. Sixteen are a word of the speaker's.
        let again = unwritten(&sound, &[said(2.0, 9.5), said(24.0, 24.65)]);
        assert_eq!(again, vec![197 * TENTH..303 * TENTH]);
        let again = unwritten(&sound, &[said(2.0, 9.5), said(24.0, 24.75)]);
        assert_eq!(again, vec![197 * TENTH..240 * TENTH, 250 * TENTH..303 * TENTH]);
    }

    #[test]
    fn what_a_stretch_gives_takes_its_place_in_time_and_the_stray_word_in_it_leaves() {
        let sound = voiced(60.0, 0.3, &[(2.0, 10.0), (20.0, 30.0), (40.0, 45.0)]);
        let segments = vec![said(2.0, 9.5), said(24.0, 24.3), said(40.0, 44.5)];
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(vec![Segment {
                start: 0.5,
                end: 9.0,
                text: "again".into(),
            }])
        };
        let again = heard_again(&sound, segments, &mut hear, AGAIN).unwrap();
        assert_eq!(lengths, [106 * TENTH]);
        let times: Vec<(i64, i64, &str)> = again
            .iter()
            .map(|s| ((s.start * 10.0).round() as i64, (s.end * 10.0).round() as i64, s.text.as_str()))
            .collect();
        assert_eq!(times, [(20, 95, "written"), (202, 287, "again"), (400, 445, "written")]);
    }

    #[test]
    fn a_stretch_heard_again_is_looked_at_again_twice_deep() {
        // An engine that writes only the first two seconds of what it hears.
        let sound = voiced(60.0, 0.3, &[(2.0, 10.0), (20.0, 30.0)]);
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(vec![Segment {
                start: 0.5,
                end: 2.0,
                text: "first".into(),
            }])
        };
        let again = heard_again(&sound, vec![said(2.0, 9.5)], &mut hear, AGAIN).unwrap();
        // The turn heard alone, then what it left unwritten heard alone, and no deeper.
        assert_eq!(lengths, [106 * TENTH, 83 * TENTH]);
        let starts: Vec<(i64, &str)> = again.iter().map(|s| ((s.start * 10.0).round() as i64, s.text.as_str())).collect();
        assert_eq!(starts, [(20, "written"), (202, "first"), (225, "first")]);
    }

    #[test]
    fn a_run_written_twice_or_of_three_words_is_not_a_loop_and_one_segment_may_hold_a_loop_alone() {
        let twice = [wrote(2.0, 4.0, "on retient le lundi"), wrote(30.0, 33.0, "on retient le lundi 16")];
        assert_eq!(looped(&voiced(40.0, 0.3, &[]), &twice), vec![]);
        let three = [
            wrote(2.0, 4.0, "la première semaine"),
            wrote(10.0, 12.0, "la première semaine"),
            wrote(20.0, 22.0, "la première semaine"),
        ];
        assert_eq!(looped(&voiced(40.0, 0.3, &[]), &three), vec![]);
        // A mark alone between two words is no word: the run is the same.
        let marked = [
            wrote(2.0, 4.0, "je réponds aux clients"),
            wrote(10.0, 12.0, "je réponds — aux clients"),
            wrote(20.0, 22.0, "je réponds aux clients"),
        ];
        assert_eq!(looped(&voiced(40.0, 0.3, &[]), &marked), vec![(0..40 * RATE, 0..3)]);
        // The segment before ends after the loop starts: the stretch starts with the loop.
        let alone = [
            wrote(2.0, 5.5, "parfait"),
            wrote(
                5.0,
                9.0,
                "les deux semaines je vais les deux semaines je vais les deux semaines je vais",
            ),
        ];
        assert_eq!(looped(&voiced(40.0, 0.3, &[]), &alone), vec![(5 * RATE..40 * RATE, 1..2)]);
    }

    #[test]
    fn a_loop_from_the_first_segment_starts_with_the_span_and_digits_are_words() {
        let segments = [
            wrote(3.0, 5.0, "le 12 novembre le 12 novembre le 12 novembre le 12"),
            wrote(8.0, 9.0, "oui"),
        ];
        assert_eq!(looped(&voiced(20.0, 0.3, &[]), &segments), vec![(0..8 * RATE, 0..1)]);
        // Without its digits, the run is three words: no loop.
        let words = [wrote(3.0, 5.0, "le novembre le novembre le novembre le"), wrote(8.0, 9.0, "oui")];
        assert_eq!(looped(&voiced(20.0, 0.3, &[]), &words), vec![]);
    }

    #[test]
    fn a_piece_of_a_loops_words_beside_it_is_of_the_loop_and_another_word_is_not() {
        let segments = [
            wrote(2.0, 9.5, "Et pour le support ?"),
            wrote(10.0, 10.0, "je"),
            wrote(11.0, 13.0, "je réponds aux clients"),
            wrote(13.0, 15.0, "je réponds aux clients"),
            wrote(15.0, 17.0, "je réponds aux clients la première"),
            wrote(18.0, 18.0, "aux clients"),
            wrote(18.5, 18.5, "la première"),
            wrote(19.0, 19.0, "clients"),
            wrote(20.0, 20.0, "Oui."),
            wrote(25.0, 26.0, "réponds"),
        ];
        // The pieces of its words beside it are of the loop; "la première" is none of them, and
        // ends it. The last word, after "Oui.", is not beside it.
        assert_eq!(looped(&voiced(40.0, 0.3, &[]), &segments), vec![(95 * TENTH..185 * TENTH, 1..6)]);
        // Marks alone, or a word of the loop with another, are no piece of it.
        let after = |text: &str| {
            let mut segments = segments[..5].to_vec();
            segments.push(wrote(18.0, 18.0, text));
            looped(&voiced(40.0, 0.3, &[]), &segments)
        };
        assert_eq!(after("..."), vec![(95 * TENTH..180 * TENTH, 1..5)]);
        assert_eq!(after("clients et budget"), vec![(95 * TENTH..180 * TENTH, 1..5)]);
    }

    fn times(segments: &[Segment]) -> Vec<(i64, i64, &str)> {
        segments
            .iter()
            .map(|s| ((s.start * 10.0).round() as i64, (s.end * 10.0).round() as i64, s.text.as_str()))
            .collect()
    }

    #[test]
    fn a_loop_is_heard_again_alone_from_the_segment_before_to_the_one_after_in_place_of_its_own() {
        let sound = voiced(60.0, 0.3, &[(2.0, 10.0), (20.0, 30.0), (40.0, 45.0)]);
        let segments = vec![
            wrote(2.0, 9.5, "Et pour le support ?"),
            wrote(20.0, 21.0, "Je réponds aux clients."),
            wrote(21.0, 22.0, "je réponds aux clients"),
            wrote(22.0, 29.5, "je réponds aux clients la première semaine"),
            said(40.0, 44.5),
        ];
        assert_eq!(looped(&sound, &segments), vec![(95 * TENTH..400 * TENTH, 1..4)]);
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(vec![wrote(10.5, 20.0, "again")])
        };
        let again = unlooped(&sound, segments, &mut hear, AGAIN).unwrap();
        assert_eq!(lengths, [305 * TENTH]);
        assert_eq!(
            times(&again),
            [(20, 95, "Et pour le support ?"), (200, 295, "again"), (400, 445, "written")]
        );
    }

    #[test]
    fn a_word_at_a_loops_edge_stays_and_marks_alone_heard_in_its_place_leave() {
        let sound = voiced(40.0, 0.3, &[(2.0, 30.0)]);
        let segments = vec![
            wrote(2.0, 19.5, "Bien, on avance sur le planning et le budget du lancement."),
            wrote(19.6, 19.6, "Bien."),
            wrote(20.0, 22.0, "je réponds aux clients"),
            wrote(22.0, 24.0, "je réponds aux clients"),
            wrote(24.0, 29.0, "je réponds aux clients"),
            wrote(29.5, 29.5, "Oui."),
        ];
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(vec![wrote(0.2, 0.4, "..."), wrote(0.5, 9.4, "said there")])
        };
        let again = unlooped(&sound, segments, &mut hear, AGAIN).unwrap();
        assert_eq!(lengths, [99 * TENTH]);
        assert_eq!(
            times(&again),
            [
                (20, 195, "Bien, on avance sur le planning et le budget du lancement."),
                (196, 196, "Bien."),
                (201, 290, "said there"),
                (295, 295, "Oui.")
            ]
        );
    }

    #[test]
    fn a_loop_heard_again_is_looked_at_again_twice_deep_and_a_span_without_one_is_left_as_it_is() {
        let sound = voiced(40.0, 0.3, &[(2.0, 30.0)]);
        let looping = || {
            vec![
                wrote(0.0, 2.0, "on fait le point"),
                wrote(2.0, 4.0, "on fait le point"),
                wrote(4.0, 9.0, "on fait le point"),
            ]
        };
        // An engine that loops each time it hears: the span's loop heard again twice, and no
        // deeper; the turn left unwritten after it as well, twice deep, its loops so each time.
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(looping())
        };
        let again = listened(&sound, &mut hear).unwrap();
        assert_eq!(lengths, [[40 * RATE; 3], [210 * TENTH; 3], [117 * TENTH; 3]].concat());
        assert_eq!(again.len(), 9);
        let mut hear = |_: &[f32]| -> io::Result<Vec<Segment>> { panic!("nothing is heard again") };
        let kept = vec![said(2.0, 9.5)];
        assert_eq!(times(&unlooped(&sound, kept.clone(), &mut hear, AGAIN).unwrap()), times(&kept));
    }

    #[test]
    fn a_turn_beside_a_loop_is_heard_again_as_on_main_once_the_loop_is() {
        // A turn from 11 to 17 s with a stray word in it, then a loop: the loop is heard again
        // from the stray word on; the turn, still unwritten, then as #124 hears it.
        let sound = voiced(40.0, 0.3, &[(2.0, 10.0), (11.0, 17.0), (20.0, 30.0)]);
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(match lengths.len() {
                1 => vec![
                    said(2.0, 9.5),
                    wrote(14.0, 14.0, "Oui."),
                    wrote(20.0, 22.0, "je réponds aux clients"),
                    wrote(22.0, 24.0, "je réponds aux clients"),
                    wrote(24.0, 29.5, "je réponds aux clients"),
                ],
                2 => vec![wrote(6.0, 15.5, "the loop's turn")],
                _ => vec![wrote(0.3, 6.0, "the turn")],
            })
        };
        let again = listened(&sound, &mut hear).unwrap();
        assert_eq!(lengths, [40 * RATE, 260 * TENTH, 66 * TENTH]);
        assert_eq!(
            times(&again),
            [(20, 95, "written"), (110, 167, "the turn"), (200, 295, "the loop's turn")]
        );
    }

    #[test]
    fn a_loop_in_what_a_stretch_heard_again_gives_is_heard_again_in_its_turn() {
        let sound = voiced(40.0, 0.3, &[(2.0, 10.0), (20.0, 30.0)]);
        let mut lengths = Vec::new();
        let mut hear = |stretch: &[f32]| {
            lengths.push(stretch.len());
            Ok(match lengths.len() {
                1 => vec![said(2.0, 9.5)],
                2 => vec![
                    wrote(0.3, 2.0, "on fait le point"),
                    wrote(2.0, 4.0, "on fait le point"),
                    wrote(4.0, 10.0, "on fait le point"),
                ],
                _ => vec![wrote(0.3, 10.0, "the turn")],
            })
        };
        let again = listened(&sound, &mut hear).unwrap();
        assert_eq!(lengths, [40 * RATE, 106 * TENTH, 106 * TENTH]);
        assert_eq!(times(&again), [(20, 95, "written"), (200, 297, "the turn")]);
    }

    #[test]
    fn pieces_of_a_loop_chain_on_each_side_and_its_words_are_those_of_every_segment_in_it() {
        let segments = [
            wrote(1.0, 1.0, "clients"),
            wrote(2.0, 2.0, "je"),
            wrote(3.0, 5.0, "je réponds aux clients"),
            wrote(5.0, 7.0, "je réponds aux clients la"),
            wrote(7.0, 9.0, "réponds aux clients la"),
            wrote(9.0, 11.0, "je réponds aux clients la"),
            wrote(12.0, 12.0, "la"),
            wrote(13.0, 13.0, "réponds"),
            wrote(15.0, 16.0, "Oui."),
        ];
        // "la" is a word of the loop by its later segments only, its first holding none; both
        // pieces before it, from the span's first segment, and both after it are of it.
        assert_eq!(looped(&voiced(20.0, 0.3, &[]), &segments), vec![(0..15 * RATE, 0..8)]);
    }

    #[test]
    fn a_number_alone_heard_where_a_loop_was_is_written() {
        let sound = voiced(40.0, 0.3, &[]);
        let segments = vec![
            wrote(2.0, 4.0, "je réponds aux clients"),
            wrote(4.0, 6.0, "je réponds aux clients"),
            wrote(6.0, 8.0, "je réponds aux clients"),
        ];
        let mut hear = |_: &[f32]| Ok(vec![wrote(1.0, 2.0, "12"), wrote(3.0, 3.5, "…")]);
        let again = unlooped(&sound, segments, &mut hear, 1).unwrap();
        assert_eq!(times(&again), [(10, 20, "12")]);
    }
}
