//! `summit` — Summit as the user meets it: an icon in the tray, a global shortcut,
//! notifications, and a window opened from the tray's menu — or at once with `--window`, the
//! user's gesture too. It starts the program that records and stops it, then the one that
//! hears; it records and hears nothing itself, and a recording goes on if this process does
//! not.
//!
//! Everything it decides is in the library beside it — `recording`, `sessions`, `settings`,
//! `transcription`, `words` —
//! and tried there. Here is only what needs a desktop.

use std::{
    collections::HashMap,
    collections::VecDeque,
    path::PathBuf,
    sync::{Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use summit_shell::{
    desktop, keys,
    meeting::{self, Read, Transcript},
    minutes::{self, Minutes},
    model::{self, Published},
    providers::{self, Online},
    recording::{Recording, Said},
    sessions::{self, Listed},
    settings::{self, After, KeepAudio, Settings},
    summarising::{self, Summarising},
    transcription::{self, Made, Transcription},
    words::{self, Language, Notice, Refusal, Trouble},
};
use tauri::{
    AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, Wry,
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_notification::NotificationExt;

const TRAY: &str = "summit";

/// What the shell holds while it runs.
struct Shell {
    /// What the user chose: read at start, changed from the window.
    settings: Mutex<Settings>,
    /// Held while a change is read onto the file and written: two saves never interleave, the
    /// later undoing the earlier.
    saving: Mutex<()>,
    /// Why the system's keyring could not be opened, when it could not.
    keyring: Mutex<Option<String>>,
    language: Language,
    /// The recording in progress, when there is one.
    recording: Mutex<Option<Recording>>,
    /// The folder it records into, once it has said so.
    recorded: Mutex<Option<PathBuf>>,
    /// The menu's entry that starts and stops, relabelled as the recording goes.
    toggle: MenuItem<Wry>,
    /// The user asked to quit while a recording ran: it is stopped first, then Summit leaves.
    leaving: Mutex<bool>,
    /// When the recording was last started or told to stop: a second press in the same
    /// half-second is the same gesture, not a second one.
    pressed: Mutex<Option<Instant>>,
    /// The recording was told to stop and has not ended yet.
    stopping: Mutex<bool>,
    /// The menu's entry that makes the transcript of the last recording.
    transcribe: MenuItem<Wry>,
    /// The transcript being made, when there is one: one at a time.
    transcription: Mutex<Option<Transcription>>,
    /// The keys the desktop granted the shortcut, as it says them; none while it has not.
    shortcut: Mutex<Option<String>>,
    /// The menu's entry that has the minutes of the last recording written.
    minutes: MenuItem<Wry>,
    /// A transcript made again: where it goes back, and where it was set aside — taken by the
    /// run's end, or by Summit quitting, whichever comes first.
    redoing: Mutex<Option<(PathBuf, PathBuf)>>,
    /// The list's small reliefs, by transcript, kept until it changes.
    shapes: Mutex<Shapes>,
    /// The minutes being written, when they are: one at a time, taken from the gesture on.
    minuting: Mutex<Option<Summarising>>,
    writing_minutes: Mutex<bool>,
    /// The sessions whose transcripts wait for the one being made, or for the model — in
    /// the order they were asked, each once.
    waiting: Mutex<VecDeque<PathBuf>>,
    /// The tray's menu, and its entry that fetches the default model — in it only while the
    /// model is needed and not there.
    menu: Menu<Wry>,
    download: MenuItem<Wry>,
    /// The model is coming.
    fetching: Mutex<bool>,
}

impl Shell {
    /// What the user chose, as it stands now.
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    /// Reads the settings file again — the user may have changed it by hand — and holds what it
    /// says; a file that cannot be read is said, and what was held stays.
    fn reread(&self) -> Result<Settings, String> {
        let read = settings::file().map_or_else(|| Ok(Settings::default()), |file| settings::read(&file));
        if let Ok(settings) = &read {
            *self.settings.lock().unwrap() = settings.clone();
        }
        read
    }
}

/// Where the tray is in the life of a recording.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    Idle,
    Recording,
    Stopping,
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            listed,
            header,
            record,
            minutes_asked,
            minutes_redo,
            transcript_redo,
            transcript_asked,
            opened,
            open_folder,
            settings_shown,
            settings_saved,
            minutes_shown,
            minutes_saved,
            minutes_tested,
            agreement_shown,
            agreed,
            local_instead,
            choose_folder,
            shortcut_settings
        ])
        .setup(|app| {
            let language = Language::system();
            let labels = words::menu(language);
            let read = settings::file().map_or_else(|| Ok(Settings::default()), |file| settings::read(&file));
            let toggle = MenuItem::with_id(app, "toggle", labels.start, true, None::<&str>)?;
            let again = MenuItem::with_id(app, "transcribe", labels.transcribe, true, None::<&str>)?;
            let minutes = MenuItem::with_id(app, "minutes", labels.minutes, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", labels.open, true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &again, &minutes, &quit])?;
            let download = MenuItem::with_id(app, "download", "", true, None::<&str>)?;
            TrayIconBuilder::with_id(TRAY)
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Summit")
                .menu(&menu.clone())
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => open_window(app),
                    "toggle" => toggle_recording(app),
                    "transcribe" => {
                        let shell = app.state::<Shell>();
                        // The latest session is the one being recorded: its transcript follows its stop.
                        if shell.recording.lock().unwrap().is_some() {
                            return;
                        }
                        match transcription::latest(&shell.settings().sessions) {
                            Some(last) => transcribe(app, last),
                            None => tell(app, &words::trouble(&Trouble::NoSession, shell.language)),
                        }
                    }
                    "minutes" => {
                        let shell = app.state::<Shell>();
                        // The latest session is the one being recorded: it has no transcript yet.
                        if shell.recording.lock().unwrap().is_some() {
                            return;
                        }
                        match transcription::latest(&shell.settings().sessions) {
                            Some(last) => make_minutes(app, last),
                            None => tell(app, &words::trouble(&Trouble::NoSession, shell.language)),
                        }
                    }
                    "download" => fetch(app),
                    "quit" => leave(app),
                    _ => {}
                })
                .build(app)?;
            app.manage(Shell {
                settings: Mutex::new(read.clone().unwrap_or_default()),
                saving: Mutex::new(()),
                keyring: Mutex::new(None),
                language,
                recording: Mutex::new(None),
                recorded: Mutex::new(None),
                toggle,
                leaving: Mutex::new(false),
                pressed: Mutex::new(None),
                stopping: Mutex::new(false),
                transcribe: again,
                transcription: Mutex::new(None),
                shortcut: Mutex::new(None),
                minutes: minutes.clone(),
                minuting: Mutex::new(None),
                shapes: Mutex::new(HashMap::new()),
                redoing: Mutex::new(None),
                writing_minutes: Mutex::new(false),
                waiting: Mutex::new(VecDeque::new()),
                menu,
                download,
                fetching: Mutex::new(false),
            });
            if let Err(why) = read {
                tell(app.handle(), &words::trouble(&Trouble::Settings(why), language));
            }
            shortcut(app.handle());
            // The audio kept longer than the settings say goes to the trash, now and every hour.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut refused = None;
                loop {
                    expire(&handle, &mut refused);
                    std::thread::sleep(Duration::from_secs(3600));
                }
            });
            // The keyring a provider's key is kept in: refused, a key cannot be kept, and the
            // settings page says so when one is given.
            let keyring = keys::open_system_keyring().err();
            *app.state::<Shell>().keyring.lock().unwrap() = keyring;
            if std::env::args().skip(1).any(|arg| arg == "--window") {
                open_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Summit could not start");
    // No window is no reason to leave: Summit lives in the tray until the user quits it.
    app.run(|_, event| {
        if let RunEvent::ExitRequested { api, code: None, .. } = event {
            api.prevent_exit();
        }
    });
}

