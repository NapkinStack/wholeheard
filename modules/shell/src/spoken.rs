//! The language a meeting is spoken in (D65): `language` of `contracts/session-audio/v2` (and
//! v1), one line, a code of ISO 639-1, which the shell writes on the user's word and alone reads
//! to tell the transcription. Absent, or a line that is not one such code, the meeting names none
//! of its own, and the settings' default holds — none: the engine detects it.

use std::{fs, io, path::Path};

/// The file's name, in the meeting's folder.
pub const FILE: &str = "language";

/// The languages offered first, by their code and as each names itself; Whisper hears these and
/// many more — any code of ISO 639-1 is kept.
pub const OFFERED: [(&str, &str); 8] = [
    ("fr", "Français"),
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("it", "Italiano"),
    ("pt", "Português"),
    ("nl", "Nederlands"),
    ("ar", "العربية"),
];

/// The codes of ISO 639-1 (the Library of Congress's list, its 183 codes in use).
const ISO_639_1: &str = "aa ab ae af ak am an ar as av ay az ba be bg bi bm bn bo br bs ca ce ch co cr cs cu cv cy \
da de dv dz ee el en eo es et eu fa ff fi fj fo fr fy ga gd gl gn gu gv ha he hi ho hr ht hu hy hz ia id ie ig ii ik io is \
it iu ja jv ka kg ki kj kk kl km kn ko kr ks ku kv kw ky la lb lg li ln lo lt lu lv mg mh mi mk ml mn mr ms mt my na nb nd \
ne ng nl nn no nr nv ny oc oj om or os pa pi pl ps pt qu rm rn ro ru rw sa sc sd se sg si sk sl sm sn so sq sr ss st su sv \
sw ta te tg th ti tk tl tn to tr ts tt tw ty ug uk ur uz ve vi vo wa wo xh yi yo za zh zu";

/// A code of ISO 639-1, as written — lower-case, once trimmed —; none otherwise.
pub fn code(text: &str) -> Option<String> {
    let code = text.trim();
    (code.len() == 2 && ISO_639_1.split_whitespace().any(|known| known == code)).then(|| code.to_string())
}

/// The language the meeting says it is spoken in, if it says one.
pub fn read(folder: &Path) -> Option<String> {
    fs::read_to_string(folder.join(FILE)).ok().and_then(|text| code(&text))
}

/// Said spoken in `language`: the line written whole — a file beside, renamed over it —; none:
/// removed. Refused before the folder holds `session.json` (`NotFound`), and for a text that is
/// not a code (`InvalidInput`).
pub fn mark(folder: &Path, language: Option<&str>) -> io::Result<()> {
    if !folder.join("session.json").is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the meeting's folder holds no session.json yet",
        ));
    }
    let file = folder.join(FILE);
    let Some(language) = language else {
        return match fs::remove_file(&file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    };
    let code = code(language).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a code of ISO 639-1"))?;
    let beside = folder.join(".language.part");
    let written = fs::write(&beside, format!("{code}\n")).and_then(|()| fs::rename(&beside, &file));
    if written.is_err() {
        let _ = fs::remove_file(&beside);
    }
    written
}
