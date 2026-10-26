//! Apple's SpeechAnalyzer on this Mac (D42, ADR-0012): what `wholeheard-speech` — transcription's
//! program, beside the shell's in the app — says it offers, and Apple's model of a language fetched
//! by macOS on the user's word. Where the program is not, nothing is offered.

use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// The languages the shell speaks, each its locale.
const LOCALES: [(&str, &str); 2] = [("fr", "fr-FR"), ("en", "en-US")];

/// What this Mac offers for a language: SpeechTranscriber there and hearing it, and whether
/// Apple's model of it is already on the Mac.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Status {
    pub offered: bool,
    pub installed: bool,
}

fn locale(language: &str) -> Option<&'static str> {
    LOCALES.iter().find(|(code, _)| *code == language).map(|(_, locale)| *locale)
}

/// What the program says of `language`, in five seconds at most — or nothing: no program, a
/// language it has no locale for, an answer it did not give.
pub fn status(program: &Path, language: &str) -> Option<Status> {
    let said = run(program, &["status", locale(language)?], Duration::from_secs(5)).ok()?;
    let value: serde_json::Value = serde_json::from_str(said.lines().next()?).ok()?;
    let yes = |key: &str| value[key].as_bool() == Some(true);
    Some(Status {
        offered: yes("available") && yes("supported"),
        installed: yes("installed"),
    })
}

/// Apple's model of `language`, fetched and installed by macOS: run only on the user's click.
pub fn install(program: &Path, language: &str) -> Result<(), String> {
    let locale = locale(language).ok_or("SpeechAnalyzer hears French or English")?;
    run(program, &["install", locale], Duration::from_secs(1800)).map(|_| ())
}

/// The program run with `args`, its output once it ended well — or its own sentence.
fn run(program: &Path, args: &[&str], within: Duration) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{} could not be started ({error})", program.display()))?;
    let given = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if given.elapsed() < within => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("wholeheard-speech did not answer in time".into());
            }
        }
    };
    let read = |stream: Option<&mut dyn std::io::Read>| {
        let mut text = String::new();
        if let Some(stream) = stream {
            let _ = stream.read_to_string(&mut text);
        }
        text
    };
    let out = read(child.stdout.as_mut().map(|out| out as &mut dyn std::io::Read));
    if status.success() {
        return Ok(out);
    }
    let err = read(child.stderr.as_mut().map(|err| err as &mut dyn std::io::Read));
    let said = err.lines().next().unwrap_or("it ended without saying why");
    Err(said.trim_start_matches("wholeheard-speech: ").to_string())
}