/// Opens the window, or brings it forward when it is open. Only ever on a gesture of the user.
fn open_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Summit")
        .inner_size(1100.0, 800.0)
        .min_inner_size(420.0, 400.0)
        .build();
}

/// What the window shows: its words, the folder it reads, and the sessions in it.
#[derive(serde::Serialize)]
struct Listing {
    words: words::Window,
    folder: PathBuf,
    sessions: Vec<Listed>,
    /// For each session, in the same order, who spoke over it — here, then remote — when its
    /// transcript reads: the small relief each row draws.
    shapes: Vec<Option<[Vec<f32>; 2]>>,
    /// Whether a recording runs, and the shortcut's keys as the desktop granted them: the header's.
    recording: bool,
    shortcut: Option<String>,
}

/// A meeting's small relief, kept until its transcript changes: the list is drawn each time the
/// window comes forward, and a transcript is read again only when it was written meanwhile.
type Shapes = HashMap<PathBuf, (Option<std::time::SystemTime>, u64, Option<[Vec<f32>; 2]>)>;

/// The list, read away from the window's and the tray's thread: it reads every transcript it
/// has not read yet.
#[tauri::command]
async fn listed(app: AppHandle) -> Result<Listing, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shell = app.state::<Shell>();
        let recorded = shell.recorded.lock().unwrap().clone();
        let sessions = sessions::list(&shell.settings().sessions, recorded.as_deref());
        let mut kept = shell.shapes.lock().unwrap();
        let shapes = sessions
            .iter()
            .map(|session| {
                let file = session.folder.join("transcript.jsonl");
                let stamp = std::fs::metadata(&file)
                    .map(|data| (data.modified().ok(), data.len()))
                    .unwrap_or((None, 0));
                if let Some((modified, length, shape)) = kept.get(&file)
                    && (*modified, *length) == stamp
                    && session.seconds.is_some()
                {
                    return shape.clone();
                }
                let shape = match meeting::transcript(&session.folder) {
                    Read::Present(transcript) if !transcript.segments.is_empty() => {
                        let said = transcript.segments.iter().map(|segment| segment.end).fold(0.0, f64::max);
                        Some(meeting::shape(&transcript, session.seconds.unwrap_or(said).max(said), 24))
                    }
                    _ => None,
                };
                kept.insert(file, (stamp.0, stamp.1, shape.clone()));
                shape
            })
            .collect();
        drop(kept);
        Listing {
            words: words::window(shell.language),
            folder: shell.settings().sessions.clone(),
            sessions,
            shapes,
            recording: shell.recording.lock().unwrap().is_some(),
            shortcut: shell.shortcut.lock().unwrap().clone(),
        }
    })
    .await
    .map_err(|error| error.to_string())
}

/// What the header shows, asked often: whether a recording runs — started from the tray or the
/// shortcut as well —, and the shortcut's keys.
#[derive(serde::Serialize)]
struct Header {
    recording: bool,
    shortcut: Option<String>,
}

#[tauri::command]
fn header(shell: State<'_, Shell>) -> Header {
    Header {
        recording: shell.recording.lock().unwrap().is_some(),
        shortcut: shell.shortcut.lock().unwrap().clone(),
    }
}

/// What a meeting lacks, made on the user's word from its page (D18): its minutes — through the
/// same gate as the tray's, the agreement included —, or its transcript.
#[tauri::command]
fn minutes_asked(app: AppHandle, shell: State<'_, Shell>, name: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    make_minutes(&app, folder);
    Ok(())
}

/// *Refaire…*: the minutes made again, with the model chosen on the page, this time only — the
/// settings' choice when none is named. Through the same gate; the minutes there stay until the
/// new ones are whole, then go to the trash.
#[tauri::command]
fn minutes_redo(
    app: AppHandle,
    shell: State<'_, Shell>,
    name: String,
    provider: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    write_minutes(&app, folder, None, provider.zip(model));
    Ok(())
}

#[tauri::command]
fn transcript_redo(app: AppHandle, shell: State<'_, Shell>, name: String, model: PathBuf) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    retranscribe(&app, folder, model)
}

#[tauri::command]
fn transcript_asked(app: AppHandle, shell: State<'_, Shell>, name: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    // The meeting being recorded has no transcript to make yet.
    if shell.recording.lock().unwrap().is_some() && shell.recorded.lock().unwrap().as_deref() == Some(folder.as_path()) {
        return Err(format!("{name} is being recorded"));
    }
    transcribe(&app, folder);
    Ok(())
}

/// The header's button: starts a recording, or stops the one running — as the tray and the
/// shortcut do.
#[tauri::command]
fn record(app: AppHandle) {
    toggle_recording(&app);
}

/// The folder of a session the window names (`sessions::folder`).
fn session_folder(shell: &Shell, name: &str) -> Result<PathBuf, String> {
    sessions::folder(&shell.settings().sessions, name).ok_or_else(|| format!("{name} is no session"))
}

/// One meeting, as its page shows it.
#[derive(serde::Serialize)]
struct Opened {
    session: Listed,
    transcript: Read<Transcript>,
    minutes: Read<Minutes>,
}

#[tauri::command]
fn opened(shell: State<'_, Shell>, name: String) -> Result<Opened, String> {
    let folder = session_folder(&shell, &name)?;
    let recorded = shell.recorded.lock().unwrap().clone();
    let session = sessions::list(&shell.settings().sessions, recorded.as_deref())
        .into_iter()
        .find(|listed| listed.folder == folder)
        .ok_or_else(|| format!("{name} is no session"))?;
    Ok(Opened {
        session,
        transcript: meeting::transcript(&folder),
        minutes: minutes::minutes(&folder),
    })
}

