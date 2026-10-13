//! `summit-summary` — writes the minutes of a session from its transcript, with no window.
//!
//!     summit-summary <session-folder> --provider <address> --model <name>
//!
//! The address speaks the OpenAI protocol: a model on the machine or an online one. A key,
//! when the provider wants one, is read from `SUMMIT_PROVIDER_KEY` and written nowhere.
//!
//! It says what happens as one JSON object per line on its output — `sending`, then
//! `written` — and what failed in one sentence on the error stream. Whatever fails, the
//! transcript is untouched and the minutes that were there are still there.

use std::{fs, io::Write, path::Path, process::ExitCode, time::Duration};

use serde_json::json;
use summit_summary::{Made, Refused, minutes, provider::Provider, read_transcript, request, write_minutes};

fn main() -> ExitCode {
    // Arguments that are not text are refused like any other mistake, not with a crash.
    let args: Vec<String> = std::env::args_os().skip(1).filter_map(|arg| arg.into_string().ok()).collect();
    let all_text = args.len() + 1 == std::env::args_os().count();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        _ if !all_text => Err(Refused("an argument is not text".into())),
        [folder, "--provider", address, "--model", model] | [folder, "--model", model, "--provider", address] => {
            summarise(Path::new(folder), address, model)
        }
        _ => Err(Refused(
            "usage: summit-summary <session-folder> --provider <address> --model <name>".into(),
        )),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(Refused(why)) => {
            eprintln!("summit-summary: {why}");
            ExitCode::FAILURE
        }
    }
}

fn summarise(folder: &Path, address: &str, model: &str) -> Result<(), Refused> {
    let transcript = fs::read_to_string(folder.join("transcript.jsonl"))
        .map_err(|error| Refused(format!("this session has no transcript to read ({error})")))?;
    let segments = read_transcript(&transcript)?;
    // A context that is there and is not text is said: sent without it, the minutes would
    // claim a context they did not have.
    let context = folder.join("context.txt");
    let context = match (fs::symlink_metadata(&context), fs::metadata(&context)) {
        (Err(error), _) if error.kind() == std::io::ErrorKind::NotFound => None,
        (_, Ok(found)) if found.is_file() => match fs::read(&context).map(String::from_utf8) {
            Ok(Ok(text)) => Some(text),
            Ok(Err(_)) => return Err(Refused("context.txt is not text: it must be UTF-8".into())),
            Err(error) => return Err(Refused(format!("context.txt is there and cannot be read ({error})"))),
        },
        _ => {
            return Err(Refused(
                "context.txt is there and cannot be read: it is not a file, or leads nowhere".into(),
            ));
        }
    };
    let asked = request(&segments, context.as_deref());

    let key = match std::env::var("SUMMIT_PROVIDER_KEY") {
        Ok(key) => Some(key),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => return Err(Refused("the key in SUMMIT_PROVIDER_KEY is not text".into())),
    };
    let provider = Provider::new(address, model, key.as_deref(), Duration::from_secs(600))?;
    // Said before anything leaves, and after the address was read: where it goes, and how much.
    let sending = json!({ "event": "sending", "host": provider.host(), "model": provider.model(), "characters": asked.chars().count() });
    if writeln!(std::io::stdout(), "{sending}").is_err() {
        return Err(Refused("nothing was sent: what is sent could not be said first".into()));
    }
    let answer = provider.ask(&asked)?;
    // A provider that sends the key back in its answer does not get it written in a file.
    if key.as_deref().is_some_and(|key| !key.is_empty() && answer.contains(key)) {
        return Err(Refused(format!(
            "the provider at {} sent the key back in its answer: nothing was written",
            provider.host()
        )));
    }

    let at = chrono::Local::now().fixed_offset();
    let written = minutes(
        &answer,
        &Made {
            at,
            provider: &provider.host(),
            model,
            context: context.as_deref(),
        },
    )?;
    write_minutes(folder, &written).map_err(|error| Refused(format!("the minutes could not be written ({error})")))?;
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "written", "file": folder.join("minutes.md") })
    );
    Ok(())
}
