//! Wholeheard started with the user's session (D32), as the desktop's convention has it: an
//! entry in the user's autostart folder ([XDG Autostart]). The file is the setting — what the
//! desktop's own tools change of it is what Settings show. Wholeheard writes it once by itself,
//! at its first start, and never again: once turned off, it stays off.
//!
//! [XDG Autostart]: https://specifications.freedesktop.org/autostart-spec/latest/

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::settings;

/// The entry's name: the package's own desktop entry's.
pub const NAME: &str = "dev.wholeheard.Wholeheard.desktop";

/// The tray alone, no window (D17); `TryExec`: a removed package starts nothing.
const ENTRY: &str = "[Desktop Entry]\n\
    Type=Application\n\
    Name=Wholeheard\n\
    Comment=Offers to record a call as it starts\n\
    Comment[fr]=Propose d’enregistrer un appel dès qu’il commence\n\
    Exec=wholeheard\n\
    TryExec=wholeheard\n\
    Icon=dev.wholeheard.Wholeheard\n\
    Terminal=false\n";

/// Where the entry is on this system: the user's configuration's `autostart` folder.
pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|folder| folder.join("autostart").join(NAME))
}

/// Whether the session starts Wholeheard: the entry is there, and neither hidden nor turned off
/// by GNOME's key.
pub fn on(entry: &Path) -> bool {
    let Ok(text) = fs::read_to_string(entry) else {
        return false;
    };
    let mut group = "";
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            group = line;
            continue;
        }
        if group != "[Desktop Entry]" {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match (key.trim(), value.trim()) {
            ("Hidden", "true") | ("X-GNOME-Autostart-enabled", "false") => return false,
            _ => {}
        }
    }
    true
}

/// Turns it on — the entry written whole — or off — the entry taken away.
pub fn set(entry: &Path, on: bool) -> io::Result<()> {
    if !on {
        return match fs::remove_file(entry) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            done => done,
        };
    }
    if let Some(folder) = entry.parent() {
        fs::create_dir_all(folder)?;
    }
    let draft = entry.with_extension("desktop.part");
    fs::write(&draft, ENTRY).and_then(|()| fs::rename(&draft, entry)).inspect_err(|_| {
        let _ = fs::remove_file(&draft);
    })
}

/// At a start: the entry written when the settings say it never was, and that it was, kept in
/// them. True when it was written now. A settings file that cannot be read writes nothing.
pub fn first(entry: &Path, settings_file: &Path) -> io::Result<bool> {
    let mut settings = settings::read(settings_file).map_err(|why| io::Error::new(io::ErrorKind::InvalidData, why))?;
    if settings.autostart.is_some() {
        return Ok(false);
    }
    set(entry, true)?;
    settings.autostart = Some(true);
    settings::write(settings_file, &settings)?;
    Ok(true)
}
