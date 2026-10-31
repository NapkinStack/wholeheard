//! The name the user gave a meeting (D70): `title.txt` of `contracts/session-audio/v2` (and v1), one
//! line of plain text, which the shell writes on the user's word and alone reads. Absent, or anything
//! but one such line, the meeting has no name given: its minutes' title, or its date, names it.

use std::{fs, io, path::Path};

/// The file's name, in the meeting's folder.
pub const FILE: &str = "title.txt";

/// As long as the contract lets a title of the minutes be, counted in characters.
pub const MOST: usize = 80;

/// A name within the contract, once trimmed: on one line, not blank, at most `MOST` characters.
pub fn within(text: &str) -> Option<&str> {
    let name = text.trim();
    let one_line = !name.chars().any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'));
    // A name that shows nothing would leave an empty row: marks that take no room are no name.
    let shows = name.chars().any(|c| !c.is_whitespace() && !unseen(c));

    (one_line && shows && name.chars().count() <= MOST).then_some(name)
}

/// A character that takes no room of its own: Unicode's default-ignorable code points, and the two
/// blank letters and the blank pattern fonts draw as nothing.
fn unseen(c: char) -> bool {
    matches!(c,
        '\u{ad}' | '\u{34f}' | '\u{61c}' | '\u{115f}' | '\u{1160}' | '\u{17b4}' | '\u{17b5}'
        | '\u{180b}'..='\u{180f}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}'
        | '\u{2800}' | '\u{3164}' | '\u{fe00}'..='\u{fe0f}' | '\u{feff}' | '\u{ffa0}' | '\u{fff0}'..='\u{fffb}'
        | '\u{1bca0}'..='\u{1bca3}' | '\u{1d173}'..='\u{1d17a}' | '\u{e0000}'..='\u{e0fff}')
}

/// The name the user gave the meeting, if they gave one.
pub fn read(folder: &Path) -> Option<String> {
    let text = fs::read_to_string(folder.join(FILE)).ok()?;
    within(&text).map(str::to_string)
}

/// Named: the line written whole — a file beside, renamed over it —; blank: removed. Refused before
/// the folder holds `session.json` (`NotFound`), and for what is no such line (`InvalidInput`).
pub fn write(folder: &Path, text: &str) -> io::Result<()> {
    if !folder.join("session.json").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the meeting's folder holds no session.json yet",
        ));
    }
    let file = folder.join(FILE);
    if text.trim().is_empty() {
        return match fs::remove_file(&file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let name = within(text).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not one line of at most 80 characters"))?;
    let beside = folder.join(".title.part");
    let written = fs::write(&beside, format!("{name}\n")).and_then(|()| fs::rename(&beside, &file));
    if written.is_err() {
        let _ = fs::remove_file(&beside);
    }
    written
}
