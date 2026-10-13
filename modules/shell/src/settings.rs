//! The settings, as one file the user can read and write without Summit: a few lines of TOML
//! in the system's place for an application's configuration. A file that is not there is
//! every default; a key this version does not know is left alone, for the version that does.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// What the user chose. Each field has its default: a file may name one and leave the rest.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
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

/// Reads the settings. No file is no choice made: the defaults. A file that cannot be read as
/// settings is said in one sentence — which file, and what in it — and nothing of it is used.
pub fn read(path: &Path) -> Result<Settings, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => return Err(format!("{} cannot be read ({error})", path.display())),
    };
    let mut settings: Settings = toml::from_str(&text).map_err(|error| {
        let line = error.span().map_or(0, |span| text[..span.start].lines().count().max(1));
        format!("{}, line {line}: {}", path.display(), error.message())
    })?;
    // A folder written as people write it: from their home, with a tilde.
    if let (Ok(rest), Some(home)) = (settings.sessions.strip_prefix("~"), dirs::home_dir()) {
        settings.sessions = home.join(rest);
    }
    if !settings.sessions.is_absolute() {
        return Err(format!(
            "{}: sessions is the folder's whole path, from the root or from ~",
            path.display()
        ));
    }
    Ok(settings)
}
