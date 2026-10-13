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
}

impl Default for Settings {
    fn default() -> Self {
        let documents = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        Self {
            reminder: true,
            sessions: documents.join("Summit"),
        }
    }
}

/// Where the settings are on this system: `summit/settings.toml` in the user's configuration.
pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|folder| folder.join("summit").join("settings.toml"))
}

/// The file as it is written: what it names, and where it names the folder.
#[derive(Deserialize)]
struct Written {
    reminder: Option<bool>,
    sessions: Option<toml::Spanned<PathBuf>>,
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
    let mut settings = Settings::default();
    settings.reminder = written.reminder.unwrap_or(settings.reminder);
    if let Some(sessions) = written.sessions {
        let at = line(sessions.span().start);
        // A folder written as people write it: from their home, with a tilde.
        let folder = match (sessions.get_ref().strip_prefix("~"), dirs::home_dir()) {
            (Ok(rest), Some(home)) => home.join(rest),
            _ => sessions.into_inner(),
        };
        if !folder.is_absolute() {
            return Err(format!(
                "{}, line {at}: sessions is the folder's whole path, from the root or from ~",
                path.display()
            ));
        }
        settings.sessions = folder;
    }
    Ok(settings)
}
