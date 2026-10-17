//! The settings, as one file the user can read and write without Summit: a few lines of TOML
//! in the system's place for an application's configuration. A file that is not there is
//! every default; a key this version does not know is left alone, for the version that does —
//! also when the window writes the file again, with the user's comments.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// What follows a recording once it is stopped, without another gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum After {
    Nothing,
    Transcript,
    /// The transcript, then the minutes.
    Minutes,
}

/// How long a session's audio is kept; its transcript and minutes stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeepAudio {
    Always,
    Days(u32),
}

/// What the user chose. Each field has its default: a file may name one and leave the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Tell, at each start, that announcing the recording is the user's to do.
    pub reminder: bool,
    /// The folder the sessions are written in, one folder each.
    pub sessions: PathBuf,
    pub after: After,
    /// The Whisper model the transcript is heard with: a file in whisper.cpp's format.
    pub model: PathBuf,
    /// Who writes the minutes: `ollama`, on this computer, or a provider's address; and the
    /// model's name as it knows it. None: not chosen yet.
    pub minutes_provider: Option<String>,
    pub minutes_model: Option<String>,
    pub keep_audio: KeepAudio,
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
            after: After::Transcript,
            model: data.join("summit").join("models").join(MODEL),
            minutes_provider: None,
            minutes_model: None,
            keep_audio: KeepAudio::Always,
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
    /// Before `after`: `false` was "nothing follows".
    transcript: Option<bool>,
    after: Option<After>,
    model: Option<toml::Spanned<PathBuf>>,
    minutes_provider: Option<String>,
    minutes_model: Option<String>,
    keep_audio: Option<toml::Spanned<toml::Value>>,
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
    settings.after = match (written.after, written.transcript) {
        (Some(after), _) => after,
        (None, Some(false)) => After::Nothing,
        (None, _) => settings.after,
    };
    settings.minutes_provider = written.minutes_provider.filter(|named| !named.trim().is_empty());
    settings.minutes_model = written.minutes_model.filter(|named| !named.trim().is_empty());
    if let Some(keep) = written.keep_audio {
        let at = line(keep.span().start);
        settings.keep_audio = match keep.get_ref() {
            toml::Value::String(always) if always == "always" => KeepAudio::Always,
            toml::Value::Integer(days) if (1..=36_500).contains(days) => KeepAudio::Days(*days as u32),
            _ => {
                return Err(format!(
                    "{}, line {at}: keep_audio is \"always\" or a number of days",
                    path.display()
                ));
            }
        };
    }
    if let Some(sessions) = written.sessions {
        settings.sessions = whole("sessions", "the folder's", sessions)?;
    }
    if let Some(model) = written.model {
        settings.model = whole("model", "the file's", model)?;
    }
    Ok(settings)
}

/// Writes the settings to their file, as `read` reads them. Only what changed is touched: a key
/// whose value is already the file's — or, absent, the default's — is left as the user wrote it;
/// a key that changes keeps the comment on its line; every other key and comment stays. A file
/// `read` refuses is never written over: the user's choice in it would be lost. Written whole or
/// not at all, with the file's permissions, through a link to where it points.
pub fn write(path: &Path, settings: &Settings) -> io::Result<()> {
    use toml_edit::{DocumentMut, Item, Value};

    let refused = |why: String| io::Error::new(io::ErrorKind::InvalidData, why);
    // Through a link, the file it points to.
    let path = &fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let before = read(path).map_err(refused)?;
    let mut document = match fs::read_to_string(path) {
        Ok(text) => text.parse::<DocumentMut>().map_err(|error| refused(error.to_string()))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => DocumentMut::new(),
        Err(error) => return Err(error),
    };
    // Sets a key to a value, keeping the comment its line had.
    fn set(document: &mut DocumentMut, key: &str, mut new: Value) {
        if let Some(old) = document.get(key).and_then(Item::as_value) {
            *new.decor_mut() = old.decor().clone();
        }
        document[key] = Item::Value(new);
    }
    let text = |path: &Path| Value::from(path.to_string_lossy().into_owned());
    if settings.reminder != before.reminder {
        set(&mut document, "reminder", Value::from(settings.reminder));
    }
    if settings.sessions != before.sessions {
        set(&mut document, "sessions", text(&settings.sessions));
    }
    if settings.after != before.after {
        set(
            &mut document,
            "after",
            Value::from(match settings.after {
                After::Nothing => "nothing",
                After::Transcript => "transcript",
                After::Minutes => "minutes",
            }),
        );
    }
    if settings.model != before.model {
        set(&mut document, "model", text(&settings.model));
    }
    if settings.keep_audio != before.keep_audio {
        set(
            &mut document,
            "keep_audio",
            match settings.keep_audio {
                KeepAudio::Always => Value::from("always"),
                KeepAudio::Days(days) => Value::from(i64::from(days)),
            },
        );
    }
    for (key, named, was) in [
        ("minutes_provider", &settings.minutes_provider, &before.minutes_provider),
        ("minutes_model", &settings.minutes_model, &before.minutes_model),
    ] {
        match named {
            Some(named) if named != was.as_deref().unwrap_or_default() => set(&mut document, key, Value::from(named.as_str())),
            Some(_) => {}
            None => {
                document.remove(key);
            }
        }
    }
    // Said by `after` now, once `after` is written: the key it replaced would contradict it.
    if document.contains_key("after") {
        document.remove("transcript");
    }
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let draft = path.with_extension("toml.part");
    let written = fs::write(&draft, document.to_string()).and_then(|()| match fs::metadata(path) {
        Ok(old) => fs::set_permissions(&draft, old.permissions()),
        Err(_) => Ok(()),
    });
    written.and_then(|()| fs::rename(&draft, path)).inspect_err(|_| {
        let _ = fs::remove_file(&draft);
    })
}
