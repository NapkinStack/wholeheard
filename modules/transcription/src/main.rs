//! `summit-transcription` — writes the transcript of a session from its two audio files,
//! with an engine on this machine and no window.
//!
//!     summit-transcription <session-folder> --model <model-file> [--language <code>]
//!
//! The model is a Whisper model in whisper.cpp's format, already on the machine: nothing is
//! fetched. Stopped at any moment, the program is run again and goes on from where it was.
//!
//! It says what happens as one JSON object per line on its output — `hearing`, then
//! `written` — and what failed in one sentence on the error stream.

use std::{io::Write, path::Path, process::ExitCode};

use serde_json::json;
use summit_transcription::{
    step::{hear, read},
    whisper::{Whisper, knows},
};

/// Seconds of sound an engine hears at a time. Long, on purpose: on the sample the engine was
/// chosen on, 34.5 % of the words differed when it heard thirty seconds at a time, 28 % at one
/// or two minutes, 26.7 % when it heard the five minutes whole (cycle 01, D4). A step that is
/// killed loses at most the span it was hearing.
const SPAN: usize = 300;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os().skip(1).filter_map(|arg| arg.into_string().ok()).collect();
    let all_text = args.len() + 1 == std::env::args_os().count();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        _ if !all_text => Err("an argument is not text".to_string()),
        [folder, "--model", model] => run(Path::new(folder), Path::new(model), None),
        [folder, "--model", model, "--language", language] | [folder, "--language", language, "--model", model] => {
            run(Path::new(folder), Path::new(model), Some(language))
        }
        _ => Err("usage: summit-transcription <session-folder> --model <model-file> [--language <code>]".to_string()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("summit-transcription: {why}");
            ExitCode::FAILURE
        }
    }
}

fn run(folder: &Path, model: &Path, language: Option<&str>) -> Result<(), String> {
    if !folder.join("session.json").is_file() {
        return Err("this folder is not a session: it holds no session.json".into());
    }
    if language.is_some_and(|code| !knows(code)) {
        return Err(
            "the language is a code the engine knows, in lower-case letters, as in fr or en; leave it out to have it detected".into(),
        );
    }
    // The session is judged before the model is loaded: nothing is said to be heard that is refused.
    let (form, session) = read(folder).map_err(|error| error.to_string())?;
    let mut engine = Whisper::load(model).map_err(|error| error.to_string())?;
    let (name, model) = summit_transcription::step::Engine::name(&engine);
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "hearing", "engine": name, "model": model, "language": language, "audio": form })
    );
    hear(folder, session, &mut engine, language, SPAN).map_err(|error| error.to_string())?;
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "written", "file": folder.join("transcript.jsonl") })
    );
    Ok(())
}