/// What the agreement page shows before a meeting first leaves for a host (D11's fifth board):
/// to whom, with which model, and what goes — the transcript, its length and how many words; the
/// context the user attached, how many words —; the audio stays. And the models that would write
/// the minutes on this computer instead.
#[derive(serde::Serialize)]
struct Agreement {
    host: String,
    model: String,
    seconds: f64,
    words: usize,
    context_words: Option<usize>,
    ollama: Vec<String>,
}

#[tauri::command]
async fn agreement_shown(shell: State<'_, Shell>, name: String) -> Result<Agreement, String> {
    let folder = session_folder(&shell, &name)?;
    let settings = shell.reread()?;
    let (Some(provider), Some(model)) = (settings.minutes_provider, settings.minutes_model) else {
        return Err("no one is chosen to write the minutes".into());
    };
    let meeting::Read::Present(transcript) = meeting::transcript(&folder) else {
        return Err(format!("{name} has no transcript to send"));
    };
    let words = |text: &str| text.split_whitespace().count();
    let listed = sessions::list(&settings.sessions, None)
        .into_iter()
        .find(|listed| listed.folder == folder);
    let said_until = transcript.segments.iter().map(|segment| segment.end).fold(0.0, f64::max);
    let base = ollama();
    let ollama = tauri::async_runtime::spawn_blocking(move || providers::ollama_models(&base))
        .await
        .unwrap_or_default();
    Ok(Agreement {
        host: providers::host(&provider).unwrap_or(provider),
        model,
        seconds: listed.and_then(|listed| listed.seconds).unwrap_or(said_until),
        words: transcript.segments.iter().map(|segment| words(&segment.text)).sum(),
        context_words: std::fs::read_to_string(folder.join("context.txt"))
            .ok()
            .map(|context| words(&context))
            .filter(|&n| n > 0),
        ollama,
    })
}

/// The user's word: this meeting is sent; and, when they ticked it, the host is agreed to for the
/// next ones, kept in the settings.
#[tauri::command]
fn agreed(app: AppHandle, shell: State<'_, Shell>, name: String, host: String, remember: bool) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    // The word given is for the host the page showed: chosen otherwise meanwhile, nothing goes.
    let now = shell.reread()?.minutes_provider.as_deref().and_then(providers::host);
    if now.as_deref() != Some(host.as_str()) {
        return Err(format!("the minutes are no longer written by {host}: look again"));
    }
    if remember {
        let _saving = shell.saving.lock().unwrap();
        let mut settings = shell.reread()?;
        // Kept only for the host shown, as the file still names it.
        if settings.minutes_provider.as_deref().and_then(providers::host).as_deref() != Some(host.as_str()) {
            return Err(format!("the minutes are no longer written by {host}: look again"));
        }
        settings.minutes_agreed = Some(host.clone());
        let file = settings::file().ok_or("this system names no folder for settings")?;
        settings::write(&file, &settings).map_err(|error| error.to_string())?;
        *shell.settings.lock().unwrap() = settings;
    }
    write_minutes(&app, folder, Some(&host), None);
    Ok(())
}

/// Written on this computer instead: Ollama's model chosen for these minutes and the next ones;
/// nothing is agreed to, nothing leaves.
#[tauri::command]
fn local_instead(app: AppHandle, shell: State<'_, Shell>, name: String, model: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    if model.trim().is_empty() {
        return Err("a model is named".into());
    }
    {
        let _saving = shell.saving.lock().unwrap();
        let mut settings = shell.reread()?;
        settings.minutes_provider = Some(providers::ollama_address(&ollama()));
        settings.minutes_model = Some(model.trim().to_string());
        let file = settings::file().ok_or("this system names no folder for settings")?;
        settings::write(&file, &settings).map_err(|error| error.to_string())?;
        *shell.settings.lock().unwrap() = settings;
    }
    make_minutes(&app, folder);
    Ok(())
}

/// Opens a session's folder in the system's file manager.
#[tauri::command]
fn open_folder(shell: State<'_, Shell>, name: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    // Waited for on a thread of its own, so that nothing of it is left in the system's table.
    let mut started = std::process::Command::new(opener)
        .arg(&folder)
        .spawn()
        .map_err(|error| error.to_string())?;
    std::thread::spawn(move || started.wait());
    Ok(())
}

/// A Whisper model the settings page offers.
#[derive(serde::Serialize)]
struct Model {
    path: PathBuf,
    name: String,
    bytes: u64,
}

/// What the settings page shows: the choices as they stand, the models on this computer, and
/// why the file is not written when it could not be read.
#[derive(serde::Serialize)]
struct SettingsShown {
    /// The shortcut's keys as the desktop granted them, and whether its keyboard settings, where
    /// they change, can be opened from here.
    shortcut: Option<String>,
    shortcut_settings: bool,
    sessions: PathBuf,
    /// The same, as people write it: from their home, with a tilde.
    sessions_said: String,
    after: After,
    reminder: bool,
    keep_audio: KeepAudio,
    model: PathBuf,
    models: Vec<Model>,
    unreadable: Option<String>,
}

/// The folder Summit keeps its models in: the default model's.
fn models_folder() -> PathBuf {
    Settings::default().model.parent().map(PathBuf::from).unwrap_or_default()
}

#[tauri::command]
fn settings_shown(shell: State<'_, Shell>) -> SettingsShown {
    let read = shell.reread();
    let unreadable = read.as_ref().err().cloned();
    let settings = read.unwrap_or_else(|_| shell.settings());
    let models = settings::whisper_models(&models_folder(), &settings)
        .into_iter()
        .map(|path| Model {
            // One outside Summit's folder says where it is: two files may share a name.
            name: match path.parent() {
                Some(parent) if parent != models_folder() => path.display().to_string(),
                _ => path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            },
            bytes: std::fs::metadata(&path).map_or(0, |data| data.len()),
            path,
        })
        .collect();
    SettingsShown {
        shortcut: shell.shortcut.lock().unwrap().clone(),
        shortcut_settings: keyboard_settings().is_some(),
        sessions_said: match dirs::home_dir().and_then(|home| settings.sessions.strip_prefix(home).ok().map(PathBuf::from)) {
            Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Some(rest) => format!("~/{}", rest.display()),
            None => settings.sessions.display().to_string(),
        },
        sessions: settings.sessions,
        after: settings.after,
        reminder: settings.reminder,
        keep_audio: settings.keep_audio,
        model: settings.model,
        models,
        unreadable,
    }
}

/// One choice the page changed.
#[derive(serde::Deserialize)]
struct Change {
    sessions: Option<PathBuf>,
    after: Option<After>,
    reminder: Option<bool>,
    keep_audio: Option<KeepAudio>,
    model: Option<PathBuf>,
}

