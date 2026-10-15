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
    // Tenths at the speaker's level and not written, one stretch across rests of up to three tenths.
    let mut stretches: Vec<Range<usize>> = Vec::new();
    for (at, loud) in level.iter().enumerate() {
        if written[at] || *loud < own - UNDER {
            continue;
        }
        match stretches.last_mut() {
            Some(last) if at <= last.end + 3 => last.end = at + 1,
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
        if !model.is_file() || std::fs::File::open(model).is_err() {
            return failed("the model is not a file that can be read", model.display());
        }
        // whisper.cpp writes its own account of the model on the error stream: kept quiet,
        // so that what this program says there is one sentence.
        whisper_rs::install_logging_hooks();
        let Some(path) = model.to_str() else {
            return failed("the model's path is not text", model.display());
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
        let mut segments = self.heard(sound, language, context)?;
        for stretch in unwritten(sound, &segments) {
            let offset = stretch.start as f64 / RATE as f64;
            for segment in self.heard(&sound[stretch], language, context)? {
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

    fn said(start: f64, end: f64) -> Segment {
        Segment {
            start,
            end,
            text: "written".into(),
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
        // The speaker at 0.3, a television at 0.06 (14 dB under), never written.
        let mut sound = voiced(60.0, 0.3, &[(2.0, 30.0)]);
        for (at, tv) in voiced(60.0, 0.06, &[(31.0, 59.0)]).into_iter().enumerate() {
            sound[at] += tv;
        }
        assert!(unwritten(&sound, &[said(2.0, 29.5)]).is_empty());
        // At 10 dB under, it is taken for the speaker.
        let mut sound = voiced(60.0, 0.3, &[(2.0, 30.0)]);
        for (at, near) in voiced(60.0, 0.095, &[(31.0, 59.0)]).into_iter().enumerate() {
            sound[at] += near;
        }
        assert_eq!(unwritten(&sound, &[said(2.0, 29.5)]).len(), 1);
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
}
