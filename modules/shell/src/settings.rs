//! The settings, as one file the user can read and write without Wholeheard: a few lines of TOML
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

/// How much the logs hold: the usual steps, or more for whoever looks into a failure (D29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Log {
    Info,
    Debug,
}

impl Log {
    /// As `WHOLEHEARD_LOG` names it, for the shell and the programs it starts.
    pub fn as_str(self) -> &'static str {
        match self {
            Log::Info => "info",
            Log::Debug => "debug",
        }
    }
}

/// The window Wholeheard opens: the one the user left, the small one first (D35).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Window {
    Small,
    Full,
}

impl Window {
    pub fn as_str(self) -> &'static str {
        match self {
            Window::Small => "small",
            Window::Full => "full",
        }
    }
}

/// Who hears a meeting on this computer: a Whisper model, or Apple's SpeechAnalyzer on a Mac
/// with macOS 26 (D42, ADR-0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Whisper,
    SpeechAnalyzer,
    /// A service the user named, as for the minutes (D43, ADR-0013).
    Service,
}

impl Engine {
    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Whisper => "whisper",
            Engine::SpeechAnalyzer => "speechanalyzer",
            Engine::Service => "service",
        }
    }
}

/// How long a session's audio is kept; its transcript and minutes stay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Who hears: the model above, Apple's SpeechAnalyzer (D42), or a service the user named (D43).
    pub engine: Engine,
    /// Who hears when a service does: its address, as `wholeheard-transcription --provider` takes it,
    /// its model's name, and the host the user agreed to send a meeting's sound to (D43).
    pub transcription_provider: Option<String>,
    pub transcription_model: Option<String>,
    pub transcription_agreed: Option<String>,
    /// Who writes the minutes: a provider's address, as `wholeheard-summary` takes it — Ollama on this
    /// computer is `http://localhost:11434/v1` —, and the model's name as it knows it. None: not
    /// chosen yet.
    pub minutes_provider: Option<String>,
    pub minutes_model: Option<String>,
    /// The language meetings are spoken in, unless one says its own (D65): a code of ISO 639-1;
    /// none, the engine detects it.
    pub meeting_language: Option<String>,
    /// The host the user agreed to send a meeting to, the first time minutes went to it; another
    /// host asks again. None: none yet.
    pub minutes_agreed: Option<String>,
    pub keep_audio: KeepAudio,
    /// Shows, under a failure on the window's page, what the program said of it (D29).
    pub developer: bool,
    /// The logs' level, from Wholeheard's next start.
    pub log: Log,
    /// Offer to record when another application takes a microphone (D30).
    pub offer: bool,
    /// Whether Wholeheard wrote its entry of the session's start, and as what (D32): none, never
    /// — it is written at the next start. The entry itself is the setting.
    pub autostart: Option<bool>,
    /// The window opened from the menu or the tray: the one left (D35).
    pub window: Window,
    /// The first run's assistant was shown (D58): it opens by itself at one start only.
    pub welcomed: bool,
    /// The full window's sidebar folded by the user (D61): opened again as it was left.
    pub folded: bool,
}

/// The model a transcript is heard with when the settings name none (ADR-0003).
pub const MODEL: &str = "ggml-large-v3-turbo-q8_0.bin";

impl Default for Settings {
    fn default() -> Self {
        let documents = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        let data = dirs::data_dir().unwrap_or_default();
        Self {
            reminder: true,
            sessions: documents.join("Wholeheard"),
            after: After::Transcript,
            model: data.join("wholeheard").join("models").join(MODEL),
            engine: Engine::Whisper,
            transcription_provider: None,
            transcription_model: None,
            transcription_agreed: None,
            minutes_provider: None,
            minutes_model: None,
            meeting_language: None,
            minutes_agreed: None,
            keep_audio: KeepAudio::Always,
            developer: false,
            offer: true,
            autostart: None,
            window: Window::Small,
            log: Log::Info,
            welcomed: false,
            folded: false,
        }
    }
}

/// The Whisper models on this computer: the `.bin` files in `folder` — Wholeheard's, where the
/// default model goes — and the one the settings name wherever it is. Each file once, however it
/// is reached — by a link, by another path —, the folder's entry kept; by name, whatever its case.
pub fn whisper_models(folder: &Path, settings: &Settings) -> Vec<PathBuf> {
    let model = |path: &Path| path.is_file() && path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("bin"));
    let mut found: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| model(path))
        .collect();
    found.push(settings.model.clone());
    let mut seen = std::collections::HashSet::new();
    found.retain(|path| path.is_file() && fs::canonicalize(path).is_ok_and(|file| seen.insert(file)));
    found.sort_by_key(|path| path.file_name().map(|name| name.to_string_lossy().to_lowercase()));
    found
}