/// Writes a change to the settings file, then holds it: the next recording follows it. Nothing
/// is written over a file that could not be read, and a model is only one found on this computer.
#[tauri::command]
fn settings_saved(shell: State<'_, Shell>, change: Change) -> Result<(), String> {
    // The change goes onto what the file says now: what the user changed by hand stays.
    let _saving = shell.saving.lock().unwrap();
    let mut settings = shell.reread()?;
    // A folder for the sessions: a whole path, made when it is not there yet.
    if let Some(sessions) = change.sessions {
        if sessions.as_os_str().is_empty() {
            return Err("a folder is named".into());
        }
        let sessions = settings::from_home(sessions);
        if !sessions.is_absolute() {
            return Err(format!("{} is no whole path, from the root or from ~", sessions.display()));
        }
        std::fs::create_dir_all(&sessions).map_err(|error| format!("{}: {error}", sessions.display()))?;
        settings.sessions = sessions;
    }
    if let Some(after) = change.after {
        settings.after = after;
    }
    if let Some(reminder) = change.reminder {
        settings.reminder = reminder;
    }
    // The same bounds as the file's: a value read refuses is never written.
    if let Some(KeepAudio::Days(days)) = change.keep_audio
        && !(1..=36_500).contains(&days)
    {
        return Err(format!("{days} is no number of days to keep audio"));
    }
    if let Some(keep) = change.keep_audio {
        settings.keep_audio = keep;
    }
    if let Some(model) = change.model {
        if !settings::whisper_models(&models_folder(), &settings).contains(&model) {
            return Err(format!("{} is no model on this computer", model.display()));
        }
        settings.model = model;
    }
    let file = settings::file().ok_or("this system names no folder for settings")?;
    settings::write(&file, &settings).map_err(|error| error.to_string())?;
    *shell.settings.lock().unwrap() = settings;
    Ok(())
}

/// Sends to the system's trash the audio of the sessions kept longer than the settings say — never
/// deleted, restored from the trash —, and says so once. A session's folder that is a link the
/// user put in the sessions' folder is followed, as the list follows it. A refusal is said once,
/// and again only when its reason changes: `refused` is the last one said.
fn expire(app: &AppHandle, refused: &mut Option<Refusal>) {
    let shell = app.state::<Shell>();
    let Ok(settings) = shell.reread() else { return };
    let KeepAudio::Days(days) = settings.keep_audio else { return };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64);
    let gone = sessions::expired_audio(&settings.sessions, days, now);
    if gone.is_empty() {
        return;
    }
    let meetings: std::collections::HashSet<_> = gone
        .iter()
        .filter_map(|path| path.strip_prefix(&settings.sessions).ok()?.components().next())
        .collect();
    let outcome = trash::delete_all(&gone)
        .map(|()| (meetings.len(), days))
        .map_err(|error| refusal(&error));
    let said_already = matches!((outcome, *refused), (Err(why), Some(said)) if why == said);
    *refused = outcome.err();
    if !said_already {
        tell(app, &words::audio_trashed(shell.language, &outcome));
    }
}

/// The desktop's keyboard settings, where a global shortcut is changed: GNOME's, when it is there.
fn keyboard_settings() -> Option<PathBuf> {
    // Another desktop's settings do not change what its portal granted.
    if !cfg!(target_os = "linux") || !desktop::gnome(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()) {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|folder| folder.join("gnome-control-center"))
        .find(|program| program.is_file())
}

/// Opens the desktop's keyboard settings: the shortcut is the desktop's to change.
#[tauri::command]
fn shortcut_settings() -> Result<(), String> {
    let program = keyboard_settings().ok_or("this desktop's keyboard settings cannot be opened from Summit")?;
    let mut started = std::process::Command::new(program)
        .arg("keyboard")
        .spawn()
        .map_err(|error| error.to_string())?;
    std::thread::spawn(move || started.wait());
    Ok(())
}

/// A folder chosen in the system's own dialog, opened where the sessions are; none when the user
/// closes it.
#[tauri::command]
async fn choose_folder(app: AppHandle) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    let shell = app.state::<Shell>();
    let from = shell.reread().unwrap_or_else(|_| shell.settings()).sessions;
    let chosen = app.dialog().file().set_directory(from).blocking_pick_folder()?;
    chosen.into_path().ok()
}

/// Ollama on this computer — or, for whoever tries Summit, where `SUMMIT_OLLAMA` says.
fn ollama() -> String {
    std::env::var("SUMMIT_OLLAMA").unwrap_or_else(|_| providers::OLLAMA.to_string())
}

/// Who can write the minutes, and who does: the page never receives a key, only whether one is
/// kept for the chosen provider.
#[derive(serde::Serialize)]
struct MinutesShown {
    ollama: Vec<String>,
    ollama_address: String,
    online: [Online; 3],
    provider: Option<String>,
    model: Option<String>,
    key_kept: bool,
}

#[tauri::command]
async fn minutes_shown(shell: State<'_, Shell>) -> Result<MinutesShown, String> {
    let settings = shell.reread().unwrap_or_else(|_| shell.settings());
    let kept = settings
        .minutes_provider
        .as_deref()
        .and_then(providers::host)
        .and_then(|host| keys::key(&host).ok().flatten())
        .is_some();
    // Ollama is asked away from the window and the tray: one that does not answer holds neither.
    let base = ollama();
    let models = tauri::async_runtime::spawn_blocking(move || providers::ollama_models(&base)).await;
    Ok(MinutesShown {
        ollama: models.unwrap_or_default(),
        ollama_address: providers::ollama_address(&ollama()),
        online: providers::ONLINE,
        provider: settings.minutes_provider,
        model: settings.minutes_model,
        key_kept: kept,
    })
}

/// Who writes the minutes, chosen: a provider's address, a model, and — for one that asks for it —
/// a key, kept in the system's keyring and nowhere else.
#[tauri::command]
async fn minutes_saved(shell: State<'_, Shell>, provider: String, model: String, key: Option<String>) -> Result<Option<String>, String> {
    // Held until the file is written, never across the keyring's wait.
    let (host, key) = {
        let _saving = shell.saving.lock().unwrap();
        // The choice goes onto what the file says now; a file that cannot be read is not written over,
        // and no key is kept for a choice that is not saved: the file first, then the key.
        let mut settings = shell.reread()?;
        let host = providers::host(provider.trim()).ok_or_else(|| format!("{provider} is no provider's address"))?;
        if model.trim().is_empty() {
            return Err("a model is named".into());
        }
        let key = key.map(|key| key.trim().to_string()).filter(|key| !key.is_empty());
        if key.is_some()
            && let Some(why) = shell.keyring.lock().unwrap().clone()
        {
            return Err(format!("the system's keyring could not be opened ({why})"));
        }
        settings.minutes_provider = Some(provider.trim().to_string());
        settings.minutes_model = Some(model.trim().to_string());
        let file = settings::file().ok_or("this system names no folder for settings")?;
        settings::write(&file, &settings).map_err(|error| error.to_string())?;
        *shell.settings.lock().unwrap() = settings;
        (host, key)
    };
    // Saved: the key now, away from the window and the tray — the keyring may ask the user first.
    // Refused, the choice stays saved and the page says only the key was not kept.
    let Some(key) = key else { return Ok(None) };
    let kept = tauri::async_runtime::spawn_blocking(move || keys::keep(&host, &key)).await;
    Ok(kept.map_err(|error| error.to_string()).and_then(|kept| kept).err())
}

