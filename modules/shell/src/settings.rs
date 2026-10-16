//! The settings, as one file the user can read and write without Summit: a few lines of TOML
//! in the system's place for an application's configuration. A file that is not there is
//! every default; a key this version does not know is left alone, for the version that does.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// What the user chose. Each field has its default: a file may name one and leave the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Tell, at each start, that announcing the recording is the user's to do.
    pub reminder: bool,
    /// The folder the sessions are written in, one folder each.
    pub sessions: PathBuf,
    /// Make the transcript of a recording once it is stopped, without another gesture.
    pub transcript: bool,
    /// The Whisper model the transcript is heard with: a file in whisper.cpp's format.
    pub model: PathBuf,
}

/// The model a transcript is heard with when the settings name none (ADR-0003).
pub const MODEL: &str = "ggml-large-v3-turbo-q8_0.bin";

impl Default for Settings {
    fn default() -> Self {
        let documents = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        let data = dirs::data_dir().unwrap_or_default();
        Self {
            reminder: true,
            sessions: documents.join("Summit"),
            transcript: true,
            model: data.join("summit").join("models").join(MODEL),
        }
    }
}

/// Where the settings are on this system: `summit/settings.toml` in the user's configuration.
pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|folder| folder.join("summit").join("settings.toml"))
}

/// The file as it is written: what it names, and where it names each path.
#[derive(Deserialize)]
struct Written {
    reminder: Option<bool>,
    sessions: Option<toml::Spanned<PathBuf>>,
    transcript: Option<bool>,
    model: Option<toml::Spanned<PathBuf>>,
}

/// Reads the settings. No file is no choice made: the defaults. A file that cannot be read as
/// settings is said in one sentence — which file, which line, and what in it — and nothing of
/// it is used.
pub fn read(path: &Path) -> Result<Settings, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => return Err(format!("{} cannot be read ({error})", path.display())),
    };
    // The line a place of the file is on: one more than the line ends before it.
    let line = |at: usize| text[..at.min(text.len())].matches('\n').count() + 1;
    let written: Written = toml::from_str(&text).map_err(|error| match error.span() {
        Some(span) => format!("{}, line {}: {}", path.display(), line(span.start), error.message()),
        None => format!("{}: {}", path.display(), error.message()),
    })?;
    // A path written as people write it: from their home, with a tilde — or from the root.
    let whole = |key: &str, what: &str, written: toml::Spanned<PathBuf>| {
        let at = line(written.span().start);
        let named = match (written.get_ref().strip_prefix("~"), dirs::home_dir()) {
            (Ok(rest), Some(home)) => home.join(rest),
            _ => written.into_inner(),
        };
        if named.is_absolute() {
            Ok(named)
        } else {
            Err(format!(
                "{}, line {at}: {key} is {what} whole path, from the root or from ~",
                path.display()
            ))
        }
    };
    let mut settings = Settings::default();
    settings.reminder = written.reminder.unwrap_or(settings.reminder);
    settings.transcript = written.transcript.unwrap_or(settings.transcript);
    if let Some(sessions) = written.sessions {
        settings.sessions = whole("sessions", "the folder's", sessions)?;
    }
    if let Some(model) = written.model {
        settings.model = whole("model", "the file's", model)?;
    }
    Ok(settings)
}
