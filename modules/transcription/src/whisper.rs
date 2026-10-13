//! The engine every system can run (ADR-0003): Whisper, through whisper.cpp, linked into
//! this program. The model is a file the user has; nothing is fetched here.

use std::{io, path::Path};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{Segment, step::Engine};

pub struct Whisper {
    engine: WhisperContext,
    model: String,
    threads: i32,
}

fn failed<T>(what: &str, error: impl std::fmt::Display) -> io::Result<T> {
    Err(io::Error::other(format!("{what} ({error})")))
}

/// Whether Whisper knows a language by this code — `fr`, `en`. Its own word for "detect it"
/// is not one: leaving the language out says that.
pub fn knows(language: &str) -> bool {
    // whisper.cpp says what it does not know on the error stream: kept quiet, as in `load`.
    whisper_rs::install_logging_hooks();
    language != "auto" && language.chars().all(|c| c.is_ascii_lowercase()) && whisper_rs::get_lang_id(language).is_some()
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
        let engine = match WhisperContext::new_with_params(path, WhisperContextParameters::default()) {
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
        })
    }
}

impl Engine for Whisper {
    fn name(&self) -> (String, String) {
        ("whisper.cpp".into(), self.model.clone())
    }

    fn hear(&mut self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> io::Result<Vec<Segment>> {
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
        if let Some(context) = context {
            params.set_initial_prompt(context);
        }
        state
            .full(params, sound)
            .or_else(|error| failed("the engine could not hear this span", error))?;
        let mut segments = Vec::new();
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