/// *Tester*: asks `summit-summary` whether the model saved answers — one sentence of its own,
/// nothing of a meeting —, the key going from the keyring to its environment, never its
/// arguments. Refused, the program's own sentence.
#[tauri::command]
async fn minutes_tested(shell: State<'_, Shell>) -> Result<(), String> {
    let settings = shell.reread()?;
    let (Some(provider), Some(model)) = (settings.minutes_provider, settings.minutes_model) else {
        return Err("no model is chosen yet".into());
    };
    let summary = program("summit-summary", "SUMMIT_SUMMARY");
    let ran = tauri::async_runtime::spawn_blocking(move || {
        let key = providers::host(&provider).and_then(|host| keys::key(&host).ok().flatten());
        let mut command = std::process::Command::new(&summary);
        command.args(["--test", "--provider", &provider, "--model", &model]);
        command.env_remove("SUMMIT_PROVIDER_KEY");
        if let Some(key) = key {
            command.env("SUMMIT_PROVIDER_KEY", key);
        }
        command.output()
    })
    .await
    .map_err(|error| error.to_string())?;
    match ran {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let said = String::from_utf8_lossy(&output.stderr);
            let said = said.lines().map(str::trim).rfind(|line| !line.is_empty()).unwrap_or("");
            let said = said.strip_prefix("summit-summary: ").unwrap_or(said);
            Err(if said.is_empty() {
                format!("summit-summary stopped without saying why ({})", output.status)
            } else {
                said.to_string()
            })
        }
        Err(error) => Err(format!("summit-summary could not be started ({error})")),
    }
}

/// Why the trash refused, as the user is told it: the system's own wording names a file and
/// changes with it.
fn refusal(error: &trash::Error) -> Refusal {
    use std::io::ErrorKind;
    match error {
        trash::Error::FileSystem { source, .. } => match source.kind() {
            ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Refusal::NotAllowed,
            ErrorKind::StorageFull | ErrorKind::QuotaExceeded => Refusal::NoSpace,
            _ => Refusal::Other,
        },
        trash::Error::CouldNotAccess { .. } => Refusal::NotAllowed,
        _ => Refusal::Other,
    }
}

/// Has a session's minutes written by whoever the settings name. Nothing leaves this machine for
/// a host the user has not agreed to: the window is opened to ask, and nothing is sent until then.
fn make_minutes(app: &AppHandle, folder: PathBuf) {
    write_minutes(app, folder, None, None);
}

/// The same, `agreed_now` naming the host the user just agreed to send this meeting to, once:
/// the gate is passed only if that is still the host the settings name as it is read here.
fn write_minutes(app: &AppHandle, folder: PathBuf, agreed_now: Option<&str>, chosen: Option<(String, String)>) {
    let shell = app.state::<Shell>();
    let mut writing = shell.writing_minutes.lock().unwrap();
    if *writing {
        return;
    }
    let settings = match shell.reread() {
        Ok(settings) => settings,
        Err(why) => return tell(app, &words::trouble(&Trouble::Settings(why), shell.language)),
    };
    // Made again with another model, this time only; or by whoever the settings name.
    let (provider, model) = match chosen.or(settings.minutes_provider.zip(settings.minutes_model)) {
        Some(choice) => choice,
        None => {
            tell(app, &words::minutes(&words::Minutes::Unchosen, shell.language));
            return open_window_at(app, "#/settings");
        }
    };
    if !matches!(meeting::transcript(&folder), meeting::Read::Present(transcript) if transcript.complete) {
        return tell(app, &words::minutes(&words::Minutes::NoTranscript, shell.language));
    }
    let Some(host) = providers::host(&provider) else {
        let why = format!("{provider} is no provider's address");
        return tell(app, &words::minutes(&words::Minutes::Failed(why), shell.language));
    };
    if agreed_now != Some(host.as_str()) && !providers::local(&provider) && settings.minutes_agreed.as_deref() != Some(host.as_str()) {
        tell(app, &words::minutes(&words::Minutes::Waiting(host), shell.language));
        let name = folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return open_window_at(app, &format!("#/agree/{name}"));
    }
    *writing = true;
    drop(writing);
    minuting(app, true);
    let app = app.clone();
    // The keyring may ask the user first: never on the window's and the tray's thread.
    std::thread::spawn(move || {
        let shell = app.state::<Shell>();
        let key = keys::key(&host).ok().flatten();
        // The minutes there stay until the new ones are whole: copied aside under a name of their
        // own, they go to the trash once replaced, and the copy is dropped — the minutes still in
        // place — when the making fails.
        // Nothing there, nothing to keep; there and not set aside, nothing is made.
        let previous = match sessions::set_aside(&folder.join("minutes.md"), false) {
            Ok(previous) => Some(previous),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                *shell.writing_minutes.lock().unwrap() = false;
                minuting(&app, false);
                let why = format!("the minutes there could not be set aside ({error}): nothing was made");
                return tell(&app, &words::minutes(&words::Minutes::Failed(why), shell.language));
            }
        };
        let started = Summarising::start(
            &program("summit-summary", "SUMMIT_SUMMARY"),
            &folder,
            &provider,
            &model,
            key.as_deref(),
        );
        let events = match started {
            Ok((making, events)) => {
                *shell.minuting.lock().unwrap() = Some(making);
                events
            }
            Err(error) => {
                if let Some(previous) = &previous {
                    let _ = std::fs::remove_file(previous);
                }
                *shell.writing_minutes.lock().unwrap() = false;
                minuting(&app, false);
                return tell(&app, &words::minutes(&words::Minutes::Failed(error.to_string()), shell.language));
            }
        };
        let made = events
            .iter()
            .find_map(|event| match event {
                summarising::Event::Ended(made) => Some(made),
                summarising::Event::Sending { .. } => None,
            })
            .unwrap_or(summarising::Made::Failed(String::new()));
        shell.minuting.lock().unwrap().take();
        // Replaced, the earlier minutes go to the trash — refused, they stay beside, and it is said.
        let mut kept_beside = None;
        if let Some(previous) = previous {
            match made {
                summarising::Made::Written(_) if trash::delete(&previous).is_err() => {
                    kept_beside = previous.file_name().map(|name| name.to_string_lossy().into_owned());
                }
                summarising::Made::Written(_) => {}
                summarising::Made::Failed(_) => {
                    let _ = std::fs::remove_file(&previous);
                }
            }
        }
        *shell.writing_minutes.lock().unwrap() = false;
        minuting(&app, false);
        if *shell.leaving.lock().unwrap() {
            return;
        }
        let said = match made {
            summarising::Made::Written(_) if kept_beside.is_some() => words::Minutes::ReadyKeptBeside(kept_beside.unwrap_or_default()),
            summarising::Made::Written(_) => words::Minutes::Ready,
            summarising::Made::Failed(why) => words::Minutes::Failed(why),
        };
        tell(&app, &words::minutes(&said, shell.language));
    });
}

