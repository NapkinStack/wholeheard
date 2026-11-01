//! `wholeheard-summary` — writes the minutes of a session from its transcript, with no window.
//!
//!     wholeheard-summary <session-folder> --provider <address> --model <name>
//!     wholeheard-summary --test --provider <address> --model <name>
//!
//! The second asks the provider one sentence of its own, nothing of a meeting, and says
//! whether the model answered: what the settings' *Tester* runs.
//!
//! The address speaks the OpenAI protocol: a model on the machine or an online one. A key,
//! when the provider wants one, is read from `WHOLEHEARD_PROVIDER_KEY` and written nowhere.
//!
//! It says what happens as one JSON object per line on its output — `sending`, then
//! `written` — and what failed in one sentence on the error stream. Whatever fails, the
//! transcript is untouched and the minutes that were there are still there.
//!
//! What it does is logged — never what was said — in the user's state folder (`log`).

use std::{
    fs,
    io::Write,
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};

use serde_json::json;
use tracing::{error, info, warn};
use wholeheard_summary::{Made, Refused, TEST, log, minutes, provider::Provider, read_transcript, reported, request, write_minutes};

fn main() -> ExitCode {
    let _run = log::start("summary");
    // Arguments that are not text are refused like any other mistake, not with a crash.
    let args: Vec<String> = std::env::args_os().skip(1).filter_map(|arg| arg.into_string().ok()).collect();
    let all_text = args.len() + 1 == std::env::args_os().count();
    let outcome = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        _ if !all_text => Err(Refused("an argument is not text".into(), None)),
        ["--test", "--provider", address, "--model", model] | ["--test", "--model", model, "--provider", address] => test(address, model),
        [folder, "--provider", address, "--model", model] | [folder, "--model", model, "--provider", address] => {
            summarise(Path::new(folder), address, model)
        }
        _ => Err(Refused(
            "usage: wholeheard-summary <session-folder> --provider <address> --model <name>, \
             or wholeheard-summary --test --provider <address> --model <name>"
                .into(),
            None,
        )),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(Refused(why, code)) => {
            error!(detail = %why, code, "summary.failed");
            // The cause, when a person acts on it differently: the output's last line (summarising/v1).
            if let Some(code) = code {
                let _ = writeln!(std::io::stdout(), "{}", json!({ "event": "failed", "code": code }));
            }
            eprintln!("wholeheard-summary: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Asks the provider the test's sentence, with a minute's patience: answered, the model is there
/// and the key is taken.
fn test(address: &str, model: &str) -> Result<(), Refused> {
    let key = key()?;
    let provider = Provider::new(address, model, key.as_deref(), Duration::from_secs(60))?;
    info!(host = %provider.host(), model, key = key.is_some(), "summary.testing");
    let sending = json!({ "event": "sending", "host": provider.host(), "model": provider.model(), "characters": TEST.chars().count() });
    if writeln!(std::io::stdout(), "{sending}").is_err() {
        return Err(Refused("nothing was sent: what is sent could not be said first".into(), None));
    }
    let asked = Instant::now();
    provider.ask(TEST)?;
    info!(milliseconds = asked.elapsed().as_millis() as u64, "summary.answered");
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "answered", "host": provider.host(), "model": provider.model() })
    );
    Ok(())
}