/// Whether the first run's assistant opens by itself at this start (D58): never shown yet, and no
/// meeting in the sessions' folder — whoever has recorded already never meets it.
pub fn welcome_due(settings: &Settings) -> bool {
    !settings.welcomed && crate::sessions::list(&settings.sessions, None).is_empty()
}

/// Where the settings are on this system: `wholeheard/settings.toml` in the user's configuration.
pub fn file() -> Option<PathBuf> {
    dirs::config_dir().map(|folder| folder.join("wholeheard").join("settings.toml"))
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
    engine: Option<Engine>,
    transcription_provider: Option<String>,
    transcription_model: Option<String>,
    transcription_agreed: Option<String>,
    minutes_provider: Option<String>,
    minutes_model: Option<String>,
    meeting_language: Option<String>,
    minutes_agreed: Option<String>,
    keep_audio: Option<toml::Spanned<toml::Value>>,
    developer: Option<bool>,
    offer: Option<bool>,
    autostart: Option<bool>,
    window: Option<Window>,
    log: Option<Log>,
    welcomed: Option<bool>,
    folded: Option<bool>,
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
        let named = from_home(written.into_inner());
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
    settings.developer = written.developer.unwrap_or(settings.developer);
    settings.offer = written.offer.unwrap_or(settings.offer);
    settings.autostart = written.autostart;
    settings.window = written.window.unwrap_or(settings.window);
    settings.welcomed = written.welcomed.unwrap_or(settings.welcomed);
    settings.folded = written.folded.unwrap_or(settings.folded);
    settings.log = written.log.unwrap_or(settings.log);
    settings.engine = written.engine.unwrap_or(settings.engine);
    settings.after = match (written.after, written.transcript) {
        (Some(after), _) => after,
        (None, Some(false)) => After::Nothing,
        (None, _) => settings.after,
    };
    settings.minutes_provider = written.minutes_provider.filter(|named| !named.trim().is_empty());
    settings.minutes_model = written.minutes_model.filter(|named| !named.trim().is_empty());
    settings.meeting_language = written.meeting_language.as_deref().and_then(crate::spoken::code);
    settings.minutes_agreed = written.minutes_agreed.filter(|named| !named.trim().is_empty());
    settings.transcription_provider = written.transcription_provider.filter(|named| !named.trim().is_empty());
    settings.transcription_model = written.transcription_model.filter(|named| !named.trim().is_empty());
    settings.transcription_agreed = written.transcription_agreed.filter(|named| !named.trim().is_empty());
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

/// A path as people write it, from their home with a tilde, made whole; any other as it is.
pub fn from_home(written: PathBuf) -> PathBuf {
    match (written.strip_prefix("~"), dirs::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => written,
    }
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
    if settings.developer != before.developer {
        set(&mut document, "developer", Value::from(settings.developer));
    }
    if settings.offer != before.offer {
        set(&mut document, "offer", Value::from(settings.offer));
    }
    if let Some(autostart) = settings.autostart
        && settings.autostart != before.autostart
    {
        set(&mut document, "autostart", Value::from(autostart));
    }
    if settings.window != before.window {
        set(&mut document, "window", Value::from(settings.window.as_str()));
    }
    if settings.welcomed != before.welcomed {
        set(&mut document, "welcomed", Value::from(settings.welcomed));
    }
    if settings.folded != before.folded {
        set(&mut document, "folded", Value::from(settings.folded));
    }
    if settings.log != before.log {
        set(&mut document, "log", Value::from(settings.log.as_str()));
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
    if settings.engine != before.engine {
        set(&mut document, "engine", Value::from(settings.engine.as_str()));
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
        ("meeting_language", &settings.meeting_language, &before.meeting_language),
        ("minutes_agreed", &settings.minutes_agreed, &before.minutes_agreed),
        (
            "transcription_provider",
            &settings.transcription_provider,
            &before.transcription_provider,
        ),
        ("transcription_model", &settings.transcription_model, &before.transcription_model),
        ("transcription_agreed", &settings.transcription_agreed, &before.transcription_agreed),
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