/// The minutes' entry while they are written, and after.
fn minuting(app: &AppHandle, now: bool) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let shell = app.state::<Shell>();
        let labels = words::menu(shell.language);
        let _ = shell.minutes.set_text(if now { labels.minuting } else { labels.minutes });
        let _ = shell.minutes.set_enabled(!now);
    });
}

/// Opens the window on one of its pages.
fn open_window_at(app: &AppHandle, page: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(format!("location.hash = {}", serde_json::to_string(page).unwrap_or_default()));
        return open_window(app);
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(format!("index.html{page}").into()))
        .title("Summit")
        .inner_size(1100.0, 800.0)
        .min_inner_size(420.0, 400.0)
        .build();
}

/// Shows a notification. A desktop that shows none is no reason to stop anything.
fn tell(app: &AppHandle, notice: &Notice) {
    let _ = app.notification().builder().title(&notice.title).body(&notice.body).show();
}

/// Puts the tray in a state: its entry's words, whether it answers, and its icon.
fn show(app: &AppHandle, shown: Shown) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let shell = app.state::<Shell>();
        let labels = words::menu(shell.language);
        let (label, enabled, icon, tooltip) = match shown {
            Shown::Idle => (labels.start, true, &include_bytes!("../icons/tray.png")[..], "Summit"),
            Shown::Recording => (
                labels.stop,
                true,
                &include_bytes!("../icons/tray-recording.png")[..],
                labels.recording,
            ),
            Shown::Stopping => (
                labels.stopping,
                false,
                &include_bytes!("../icons/tray-recording.png")[..],
                labels.recording,
            ),
        };
        let _ = shell.toggle.set_text(label);
        let _ = shell.toggle.set_enabled(enabled);
        // A transcript is asked for at rest only, and one at a time.
        let free = shown == Shown::Idle && shell.transcription.lock().unwrap().is_none();
        let _ = shell.transcribe.set_enabled(free);
        if let Some(tray) = app.tray_by_id(TRAY) {
            let _ = tray.set_icon(Image::from_bytes(icon).ok());
            let _ = tray.set_tooltip(Some(tooltip));
        }
    });
}

/// A step's program: beside this one, as an installation puts it — or, for whoever builds
/// Summit, where its variable says (`SUMMIT_CAPTURE`, `SUMMIT_TRANSCRIPTION`), or on the path.
fn program(name: &str, variable: &str) -> PathBuf {
    let name = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let beside = std::env::current_exe().ok().and_then(|own| Some(own.parent()?.join(&name)));
    std::env::var_os(variable)
        .map(PathBuf::from)
        .or_else(|| beside.filter(|path| path.is_file()))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Starts a recording when there is none, stops it when there is one: the menu's entry and
/// the global shortcut both come here.
fn toggle_recording(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let mut pressed = shell.pressed.lock().unwrap();
    if pressed.is_some_and(|last| last.elapsed() < Duration::from_millis(500)) {
        return;
    }
    *pressed = Some(Instant::now());
    let mut recording = shell.recording.lock().unwrap();
    if let Some(running) = recording.as_mut() {
        // Told once: a recording that is stopping is not told again. A stop that cannot be
        // said means the program is already ending: its end is told by what it says.
        if !std::mem::replace(&mut *shell.stopping.lock().unwrap(), true) {
            let _ = running.stop();
            show(app, Shown::Stopping);
        }
        return;
    }
    // The settings as the file says them now: the user may have changed it by hand.
    let _ = shell.reread();
    let started = std::fs::create_dir_all(&shell.settings().sessions)
        .and_then(|()| Recording::start(&program("summit-capture", "SUMMIT_CAPTURE"), &shell.settings().sessions));
    match started {
        Ok((running, said)) => {
            *recording = Some(running);
            show(app, Shown::Recording);
            let app = app.clone();
            std::thread::spawn(move || listen(&app, &said));
        }
        Err(error) => tell(app, &words::trouble(&Trouble::NoRecorder(error.to_string()), shell.language)),
    }
}

/// Tells the user what a recording says, until it has ended; then the tray is at rest again.
fn listen(app: &AppHandle, said: &Receiver<Said>) {
    let shell = app.state::<Shell>();
    let mut started = false;
    let mut stopped = None;
    for event in said {
        started |= matches!(event, Said::Started(..));
        if let Said::Started(folder, _) = &event {
            *shell.recorded.lock().unwrap() = Some(folder.clone());
        }
        let notice = match &event {
            // Ended before it had started: nothing was recorded, and that is the trouble.
            Said::Failed(why) if !started => Some(words::trouble(&Trouble::NoRecorder(why.clone()), shell.language)),
            Said::Stopped(folder, _) if shell.settings().after != settings::After::Nothing => {
                stopped = Some(folder.clone());
                words::of(&event, shell.settings().reminder, shell.language).map(|notice| Notice {
                    body: format!("{} {}", notice.body, words::follows(shell.language)),
                    ..notice
                })
            }
            _ => words::of(&event, shell.settings().reminder, shell.language),
        };
        if let Some(notice) = notice {
            tell(app, &notice);
        }
    }
    shell.recorded.lock().unwrap().take();
    let ended = shell.recording.lock().unwrap().take();
    *shell.stopping.lock().unwrap() = false;
    show(app, Shown::Idle);
    if *shell.leaving.lock().unwrap() {
        app.exit(0);
    }
    // Its output is closed: the program is ending. It is waited for here, however long it
    // takes, so that nothing of it is left in the system's table of programs.
    if let Some(ended) = ended {
        ended.wait();
    }
    // Quitting, the stop's transcript is not begun: it is made from the menu next time.
    if let Some(folder) = stopped.filter(|_| !*shell.leaving.lock().unwrap()) {
        transcribe(app, folder);
    }
}

/// Puts a session in line for its transcript, once.
fn wait(shell: &Shell, folder: PathBuf) {
    let mut waiting = shell.waiting.lock().unwrap();
    if !waiting.contains(&folder) {
        waiting.push_back(folder);
    }
}

/// *Refaire…* on a transcript: made again with the model picked, this time only. The transcript
/// there stays in place until the new one is whole — set aside for the run, since the program
/// goes on from a transcript it finds —, then goes to the system's trash; a run that fails, or
/// that Summit ends as it quits, puts it back.
fn retranscribe(app: &AppHandle, folder: PathBuf, model: PathBuf) -> Result<(), String> {
    let shell = app.state::<Shell>();
    let mut running = shell.transcription.lock().unwrap();
    if running.is_some() {
        return Err("a transcript is being made: try again once it is done".into());
    }
    let settings = shell.reread()?;
    if !settings::whisper_models(&models_folder(), &settings).contains(&model) {
        return Err(format!("{} is no model on this computer", model.display()));
    }
    // Set aside under a name of its own, never written over; put back if the new one is not made.
    let current = folder.join("transcript.jsonl");
    let previous = match sessions::set_aside(&current, true) {
        Ok(previous) => Some(previous),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("the transcript there could not be set aside ({error}): nothing was made")),
    };
    let started = Transcription::start(&program("summit-transcription", "SUMMIT_TRANSCRIPTION"), &folder, &model);
    let (transcription, made) = match started {
        Ok(started) => started,
        Err(error) => {
            if let Some(previous) = &previous {
                let _ = std::fs::rename(previous, &current);
            }
            return Err(error.to_string());
        }
    };
    *running = Some(transcription);
    // Who puts the earlier transcript back, or trashes it: this run's end, or Summit quitting —
    // whichever comes first takes it, the other finds nothing.
    *shell.redoing.lock().unwrap() = previous.clone().map(|previous| (current.clone(), previous));
    drop(running);
    transcribing(app, true);
    let app = app.clone();
    std::thread::spawn(move || {
        let made = made.recv().unwrap_or(Made::Failed(String::new()));
        let shell = app.state::<Shell>();
        shell.transcription.lock().unwrap().take();
        transcribing(&app, false);
        // Replaced, the earlier transcript goes to the trash — refused, it stays beside, and it is
        // said; not replaced, it is put back — failing that, it is said where it is.
        let mut kept_beside = None;
        if let Some((current, previous)) = shell.redoing.lock().unwrap().take() {
            let name = previous.file_name().map(|name| name.to_string_lossy().into_owned());
            match made {
                Made::Written(_) if trash::delete(&previous).is_err() => kept_beside = name,
                Made::Written(_) => {}
                Made::Failed(_) if std::fs::rename(&previous, &current).is_err() => kept_beside = name,
                Made::Failed(_) => {}
            }
        }
        if *shell.leaving.lock().unwrap() {
            return;
        }
        tell(&app, &words::made(&made, shell.language));
        if let Some(name) = kept_beside {
            tell(&app, &words::transcript_kept_beside(shell.language, &name));
        }
        // The transcripts waiting in line go on, as after any other.
        let next = shell.waiting.lock().unwrap().pop_front();
        if let Some(folder) = next {
            transcribe(&app, folder);
        }
    });
    Ok(())
}