/// The key, when the provider wants one: from `WHOLEHEARD_PROVIDER_KEY`, written nowhere.
fn key() -> Result<Option<String>, Refused> {
    match std::env::var("WHOLEHEARD_PROVIDER_KEY") {
        Ok(key) => Ok(Some(key)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(Refused(
            "the key in WHOLEHEARD_PROVIDER_KEY is not text".into(),
            Some("settings.invalid"),
        )),
    }
}

fn summarise(folder: &Path, address: &str, model: &str) -> Result<(), Refused> {
    // The folder's name, never its path: the path says whose meetings and where.
    let named = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    info!(session = %named, "summary.started");
    let transcript = fs::read_to_string(folder.join("transcript.jsonl")).map_err(|error| {
        Refused(
            format!("this session has no transcript to read ({error})"),
            Some("transcript.unusable"),
        )
    })?;
    let segments = read_transcript(&transcript)?;
    // A context that is there and is not text is said: sent without it, the minutes would
    // claim a context they did not have.
    let context = folder.join("context.txt");
    let context = match (fs::symlink_metadata(&context), fs::metadata(&context)) {
        (Err(error), _) if error.kind() == std::io::ErrorKind::NotFound => None,
        (_, Ok(found)) if found.is_file() => match fs::read(&context).map(String::from_utf8) {
            Ok(Ok(text)) => Some(text),
            Ok(Err(_)) => return Err(Refused("context.txt is not text: it must be UTF-8".into(), None)),
            Err(error) => return Err(Refused(format!("context.txt is there and cannot be read ({error})"), None)),
        },
        _ => {
            return Err(Refused(
                "context.txt is there and cannot be read: it is not a file, or leads nowhere".into(),
                None,
            ));
        }
    };
    let asked = request(&segments, context.as_deref());
    info!(segments = segments.len(), context = context.is_some(), "summary.read");

    let key = key()?;
    let provider = Provider::new(address, model, key.as_deref(), Duration::from_secs(600))?;
    // Said before anything leaves, and after the address was read: where it goes, and how much.
    let sending = json!({ "event": "sending", "host": provider.host(), "model": provider.model(), "characters": asked.chars().count() });
    if writeln!(std::io::stdout(), "{sending}").is_err() {
        return Err(Refused("nothing was sent: what is sent could not be said first".into(), None));
    }
    info!(host = %provider.host(), model, key = key.is_some(), characters = asked.chars().count(), "summary.sending");
    // An answer that is not minutes is asked for once more — a model's form varies from one
    // answer to the next (#366); any other failure is not.
    let mut again = true;
    let (written, cost) = loop {
        let sent = Instant::now();
        let answered = provider.ask(&asked)?;
        let answer = answered.text;
        info!(
            milliseconds = sent.elapsed().as_millis() as u64,
            characters = answer.chars().count(),
            "summary.answered"
        );
        // A provider that sends the key back in its answer does not get it written in a file.
        if key.as_deref().is_some_and(|key| !key.is_empty() && answer.contains(key)) {
            return Err(Refused(
                format!("{} sent the key back in its answer: nothing was written", provider.named()),
                None,
            ));
        }
        let cost = reported(&provider.host(), &answered.usage);
        let at = chrono::Local::now().fixed_offset();
        let made = Made {
            at,
            provider: &provider.host(),
            model,
            context: context.as_deref(),
            cost: cost.clone().ok().flatten(),
            segments: &segments,
        };
        match minutes(&answer, &made) {
            Err(Refused(why, Some("answer.not_minutes"))) if std::mem::take(&mut again) => {
                info!(detail = %why, "summary.asked_again");
            }
            done => break (done?, cost),
        }
    };
    // Nor when the title, its marks read, spells it (D48): what is judged is what is written.
    if key.as_deref().is_some_and(|key| !key.is_empty() && written.as_str().contains(key)) {
        return Err(Refused(
            format!("{} sent the key back in its answer: nothing was written", provider.named()),
            None,
        ));
    }
    // Of the answer written alone, and counted: never what it held.
    if written.removed() > 0 {
        warn!(removed = written.removed(), "summary.times_removed");
    }
    if let Err(why) = cost {
        warn!(detail = why, "summary.cost_refused");
    }
    write_minutes(folder, &written)
        .map_err(|error| Refused(format!("the minutes could not be written ({error})"), Some("minutes.unwritable")))?;
    info!("summary.written");
    let _ = writeln!(
        std::io::stdout(),
        "{}",
        json!({ "event": "written", "file": folder.join("minutes.md") })
    );
    Ok(())
}
