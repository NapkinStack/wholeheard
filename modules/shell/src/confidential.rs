//! A meeting said confidential (D60): `confidential` of `contracts/session-audio/v2` (and v1), an
//! empty file the shell writes on the user's word and alone reads. Present, the meeting is heard
//! and its minutes written on this computer alone, whatever the settings name — the shell chooses
//! here, before it starts either step; and what had already left before it was said is told.

use std::{fs, io, path::Path};

use serde::Serialize;

use crate::{
    meeting::{Read, Transcript},
    minutes::Minutes,
    providers,
    settings::{Engine, Settings},
    transcription::Hearer,
};

/// The file's name, in the meeting's folder.
pub const FILE: &str = "confidential";

/// Whether the meeting is said confidential: the file is there — whatever it holds, the user's
/// word all the same.
pub fn marked(folder: &Path) -> bool {
    folder.join(FILE).is_file()
}

/// Said confidential (`on`): the empty file written whole — a file beside it, renamed over it —;
/// said not to be: removed. Refused before the folder holds `session.json` (`NotFound`).
pub fn mark(folder: &Path, on: bool) -> io::Result<()> {
    if !folder.join("session.json").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the meeting's folder holds no session.json yet",
        ));
    }
    let file = folder.join(FILE);
    if !on {
        return match fs::remove_file(&file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let beside = folder.join(".confidential.part");
    let written = fs::write(&beside, b"").and_then(|()| fs::rename(&beside, &file));
    if written.is_err() {
        let _ = fs::remove_file(&beside);
    }
    written
}

/// Who hears a confidential meeting: Apple's SpeechAnalyzer where the settings chose it already
/// and this Mac offers it (`apple_ready`, asked only then), else Whisper with the model the
/// settings name — never a service, even one of this computer.
pub fn hearer(settings: &Settings, language: &str, apple_ready: impl FnOnce() -> bool) -> Hearer {
    let engine = match settings.engine {
        Engine::SpeechAnalyzer => Engine::SpeechAnalyzer,
        _ => Engine::Whisper,
    };
    Hearer::chosen(engine, &settings.model, language, apple_ready)
}

/// Why a confidential meeting's minutes wait: Ollama does not answer, or holds no model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Wait {
    Silent,
    Empty,
}

/// Who writes a confidential meeting's minutes, as `(provider, model)`: the one `named` — the
/// settings', or a Refaire…'s — when it is this computer's; else Ollama at `base`, with the
/// recommended model when it holds it, else the first it holds (`held`, asked only then).
pub fn writer(named: Option<(String, String)>, base: &str, held: impl FnOnce() -> Option<Vec<String>>) -> Result<(String, String), Wait> {
    if let Some((provider, model)) = named
        && providers::local(&provider)
        && !model.trim().is_empty()
    {
        return Ok((provider, model));
    }
    let held = held().ok_or(Wait::Silent)?;
    let model = held
        .iter()
        .find(|model| *model == providers::RECOMMENDED)
        .or(held.first())
        .ok_or(Wait::Empty)?;
    Ok((providers::ollama_address(base), model.clone()))
}

/// What had left the computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum What {
    /// The meeting's sound, to a service that heard it.
    Sound,
    /// Its transcript, to a service that wrote the minutes.
    Transcript,
}

/// What had left, and to which host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Left {
    pub what: What,
    pub host: String,
}

/// Adds what left to what is said, once each.
pub fn add(left: &mut Vec<Left>, what: What, host: &str) {
    if !left.iter().any(|said| said.what == what && said.host == host) {
        left.push(Left { what, host: host.into() });
    }
}

/// What the meeting's files say had left: a transcript whose header names a service's host as
/// its engine — this computer's engines name themselves —, minutes whose front matter names a
/// host and not `local`.
pub fn left(transcript: &Read<Transcript>, minutes: &Read<Minutes>) -> Vec<Left> {
    let mut left = Vec::new();
    if let Read::Present(transcript) = transcript
        && let Some(engine) = transcript.engine.as_deref().filter(|engine| !engine.trim().is_empty())
        && !matches!(engine, "whisper.cpp" | "SpeechAnalyzer")
    {
        add(&mut left, What::Sound, engine);
    }
    if let Read::Present(minutes) = minutes
        && let Some(provider) = minutes.provider.as_deref().filter(|provider| !provider.trim().is_empty())
        && provider != "local"
    {
        add(&mut left, What::Transcript, provider);
    }
    left
}
