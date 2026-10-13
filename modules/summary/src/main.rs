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

use std::{fs, path::Path, process::ExitCode, time::Duration};

use serde_json::json;
use summit_summary::{Made, Refused, minutes, provider::Provider, read_transcript, request, write_minutes};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
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
    let context = match fs::read(folder.join("context.txt")) {
        Ok(bytes) => Some(String::from_utf8(bytes).map_err(|_| Refused("context.txt is not text: it must be UTF-8".into()))?),
        Err(_) => None,
    };
    let asked = request(&segments, context.as_deref());

    let key = std::env::var("SUMMIT_PROVIDER_KEY").ok();
    let provider = Provider::new(address, model, key.as_deref(), Duration::from_secs(600))?;
    // Said before anything leaves, and after the address was read: where it goes, and how much.
    println!(
        "{}",
        json!({ "event": "sending", "host": provider.host(), "model": provider.model(), "characters": asked.chars().count() })
    );
    let answer = provider.ask(&asked)?;

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
    println!("{}", json!({ "event": "written", "file": folder.join("minutes.md") }));
    Ok(())
}
