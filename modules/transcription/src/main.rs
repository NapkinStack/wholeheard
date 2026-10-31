//! `wholeheard-transcription` — writes the transcript of a session from its two audio files,
//! with an engine on this machine and no window.
//!
//!     wholeheard-transcription <session-folder> --model <model-file> [--language <code>]
//!     wholeheard-transcription <session-folder> --engine speechanalyzer --language <fr|en>
//!
//! The model is a Whisper model in whisper.cpp's format, already on the machine: nothing is
//! fetched. On a Mac with macOS 26, Apple's SpeechAnalyzer hears instead, through
//! `wholeheard-speech` beside this program (D42, ADR-0012), its model already on the Mac. Stopped at any moment, the program is run again and goes on from where it was.
//!
//! It says what happens as one JSON object per line on its output — `hearing`, then
//! `written` — and what failed in one sentence on the error stream. `hearing` says whether the
//! model times each word, which puts a turn where it was said, or each segment only.
//!
//! What it does is logged — never what was said — in the user's state folder (`log`).

use std::{io::Write, path::Path, process::ExitCode, time::Instant};

use serde_json::json;
use tracing::{error, info};
use wholeheard_transcription::{
    log,
    online::{Refused, Service},
    speech::SpeechAnalyzer,
    step::{Engine, hear, read},
    whisper::{Whisper, knows},
};

/// Who hears: a Whisper model, Apple's SpeechAnalyzer, or a service the user named (D43).
enum Hearer<'a> {
    Model(&'a Path),
    SpeechAnalyzer,
    Service { provider: &'a str, model: &'a str },
}

/// The key the service wants, when it wants one: handed by the shell, written nowhere (ADR-0013).
fn key() -> Option<String> {
    std::env::var("WHOLEHEARD_PROVIDER_KEY").ok().filter(|key| !key.is_empty())
}

/// Seconds of sound an engine hears at a time. Long, on purpose: on the sample the engine was
/// chosen on, 34.5 % of the words differed when it heard thirty seconds at a time, 28 % at one
/// or two minutes, 26.7 % when it heard the five minutes whole (cycle 01, D4). A step that is
/// killed loses at most the span it was hearing.
const SPAN: usize = 300;

fn main() -> ExitCode {
    let _run = log::start("transcription");
    let args: Vec<String> = std::env::args_os().skip(1).filter_map(|arg| arg.into_string().ok()).collect();
    let all_text = args.len() + 1 == std::env::args_os().count();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        _ if !all_text => Err("an argument is not text".to_string()),
        [folder, "--model", model] => run(Path::new(folder), Hearer::Model(Path::new(model)), None),
        [folder, "--model", model, "--language", language] | [folder, "--language", language, "--model", model] => {
            run(Path::new(folder), Hearer::Model(Path::new(model)), Some(language))
        }
        [folder, "--engine", "speechanalyzer", "--language", language] | [folder, "--language", language, "--engine", "speechanalyzer"] => {
            run(Path::new(folder), Hearer::SpeechAnalyzer, Some(language))
        }
        ["--test", "--provider", provider, "--model", model] => test(provider, model),
        [folder, "--provider", provider, "--model", model] => run(Path::new(folder), Hearer::Service { provider, model }, None),
        [folder, "--provider", provider, "--model", model, "--language", language] => {
            run(Path::new(folder), Hearer::Service { provider, model }, Some(language))
        }
        _ => Err(
            "usage: wholeheard-transcription <session-folder> --model <model-file> [--language <code>] \
             | --engine speechanalyzer --language <fr|en> \
             | --provider <address> --model <name> [--language <code>] | --test --provider <address> --model <name>"
                .to_string(),
        ),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            error!(detail = %why, "transcription.failed");
            eprintln!("wholeheard-transcription: {why}");
            ExitCode::FAILURE
        }
    }
}

/// A test of a service: what failed, for the log, and its cause on the output's last line (`transcript/v1`).
fn test(provider: &str, model: &str) -> Result<(), String> {
    let refused = match Service::new(provider, model, key().as_deref()) {
        Ok(service) => service.tested().err(),
        Err(why) => Some(Refused {
            code: Some("settings.invalid"),
            why,
        }),
    };
    let Some(refused) = refused else { return Ok(()) };
    if let Some(code) = refused.code {
        // An output that closed takes no line: the sentence is still said.
        let _ = writeln!(std::io::stdout(), "{}", serde_json::json!({ "event": "failed", "code": code }));
    }
    Err(refused.why)
}

fn run(folder: &Path, hearer: Hearer, language: Option<&str>) -> Result<(), String> {
    let began = Instant::now();
    // The folder's name, never its path: the path says whose meetings and where.
    let named = |path: &Path| path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let asked = match hearer {
        Hearer::Model(model) => named(model),
        Hearer::SpeechAnalyzer => "SpeechAnalyzer".into(),
        Hearer::Service { model, .. } => model.to_string(),
    };
    info!(session = %named(folder), model = %asked, language, "transcription.started");
    if !folder.join("session.json").is_file() {
        return Err("this folder is not a session: it holds no session.json".into());
    }
    if matches!(hearer, Hearer::Model(_)) && language.is_some_and(|code| !knows(code)) {
        return Err(
            "the language is a code the engine knows, in lower-case letters, as in fr or en; leave it out to have it detected".into(),
        );
    }
    // The session is judged before the model is loaded: nothing is said to be heard that is refused.
    let (form, session) = read(folder).map_err(|error| error.to_string())?;
    let loading = Instant::now();
    let (mut engine, times): (Box<dyn Engine>, &str) = match hearer {
        Hearer::Model(model) => {
            let engine = Whisper::load(model).map_err(|error| error.to_string())?;
            // A model whose alignment heads are not known has its segments where the engine put them.
            let times = if engine.times_words() { "words" } else { "segments" };
            (Box::new(engine), times)
        }
        Hearer::SpeechAnalyzer => {
            let language = language.unwrap_or_default();
            {
                let engine = SpeechAnalyzer::at(&SpeechAnalyzer::program(), language)?
                    .spans_in(&folder.join("derived"))
                    .map_err(|error| format!("the session's derived folder cannot be made ({error})"))?;
                (Box::new(engine), "segments")
            }
        }
        // Sent nowhere before the shell asked the user to agree to the host, as for the minutes.
        Hearer::Service { provider, model } => (Box::new(Service::new(provider, model, key().as_deref())?), "segments"),
    };
    let (name, model) = engine.name();
    info!(engine = %name, model = %model, times, audio = form, milliseconds = loading.elapsed().as_millis() as u64, "transcription.loaded");
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "hearing", "engine": name, "model": model, "language": language, "audio": form, "times": times })
    );
    hear(folder, session, engine.as_mut(), language, SPAN).map_err(|error| error.to_string())?;
    info!(milliseconds = began.elapsed().as_millis() as u64, "transcription.written");
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "written", "file": folder.join("transcript.jsonl") })
    );
    Ok(())
}