/// Makes the transcript of a session — after the one being made, if there is one. What is
/// missing — the model, the program — is said in one sentence; the recording is never touched.
fn transcribe(app: &AppHandle, folder: PathBuf) {
    let shell = app.state::<Shell>();
    let mut running = shell.transcription.lock().unwrap();
    if running.is_some() {
        return wait(&shell, folder);
    }
    let _ = shell.reread();
    let model = &shell.settings().model;
    if !model.is_file() {
        // The default model is offered, its size and host shown; one the user named is theirs.
        if *model == Settings::default().model {
            wait(&shell, folder);
            // Already coming: the session waits for it, and nothing is offered twice.
            if !*shell.fetching.lock().unwrap() {
                offer(app);
            }
        } else {
            tell(app, &words::trouble(&Trouble::NoModel(model.display().to_string()), shell.language));
        }
        return;
    }
    let started = Transcription::start(&program("summit-transcription", "SUMMIT_TRANSCRIPTION"), &folder, model);
    let (transcription, made) = match started {
        Ok(started) => started,
        Err(error) => return tell(app, &words::trouble(&Trouble::NoTranscriber(error.to_string()), shell.language)),
    };
    *running = Some(transcription);
    transcribing(app, true);
    let app = app.clone();
    std::thread::spawn(move || {
        let made = made.recv().unwrap_or(Made::Failed(String::new()));
        let shell = app.state::<Shell>();
        shell.transcription.lock().unwrap().take();
        transcribing(&app, false);
        // Ended by Summit as it quits: nothing to tell, it goes on next time.
        if *shell.leaving.lock().unwrap() {
            return;
        }
        let notice = words::made(&made, shell.language);
        match made {
            Made::Written(folder) => {
                tell_opening(&app, &notice, folder.clone());
                // The minutes follow, when the settings say so — as they are now.
                if shell.reread().is_ok_and(|settings| settings.after == settings::After::Minutes) {
                    make_minutes(&app, folder);
                }
            }
            Made::Failed(_) => tell(&app, &notice),
        }
        let next = shell.waiting.lock().unwrap().pop_front();
        if let Some(folder) = next {
            transcribe(&app, folder);
        }
    });
}

/// The default model as published — or, for whoever tries Summit, as `SUMMIT_MODEL_PUBLISHED`
/// says: its address, its size in bytes and its SHA-256, separated by spaces.
fn published() -> Published {
    let named = std::env::var("SUMMIT_MODEL_PUBLISHED").ok().and_then(|named| {
        let [address, bytes, sha256] = named.split(' ').collect::<Vec<_>>()[..] else {
            return None;
        };
        let leaked = |text: &str| -> &'static str { Box::leak(text.to_owned().into_boxed_str()) };
        let host = address.split('/').nth(2).unwrap_or(address);
        Some(Published {
            address: leaked(address),
            host: leaked(host),
            bytes: bytes.parse().ok()?,
            sha256: leaked(sha256),
        })
    });
    named.unwrap_or(model::DEFAULT)
}

/// Puts the entry that fetches the model in the menu, and says why: nothing is fetched yet.
fn offer(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let wanted = published();
    let size = model::size(wanted.bytes);
    tell(app, &words::model_needed(shell.language, &size, wanted.host));
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let shell = app.state::<Shell>();
        let _ = shell.download.set_text(words::download(shell.language, &size, wanted.host));
        let _ = shell.download.set_enabled(true);
        if !shell
            .menu
            .items()
            .unwrap_or_default()
            .iter()
            .any(|item| item.id() == shell.download.id())
        {
            let _ = shell.menu.insert(&shell.download, 3);
        }
    });
}

