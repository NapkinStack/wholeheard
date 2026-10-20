//! Summit became Wholeheard (D28, PDR-0001): what a user held under the old name is carried over
//! once, at the first start, before anything is read — never asked for again, never lost.
//!
//! - The settings' folder and the data's (the Whisper model, 874 MB) are moved, not copied: the
//!   model is never fetched again.
//! - The meetings stay where they are: when the settings named no folder, the one Summit used is
//!   written in them, so that Wholeheard keeps reading it.
//! - A model the settings named inside Summit's data folder is named where it was moved to.
//!
//! Nothing is overwritten: a folder already there under the new name is Wholeheard's, and the old
//! one is left as it is. The keys in the keyring are carried over as they are read (`keys`).

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The name of the folders before D28.
pub const BEFORE: &str = "summit";
/// The name of the folders now.
pub const NOW: &str = "wholeheard";
/// The meetings' folder Summit made in the documents when the settings named none.
pub const MEETINGS_BEFORE: &str = "Summit";

/// Where a user's things are on this system: its configuration, its data, its documents.
pub struct Places {
    pub config: PathBuf,
    pub data: PathBuf,
    pub documents: PathBuf,
}

impl Places {
    /// This system's, as the settings read them.
    pub fn here() -> Option<Self> {
        Some(Self {
            config: dirs::config_dir()?,
            data: dirs::data_dir()?,
            documents: dirs::document_dir().or_else(dirs::home_dir)?,
        })
    }
}

/// What was carried over, to be said once.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Carried {
    pub settings: bool,
    pub data: bool,
    pub meetings_kept: Option<PathBuf>,
}

/// Carries Summit's folders over to Wholeheard's, once; nothing to carry, nothing done.
pub fn carry_over(places: &Places) -> io::Result<Carried> {
    let mut carried = Carried::default();
    let settings_before = places.config.join(BEFORE);
    let settings_now = places.config.join(NOW);
    if !settings_now.exists() && settings_before.is_dir() {
        fs::rename(&settings_before, &settings_now)?;
        carried.settings = true;
    }
    let data_before = places.data.join(BEFORE);
    let data_now = places.data.join(NOW);
    if !data_now.exists() && data_before.is_dir() {
        fs::rename(&data_before, &data_now)?;
        carried.data = true;
    }
    let file = settings_now.join("settings.toml");
    // A model named inside Summit's data folder: named where it is now.
    if carried.data && file.is_file() {
        let text = fs::read_to_string(&file)?;
        let (before, now) = (data_before.to_string_lossy(), data_now.to_string_lossy());
        if text.contains(&*before) {
            fs::write(&file, text.replace(&*before, &now))?;
        }
    }
    // The meetings stay where Summit kept them: named, when the settings named no folder.
    let meetings = places.documents.join(MEETINGS_BEFORE);
    if (carried.settings || !file.exists()) && meetings.is_dir() && !names_sessions(&file)? {
        fs::create_dir_all(&settings_now)?;
        let mut written = fs::OpenOptions::new().create(true).append(true).open(&file)?;
        let line = toml::Value::String(meetings.to_string_lossy().into_owned());
        writeln!(written, "sessions = {line}")?;
        carried.meetings_kept = Some(meetings);
    }
    Ok(carried)
}

/// Whether the settings' file names a folder for the meetings.
fn names_sessions(file: &Path) -> io::Result<bool> {
    if !file.exists() {
        return Ok(false);
    }
    let text = fs::read_to_string(file)?;
    Ok(text.parse::<toml::Table>().map_or_else(
        // A file that cannot be read is left as it is: the settings say why.
        |_| true,
        |table| table.contains_key("sessions"),
    ))
}
