//! Summit became Wholeheard (D28, PDR-0001): what a user held under the old name is carried over
//! at the start, before anything is read — never asked for again, never lost.
//!
//! Each step looks at what is there and does what is left of it, so that one interrupted — a
//! folder that could not be moved, a crash — is done at the next start, and a start with nothing
//! left to do does nothing:
//!
//! 1. The meetings stay where Summit kept them: when the settings name no folder and Summit's
//!    holds meetings, it is written in them — first, so that it is never lost to a step after.
//! 2. The settings' folder and the data's (the Whisper model, 874 MB) are moved, not copied: the
//!    model is never fetched again. A folder already there under the new name and holding
//!    something is Wholeheard's: nothing is overwritten, the old one is left as it is.
//! 3. A model the settings name inside Summit's data folder is named where it was moved to.
//!
//! The settings are edited where a key is, the rest of the file — comments, order — as it was,
//! and written whole or not at all. The keys in the keyring are carried over as they are read
//! (`keys`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, value};

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

/// What was carried over at this start, to be said once.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Carried {
    pub settings: bool,
    pub data: bool,
    pub meetings_kept: Option<PathBuf>,
    pub model_named: bool,
}

/// Carries what is left of Summit's over to Wholeheard's; nothing left, nothing done.
pub fn carry_over(places: &Places) -> io::Result<Carried> {
    let mut carried = Carried::default();
    let (settings_before, settings_now) = (places.config.join(BEFORE), places.config.join(NOW));
    let (data_before, data_now) = (places.data.join(BEFORE), places.data.join(NOW));
    // The file the settings are in now — Summit's while its folder is not moved yet.
    let file = if holds_something(&settings_now)? || !settings_before.is_dir() {
        settings_now.join("settings.toml")
    } else {
        settings_before.join("settings.toml")
    };

    // 1. The meetings: Summit's folder named, when the settings name none and it holds meetings —
    // while something of Summit's is still to carry over, or no settings are written yet: a user
    // who takes the folder out of their settings afterwards is not overruled.
    let meetings = places.documents.join(MEETINGS_BEFORE);
    let still = settings_before.is_dir() || data_before.is_dir() || !settings_now.join("settings.toml").exists();
    if still && holds_meetings(&meetings)? {
        let mut settings = read(&file)?;
        if let Some(settings) = settings.as_mut().filter(|settings| !settings.contains_key("sessions")) {
            settings["sessions"] = value(meetings.to_string_lossy().into_owned());
            write(&file, settings)?;
            carried.meetings_kept = Some(meetings);
        }
    }

    // 2. The folders, moved.
    carried.settings = moved(&settings_before, &settings_now)?;
    carried.data = moved(&data_before, &data_now)?;

    // 3. A model named inside Summit's data folder, named where it is now.
    let file = settings_now.join("settings.toml");
    if let Some(mut settings) = read(&file)?
        && let Some(written) = settings.get("model").and_then(|model| model.as_str())
        && let Ok(rest) = crate::settings::from_home(PathBuf::from(written)).strip_prefix(&data_before)
        && !data_before.exists()
    {
        settings["model"] = value(data_now.join(rest).to_string_lossy().into_owned());
        write(&file, &settings)?;
        carried.model_named = true;
    }
    Ok(carried)
}

/// Moves a folder to its new name, unless something of the new one is there already.
fn moved(before: &Path, now: &Path) -> io::Result<bool> {
    if !before.is_dir() || holds_something(now)? {
        return Ok(false);
    }
    // An empty folder under the new name holds nothing of Wholeheard's: it gives way.
    if now.is_dir() {
        fs::remove_dir(now)?;
    }
    if let Some(parent) = now.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(before, now)?;
    Ok(true)
}

/// Whether a folder is there and holds anything.
fn holds_something(folder: &Path) -> io::Result<bool> {
    match fs::read_dir(folder) {
        Ok(mut entries) => Ok(entries.next().is_some()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Whether a folder holds meetings — a folder of its own with its `session.json` — and is not
/// merely a folder of that name.
fn holds_meetings(folder: &Path) -> io::Result<bool> {
    let Ok(entries) = fs::read_dir(folder) else { return Ok(false) };
    for entry in entries {
        if entry?.path().join("session.json").is_file() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The settings as written — `Some` empty when there is no file yet; `None` when the file cannot
/// be read: left as it is, the settings say why.
fn read(file: &Path) -> io::Result<Option<DocumentMut>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text.parse::<DocumentMut>().ok()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Some(DocumentMut::new())),
        Err(error) => Err(error),
    }
}

/// Writes the settings whole or not at all: beside, then in place — through a link to where it
/// leads (a file kept in the user's own dotfiles stays there), with the permissions it had.
fn write(file: &Path, settings: &DocumentMut) -> io::Result<()> {
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder)?;
    }
    let file = fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let mut text = settings.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let beside = file.with_extension(format!("toml.carrying.{}", std::process::id()));
    fs::write(&beside, text)?;
    if let Ok(had) = fs::metadata(&file) {
        fs::set_permissions(&beside, had.permissions())?;
    }
    fs::rename(&beside, &file)
}