/// Fetches the default model, on the user's click — their agreement. Ready, the entry leaves
/// the menu and the waiting transcript is made; cut or refused, it is said, and the entry
/// takes it up again.
fn fetch(app: &AppHandle) {
    let shell = app.state::<Shell>();
    if std::mem::replace(&mut *shell.fetching.lock().unwrap(), true) {
        return;
    }
    let (wanted, to, language) = (published(), shell.settings().model.clone(), shell.language);
    let label = |app: &AppHandle, text: String, enabled: bool| {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || {
            let shell = app.state::<Shell>();
            let _ = shell.download.set_text(text);
            let _ = shell.download.set_enabled(enabled);
        });
    };
    label(app, words::fetching(language, 0, wanted.bytes), false);
    let app = app.clone();
    std::thread::spawn(move || {
        let mut shown = String::new();
        let outcome = model::fetch(&wanted, wanted.address, &to, &mut |held| {
            let text = words::fetching(language, held, wanted.bytes);
            if text != shown {
                shown.clone_from(&text);
                label(&app, text, false);
            }
        });
        let shell = app.state::<Shell>();
        *shell.fetching.lock().unwrap() = false;
        tell(&app, &words::fetched(&outcome, language));
        if outcome.is_err() {
            return label(&app, words::download(language, &model::size(wanted.bytes), wanted.host), true);
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let shell = handle.state::<Shell>();
            let _ = shell.menu.remove(&shell.download);
        });
        let next = shell.waiting.lock().unwrap().pop_front();
        if let Some(folder) = next.or_else(|| transcription::latest(&shell.settings().sessions)) {
            transcribe(&app, folder);
        }
    });
}

/// The menu's transcript entry: its words, and whether it answers.
fn transcribing(app: &AppHandle, now: bool) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        let shell = app.state::<Shell>();
        let labels = words::menu(shell.language);
        let _ = shell.transcribe.set_text(if now { labels.transcribing } else { labels.transcribe });
        let _ = shell.transcribe.set_enabled(!now && shell.recording.lock().unwrap().is_none());
    });
}

/// Shows a notification whose click opens a folder. Under a Linux desktop the notification
/// server says when it was clicked; elsewhere it is a notification alone, for now.
#[cfg(target_os = "linux")]
fn tell_opening(_: &AppHandle, notice: &Notice, folder: PathBuf) {
    let (title, body) = (notice.title.clone(), notice.body.clone());
    std::thread::spawn(move || {
        let shown = notify_rust::Notification::new()
            .appname("Summit")
            .summary(&title)
            .body(&body)
            .action("default", &title)
            .show();
        if let Ok(shown) = shown {
            shown.wait_for_action(|action| {
                if action == "default" {
                    // Waited for, so that nothing of it is left in the system's table of programs.
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&folder)
                        .spawn()
                        .and_then(|mut opened| opened.wait());
                }
            });
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn tell_opening(app: &AppHandle, notice: &Notice, _: PathBuf) {
    tell(app, notice);
}

/// Quits Summit. A recording in progress is stopped first, as the user would stop it: its
/// files are whole before Summit is gone. Asked a second time, Summit leaves at once: a
/// recorder that does not end is not a reason to keep the user.
fn leave(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let mut recording = shell.recording.lock().unwrap();
    let asked_before = std::mem::replace(&mut *shell.leaving.lock().unwrap(), true);
    // A transcript being made is ended: run again, it goes on from where it was.
    if let Some(transcription) = shell.transcription.lock().unwrap().as_ref() {
        transcription.end();
    }
    // A transcript being made again is ended: the earlier one is put back now, before Summit goes.
    if let Some((current, previous)) = shell.redoing.lock().unwrap().take() {
        let _ = std::fs::rename(&previous, &current);
    }
    // Minutes being written are ended: nothing half-written is left, and they can be asked again.
    if let Some(making) = shell.minuting.lock().unwrap().as_ref() {
        making.end();
    }
    match recording.as_mut() {
        Some(running) if !asked_before => {
            if !std::mem::replace(&mut *shell.stopping.lock().unwrap(), true) {
                let _ = running.stop();
            }
            show(app, Shown::Stopping);
        }
        _ => app.exit(0),
    }
}

/// Binds the global shortcut that starts and stops a recording. Under a Linux desktop it is
/// the desktop's to grant, through its portal, to an installed application; elsewhere the
/// system takes it directly. Refused, it is said once: the tray's menu still does the same.
fn shortcut(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(why) = bound(&handle).await {
            // Said from the main thread: a notification waits on the desktop, which a task of
            // this runtime may not do.
            let app = handle.clone();
            let _ = handle.run_on_main_thread(move || {
                let language = app.state::<Shell>().language;
                tell(&app, &words::trouble(&Trouble::Shortcut(why), language));
            });
        }
    });
}

#[cfg(target_os = "linux")]
async fn bound(app: &AppHandle) -> Result<(), String> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    let said = |error: ashpd::Error| error.to_string();
    // The desktop answers an application it knows: one whose desktop entry is installed
    // under this very name (`linux/dev.summit.Summit.desktop`).
    let id: ashpd::AppID = "dev.summit.Summit"
        .parse()
        .map_err(|_| "the application's name was refused".to_string())?;
    ashpd::register_host_app(id).await.map_err(said)?;
    let shortcuts = GlobalShortcuts::new().await.map_err(said)?;
    let session = shortcuts.create_session(Default::default()).await.map_err(said)?;
    let labels = words::menu(app.state::<Shell>().language);
    let wanted = NewShortcut::new("record", format!("{} / {}", labels.start, labels.stop)).preferred_trigger("LOGO+ALT+m");
    let bound = shortcuts
        .bind_shortcuts(&session, &[wanted], None, Default::default())
        .await
        .map_err(said)?;
    let granted = bound.response().map_err(said)?;
    let Some(first) = granted.shortcuts().first() else {
        return Err("no key was bound".into());
    };
    *app.state::<Shell>().shortcut.lock().unwrap() = desktop::keys(first.trigger_description());
    // A press records or stops; a change the user makes in the desktop's settings is what the
    // window shows next.
    enum Heard {
        Pressed,
        Changed(Option<String>),
    }
    let pressed = shortcuts.receive_activated().await.map_err(said)?.map(|_| Heard::Pressed);
    let changed = shortcuts.receive_shortcuts_changed().await.map_err(said)?.map(|changed| {
        Heard::Changed(
            changed
                .shortcuts()
                .first()
                .and_then(|shortcut| desktop::keys(shortcut.trigger_description())),
        )
    });
    let mut heard = std::pin::pin!(futures_util::stream::select(pressed, changed));
    while let Some(event) = heard.next().await {
        match event {
            Heard::Pressed => {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || toggle_recording(&handle));
            }
            Heard::Changed(keys) => *app.state::<Shell>().shortcut.lock().unwrap() = keys,
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
async fn bound(app: &AppHandle) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

    let keys = if cfg!(target_os = "macos") {
        Modifiers::ALT | Modifiers::SUPER
    } else {
        Modifiers::CONTROL | Modifiers::ALT
    };
    let handler = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_recording(app);
            }
        })
        .build();
    app.plugin(handler).map_err(|error| error.to_string())?;
    app.global_shortcut()
        .register(Shortcut::new(Some(keys), Code::KeyM))
        .map_err(|error| error.to_string())?;
    *app.state::<Shell>().shortcut.lock().unwrap() = Some(if cfg!(target_os = "macos") { "⌥⌘M" } else { "Ctrl+Alt+M" }.to_string());
    Ok(())
}
