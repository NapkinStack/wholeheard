//! `wholeheard` — Wholeheard as the user meets it: an icon in the tray, a global shortcut,
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
    path::{Path, PathBuf},
    sync::{Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use tauri::{
    AppHandle, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, Wry,
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_notification::NotificationExt;
// A call starting is seen only on Linux, by PipeWire (D30).
#[cfg(target_os = "linux")]
use wholeheard_shell::microphone;
use wholeheard_shell::{
    autostart, context, desktop, keys, log,
    meeting::{self, Read, Transcript},
    migrate,
    minutes::{self, Minutes},
    model::{self, Published},
    player,
    providers::{self, Online},
    recording::{self, Recording, Said, Source},
    registry,
    sessions::{self, Listed},
    settings::{self, After, Engine, KeepAudio, Log, Settings},
    speech,
    summarising::{self, Summarising},
    transcription::{self, Hearer, Made, Transcription},
    words::{self, Language, Notice, Refusal, Trouble},
};

const TRAY: &str = "wholeheard";

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
    /// The user asked to quit while a recording ran: it is stopped first, then Wholeheard leaves.
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
    /// run's end, or by Wholeheard quitting, whichever comes first.
    redoing: Mutex<Option<(PathBuf, PathBuf)>>,
    /// The meeting whose transcript is being made, when one is: it cannot be freed meanwhile.
    hearing: Mutex<Option<PathBuf>>,
    /// The recording's meter, for the small window: when it started, and each source's level now.
    meter: Mutex<(Option<Instant>, [f32; 2])>,
    /// What the recording running is recorded from, for the small window (D33).
    devices: Mutex<recording::Devices>,
    /// The list's small reliefs, by transcript, kept until it changes.
    shapes: Mutex<Shapes>,
    /// The minutes being written, when they are: one at a time, taken from the gesture on.
    minuting: Mutex<Option<Summarising>>,
    /// Minutes made again (*Refaire…*) with a model, waiting for the user's agreement to send,
    /// by meeting: the model chosen, and who the settings named then — changed since, it is
    /// dropped: the page the user agrees on shows the settings as they are.
    asked_again: Mutex<HashMap<PathBuf, AskedAgain>>,
    /// The meeting whose minutes are being written: busy, never freed meanwhile (D19).
    writing_minutes: Mutex<Option<PathBuf>>,
    /// What writes them, and since when: said on the meeting's page (D25).
    minutes_by: Mutex<Option<(String, String, Instant)>>,
    /// Meetings whose minutes wait for the user's agreement to send them, and to which host: set
    /// aside, said on their page, in the order they were set aside; the line never waits behind
    /// them (D25).
    awaiting: Mutex<Vec<(PathBuf, String)>>,
    /// Minutes asked while others were written: made in turn, never dropped (D25).
    minutes_waiting: Mutex<VecDeque<Queued>>,
    /// Why the last minutes of a meeting were not written, until they are (D25).
    /// With their cause, when wholeheard-summary named one this version knows: one entry, so
    /// that a later failure never shows an earlier one's cause (D29).
    minutes_failed: Mutex<HashMap<PathBuf, (String, Option<summarising::Cause>)>>,
    /// The loopback server the opened meeting is played from, started with the first (D26).
    served: Mutex<Option<player::Served>>,
    /// How many times each meeting's minutes were written since the start: a page drawn with
    /// fewer is drawn again, however quickly they were made (D25).
    minutes_made: Mutex<HashMap<PathBuf, u64>>,
    /// The model each meeting's last minutes were asked with, when one was named (a Refaire…):
    /// tried again, with that one (D25).
    minutes_tried: Mutex<HashMap<PathBuf, Option<(String, String)>>>,
    /// The sessions whose transcripts wait for the one being made, or for the model — in
    /// the order they were asked, each once; made again as a *Refaire…* asked (D25, D59).
    waiting: Mutex<VecDeque<(PathBuf, Option<Again>)>>,
    /// The model making the transcript, and since when: said on the meeting's page (D25).
    hearing_by: Mutex<Option<(String, Instant)>>,
    /// Why a meeting's last transcript was not made, until one is begun (D25).
    transcripts_failed: Mutex<HashMap<PathBuf, String>>,
    /// How many transcripts each meeting got since the start: its page drawn with fewer is drawn
    /// again (D25).
    transcripts_made: Mutex<HashMap<PathBuf, u64>>,
    /// The same for the transcript: made again as a Refaire… asked, tried again so.
    transcripts_tried: Mutex<HashMap<PathBuf, Option<Again>>>,
    /// The tray's menu, and its entry that fetches the default model — in it only while the
    /// model is needed and not there.
    menu: Menu<Wry>,
    download: MenuItem<Wry>,
    /// The model is coming.
    fetching: Mutex<bool>,
    /// The model being fetched, by its file, and how far: the bytes held, of how many (D37).
    fetched: Mutex<Option<(String, u64, u64)>>,
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

/// How a transcript is made again: with the Whisper model a *Refaire…* picked, this time only;
/// or by whoever the settings name to hear, its minutes after it — *Refaire avec ce contexte*
/// (D59).
#[derive(Clone, PartialEq)]
enum Again {
    Model(PathBuf),
    Settings,
}

/// Where the tray is in the life of a recording.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    Idle,
    Recording,
    Stopping,
}

fn main() {
    // The logs' level the user chose in the developer mode, unless the environment names one:
    // set before anything runs beside it, so that the shell and every program it starts read it.
    if std::env::var_os("WHOLEHEARD_LOG").is_none()
        && let Some(level) = settings::file()
            .and_then(|file| settings::read(&file).ok())
            .map(|chosen| chosen.log)
            .filter(|level| *level != Log::Info)
    {
        // SAFETY: the program's first step, before any thread of it exists.
        unsafe { std::env::set_var("WHOLEHEARD_LOG", level.as_str()) };
    }
    // What the shell does is logged — never what was said — in the user's state folder (ADR-0007).
    let _run = log::start("shell");
    tracing::info!("shell.started");
    let app = tauri::Builder::default()
        // One Wholeheard per session: a second start hands its arguments over and leaves — from
        // the applications' menu (`--window`) its window opens; started again with the session,
        // nothing does (D35, #386). The plugin comes first, as its documentation asks.
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().skip(1).any(|arg| arg == "--window") {
                open_window(app);
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            listed,
            model_fetch,
            model_offered_fetch,
            registry_refresh,
            speech_shown,
            transcription_saved,
            transcription_tested,
            speech_installed,
            ollama_pull_offered,
            header,
            makings,
            record,
            recording_now,
            small_window,
            full_window,
            minutes_asked,
            minutes_again,
            busy_meetings,
            freed,
            minutes_redo,
            context_shown,
            context_saved,
            context_redo,
            transcript_redo,
            transcript_asked,
            transcript_again,
            opened,
            open_folder,
            open_logs,
            diagnostic,
            settings_shown,
            settings_saved,
            minutes_shown,
            minutes_saved,
            minutes_tested,
            agreement_shown,
            agreed,
            local_instead,
            choose_folder,
            shortcut_settings,
            page_failed
        ])
        .setup(|app| {
            let language = Language::system();
            let labels = words::menu(language);
            // Summit became Wholeheard (D28): what the user held under the old name, carried over
            // once, before anything is read; a failure leaves everything where it was, and says so.
            let carried = migrate::Places::here().map(|places| migrate::carry_over(&places));
            match &carried {
                Some(Ok(carried)) if carried.settings || carried.data => tell(app.handle(), &words::carried_over(language)),
                Some(Err(error)) => {
                    eprintln!("wholeheard: Summit's folders could not all be carried over: {error}");
                    tracing::error!(detail = %error, "shell.carry_failed");
                    tell(app.handle(), &words::carry_failed(language));
                }
                _ => {}
            }
            // Started with the user's session from now on, written once by itself (D32); a
            // settings file that cannot be read writes nothing, and is said below.
            if let (Some(entry), Some(file)) = (autostart::file(), settings::file()) {
                match autostart::first(&entry, &file) {
                    Ok(true) => tracing::info!("shell.autostart_written"),
                    Ok(false) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {}
                    Err(error) => tracing::warn!(kind = ?error.kind(), "shell.autostart_refused"),
                }
            }
            let read = settings::file().map_or_else(|| Ok(Settings::default()), |file| settings::read(&file));
            // The first run (D58): never set up, nothing recorded — the assistant, once.
            let welcome = read.as_ref().is_ok_and(settings::welcome_due);
            let toggle = MenuItem::with_id(app, "toggle", labels.start, true, None::<&str>)?;
            let again = MenuItem::with_id(app, "transcribe", labels.transcribe, true, None::<&str>)?;
            let minutes = MenuItem::with_id(app, "minutes", labels.minutes, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", labels.open, true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &again, &minutes, &quit])?;
            let download = MenuItem::with_id(app, "download", "", true, None::<&str>)?;
            TrayIconBuilder::with_id(TRAY)
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Wholeheard")
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
                    "download" => fetch(app, 0),
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
                meter: Mutex::new((None, [0.0, 0.0])),
                devices: Mutex::new(recording::Devices::default()),
                redoing: Mutex::new(None),
                hearing: Mutex::new(None),
                writing_minutes: Mutex::new(None),
                minutes_by: Mutex::new(None),
                minutes_waiting: Mutex::new(VecDeque::new()),
                awaiting: Mutex::new(Vec::new()),
                minutes_failed: Mutex::new(HashMap::new()),
                served: Mutex::new(None),
                minutes_made: Mutex::new(HashMap::new()),
                minutes_tried: Mutex::new(HashMap::new()),
                asked_again: Mutex::new(HashMap::new()),
                waiting: Mutex::new(VecDeque::new()),
                hearing_by: Mutex::new(None),
                transcripts_failed: Mutex::new(HashMap::new()),
                transcripts_made: Mutex::new(HashMap::new()),
                transcripts_tried: Mutex::new(HashMap::new()),
                menu,
                download,
                fetching: Mutex::new(false),
                fetched: Mutex::new(None),
            });
            if let Err(why) = read {
                tell(app.handle(), &words::trouble(&Trouble::Settings(why), language));
            }
            shortcut(app.handle());
            offers(app.handle());
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
            if welcome {
                show_welcome(app.handle());
            } else if std::env::args().skip(1).any(|arg| arg == "--window") {
                open_window(app.handle());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Wholeheard could not start");
    // No window is no reason to leave: Wholeheard lives in the tray until the user quits it.
    app.run(|_, event| {
        if let RunEvent::ExitRequested { api, code: None, .. } = event {
            api.prevent_exit();
        }
    });
}

/// Another application taking a microphone — a call starting — is offered to be recorded, by a
/// notification naming it: never a window (D17), never while recording, never with offers off;
/// what was seen is shown, and kept nowhere (D30).
#[cfg(target_os = "linux")]
fn offers(app: &AppHandle) {
    let app = app.clone();
    let offers = Mutex::new(microphone::Offers::default());
    microphone::watch(move |change| {
        let shell = app.state::<Shell>();
        let recording = shell.recording.lock().unwrap().is_some();
        if let Some(name) = offers.lock().unwrap().on(&change, recording, shell.settings().offer) {
            offer_recording(&app, name);
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn offers(_: &AppHandle) {}

#[cfg(target_os = "linux")]
fn offer_recording(app: &AppHandle, application: String) {
    let (notice, [record, later, never]) = words::offer(&application, app.state::<Shell>().language);
    let app = app.clone();
    // Logged without the application: what it was is the notification's alone.
    tracing::info!("shell.offer_made");
    std::thread::spawn(move || {
        let shown = notify_rust::Notification::new()
            .appname("Wholeheard")
            .summary(&notice.title)
            .body(&notice.body)
            .action("record", record)
            .action("later", later)
            .action("never", never)
            .show();
        let Ok(shown) = shown else { return };
        shown.wait_for_action(|action| match action {
            "record" => {
                tracing::info!("shell.offer_taken");
                // Started meanwhile from the menu: left as it is.
                if app.state::<Shell>().recording.lock().unwrap().is_none() {
                    toggle_recording(&app);
                }
            }
            "never" => {
                tracing::info!("shell.offer_off");
                let shell = app.state::<Shell>();
                let _saving = shell.saving.lock().unwrap();
                let saved = shell.reread().and_then(|mut settings| {
                    settings.offer = false;
                    let file = settings::file().ok_or("this system names no folder for settings")?;
                    settings::write(&file, &settings).map_err(|error| error.to_string())?;
                    *shell.settings.lock().unwrap() = settings;
                    Ok(())
                });
                if let Err(error) = saved {
                    tracing::error!(detail = %error, "shell.offer_off_refused");
                }
            }
            _ => tracing::info!("shell.offer_declined"),
        });
    });
}

/// Opens the window the user last left — the small one the first time — or brings it forward.
/// Only ever on a gesture of the user (D35).
fn open_window(app: &AppHandle) {
    // A settings file that cannot be read is said in the full window, which the small one has no
    // room for: it opens there.
    match app.state::<Shell>().reread().map(|settings| settings.window) {
        Ok(settings::Window::Small) => show_small(app),
        Ok(settings::Window::Full) | Err(_) => show_full(app),
    }
}

/// The full window, in place of the small one: one window at a time (D35).
fn show_full(app: &AppHandle) {
    left(app, settings::Window::Full);
    if let Some(small) = app.get_webview_window("small") {
        let _ = small.close();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    build_full(app, "");
}

/// The full window made, on one of its pages.
fn build_full(app: &AppHandle, page: &str) {
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(format!("index.html{page}").into()))
        .title("Wholeheard")
        .inner_size(1100.0, 800.0)
        .min_inner_size(420.0, 400.0)
        .build();
}

/// The first start (D58): the full window opens on the assistant — a first launch is the user's
/// gesture —, and it is said shown at once, so that it never opens by itself again; closed
/// midway, the defaults stay, already private. The window left is not changed: the small one
/// stays the one reopened (D35).
fn show_welcome(app: &AppHandle) {
    let shell = app.state::<Shell>();
    {
        let _saving = shell.saving.lock().unwrap();
        let written = shell.reread().and_then(|mut settings| {
            settings.welcomed = true;
            let file = settings::file().ok_or("this system names no folder for settings")?;
            settings::write(&file, &settings).map_err(|error| error.to_string())?;
            *shell.settings.lock().unwrap() = settings;
            Ok(())
        });
        if let Err(error) = written {
            tracing::warn!(detail = %error, "shell.welcomed_not_kept");
        }
    }
    tracing::info!("shell.welcome_shown");
    build_full(app, "#/welcome");
}

/// The small window's size, the same at rest and recording: GTK does not shrink a window the
/// user cannot resize, and a window that changes size under the pointer is one more surprise (D35).
const SMALL: (f64, f64) = (380.0, 400.0);

/// The small window, in place of the full one, which is hidden: at rest, or showing the
/// recording above the other windows (D35, board 6).
fn show_small(app: &AppHandle) {
    left(app, settings::Window::Small);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    if let Some(window) = app.get_webview_window("small") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let recording = app.state::<Shell>().recording.lock().unwrap().is_some();
    let _ = WebviewWindowBuilder::new(app, "small", WebviewUrl::App("index.html#/small".into()))
        .title("Wholeheard")
        .inner_size(SMALL.0, SMALL.1)
        .resizable(false)
        .always_on_top(recording)
        .build();
}

/// A recording begins or ends: the small window, when open, is above the others while it runs
/// only (D35).
fn small_recording(app: &AppHandle, recording: bool) {
    if let Some(small) = app.get_webview_window("small") {
        let _ = small.set_always_on_top(recording);
    }
}

/// Keeps the window left in the settings, for the next opening — on a thread of its own: a window
/// opens from paths that may hold the settings' locks, and never waits for them. A file that
/// cannot be read is said in the full window, and kept as it is; one that cannot be written
/// keeps what it says: the window opens all the same.
fn left(app: &AppHandle, window: settings::Window) {
    let app = app.clone();
    std::thread::spawn(move || {
        let shell = app.state::<Shell>();
        let _saving = shell.saving.lock().unwrap();
        let Ok(mut settings) = shell.reread() else {
            return;
        };
        if settings.window == window {
            return;
        }
        settings.window = window;
        let written = settings::file()
            .ok_or_else(|| "this system names no folder for settings".to_string())
            .and_then(|file| settings::write(&file, &settings).map_err(|error| error.to_string()));
        match written {
            Ok(()) => *shell.settings.lock().unwrap() = settings,
            Err(error) => tracing::warn!(detail = %error, "shell.window_not_kept"),
        }
    });
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

/// *Try again*, on a meeting whose minutes were not made: asked as they were last, with the model
/// a Refaire… named then.
#[tauri::command]
fn minutes_again(app: AppHandle, shell: State<'_, Shell>, name: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    let chosen = shell.minutes_tried.lock().unwrap().get(&folder).cloned().flatten();
    write_minutes(&app, folder, None, chosen);
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
    // The settings' model, as they name it now, when none is chosen: still a redo, which waits
    // its turn behind the meeting's own making — never taken for a second first one.
    let chosen = match provider.zip(model) {
        Some(choice) => Some(choice),
        None => {
            let settings = shell.reread()?;
            settings.minutes_provider.zip(settings.minutes_model)
        }
    };
    write_minutes(&app, folder, None, chosen);
    Ok(())
}

#[tauri::command]
fn transcript_redo(app: AppHandle, shell: State<'_, Shell>, name: String, model: PathBuf) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    retranscribe(&app, folder, Again::Model(model))
}

/// *Try again*, on a meeting whose transcript was not made: as it was last asked — made again
/// with the model a Refaire… picked, the transcript there kept until the new one is whole.
#[tauri::command]
fn transcript_again(app: AppHandle, shell: State<'_, Shell>, name: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    // Read, and the lock let go, before it is asked again: retranscribe takes it too.
    let tried = shell.transcripts_tried.lock().unwrap().get(&folder).cloned().flatten();
    match tried {
        Some(again) => retranscribe_or_say(&app, folder, again),
        None => transcribe(&app, folder),
    }
    Ok(())
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

/// The meetings that cannot be freed now: the one being recorded, the one being transcribed.
fn busy(shell: &Shell) -> Vec<PathBuf> {
    let recording = shell.recording.lock().unwrap().is_some();
    let recorded = shell.recorded.lock().unwrap().clone().filter(|_| recording);
    recorded
        .into_iter()
        .chain(shell.hearing.lock().unwrap().clone())
        .chain(shell.writing_minutes.lock().unwrap().clone())
        .collect()
}

/// Their names, for the page that frees space (D19).
#[tauri::command]
fn busy_meetings(shell: State<'_, Shell>) -> Vec<String> {
    busy(&shell)
        .iter()
        .filter_map(|folder| folder.file_name().map(|name| name.to_string_lossy().into_owned()))
        .collect()
}

/// Frees space on the user's word (D19): the meetings' audio alone — their transcript and minutes
/// still read —, or the whole meetings; to the system's trash, never deleted, restored from there.
/// A meeting being recorded, transcribed or its minutes written is refused.
#[tauri::command]
fn freed(shell: State<'_, Shell>, names: Vec<String>, whole: bool) -> Result<(), String> {
    let busy = busy(&shell);
    let mut gone = Vec::new();
    for name in &names {
        let folder = session_folder(&shell, name)?;
        if busy.contains(&folder) {
            return Err(format!("{name} is being recorded, transcribed or its minutes written"));
        }
        if whole {
            gone.push(folder);
        } else {
            gone.extend(sessions::audio_of(&folder));
        }
    }
    if gone.is_empty() {
        return Ok(());
    }
    trash::delete_all(&gone).map_err(|error| refusal_said(&error, shell.language))
}

/// A trash's refusal, in the user's words (as the audio's expiry says it).
fn refusal_said(error: &trash::Error, language: Language) -> String {
    words::audio_trashed(language, &Err(refusal(error))).body
}

/// What the small window shows (D22), asked ten times a second: whether a recording runs, for
/// how long, and each source's level now.
#[derive(serde::Serialize)]
struct Now {
    recording: bool,
    seconds: f64,
    levels: [f32; 2],
    /// The microphone and the output recorded now, as the system names them (D33).
    devices: [Option<String>; 2],
    /// Whether the transcript is made at the stop, by itself: the small window says so then only.
    transcript_follows: bool,
    /// The meeting being recorded, once its folder holds its session: its context can be written
    /// (D59).
    meeting: Option<String>,
}

#[tauri::command]
fn recording_now(shell: State<'_, Shell>) -> Now {
    let (started, levels) = *shell.meter.lock().unwrap();
    let recording = shell.recording.lock().unwrap().is_some();
    let meeting = shell
        .recorded
        .lock()
        .unwrap()
        .clone()
        .filter(|folder| recording && folder.join("session.json").is_file())
        .map(|folder| named(&folder));
    Now {
        recording: shell.recording.lock().unwrap().is_some() && started.is_some(),
        seconds: started.map_or(0.0, |started| started.elapsed().as_secs_f64()),
        levels,
        devices: shell.devices.lock().unwrap().names(),
        transcript_follows: shell.settings.lock().unwrap().after != settings::After::Nothing,
        meeting,
    }
}

/// From the full window to the small one, at rest or recording (D35). Asynchronous, as is
/// `full_window`: each hides or closes the window that asked, which its own call, run where the
/// windows are, would wait for.
#[tauri::command]
async fn small_window(app: AppHandle) {
    show_small(&app);
}

/// From the small window to the full one, on one of its pages when named: a meeting, the
/// list, the settings (D35).
#[tauri::command]
async fn full_window(app: AppHandle, page: Option<String>) {
    match page.filter(|page| page.starts_with("#/")) {
        Some(page) => open_window_at(&app, &page),
        None => show_full(&app),
    }
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
    /// The developer mode is on: the page offers a diagnostic of this meeting (D29).
    developer: bool,
    /// Where the page plays it from: its tracks as one sound, from the loopback server — never
    /// a path (D26, ADR-0009); none when its audio went to the trash or holds no track.
    listen: Option<String>,
    context: ContextShown,
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
        minutes: minutes::minutes(&folder, words::window(shell.language).minutes_sections),
        developer: shell.settings().developer,
        listen: playable(&shell, &folder),
        context: context_of(&shell, &folder),
    })
}

/// A meeting's context, as the field shows it (D59).
#[derive(serde::Serialize)]
struct ContextShown {
    text: String,
    /// Its transcript or minutes were made with another: *Refaire avec ce contexte* is offered.
    changed: bool,
}

fn context_of(shell: &Shell, folder: &Path) -> ContextShown {
    let text = context::read(folder);
    let transcript = meeting::transcript(folder);
    let minutes = minutes::minutes(folder, words::window(shell.language).minutes_sections);
    ContextShown {
        changed: context::changed(text.as_deref(), &transcript, &minutes),
        text: text.unwrap_or_default(),
    }
}

/// The meeting's transcript or minutes are being made: their making read its context as it began.
fn making(shell: &Shell, folder: &Path) -> bool {
    shell.hearing.lock().unwrap().as_deref() == Some(folder) || shell.writing_minutes.lock().unwrap().as_deref() == Some(folder)
}

#[tauri::command]
fn context_shown(shell: State<'_, Shell>, name: String) -> Result<ContextShown, String> {
    let folder = session_folder(&shell, &name)?;
    Ok(context_of(&shell, &folder))
}

/// The field written (D59): kept whole as the meeting's `context.txt` — the meeting being
/// recorded once its folder holds its session —; refused while a making of it runs, or its
/// transcript is left unfinished.
#[tauri::command]
fn context_saved(shell: State<'_, Shell>, name: String, text: String) -> Result<ContextShown, String> {
    let folder = session_folder(&shell, &name)?;
    if let Some(closed) = context::closed(making(&shell, &folder), &meeting::transcript(&folder)) {
        return Err(format!("the context cannot change now: {closed:?}"));
    }
    context::write(&folder, &text).map_err(|error| error.to_string())?;
    Ok(context_of(&shell, &folder))
}

/// *Refaire avec ce contexte* (D59): the transcript made again by whoever the settings name to
/// hear, then the minutes by whoever they name to write — each kept in place until the new one
/// is whole, put back if it is not.
#[tauri::command]
fn context_redo(app: AppHandle, shell: State<'_, Shell>, name: String) -> Result<(), String> {
    let folder = session_folder(&shell, &name)?;
    if shell.recording.lock().unwrap().is_some() && shell.recorded.lock().unwrap().as_deref() == Some(folder.as_path()) {
        return Err(format!("{name} is being recorded"));
    }
    if context::closed(making(&shell, &folder), &meeting::transcript(&folder)).is_some() {
        return Err(format!("{name} is being made, or its transcript is unfinished"));
    }
    // One that cannot begin — no model, the program missing — says why on its page.
    retranscribe_or_say(&app, folder, Again::Settings);
    Ok(())
}

/// A meeting's tracks, offered to the window's player by the loopback server as one sound — these
/// files, read only, and no other: an earlier meeting's are no longer served (ADR-0008, ADR-0009).
fn playable(shell: &Shell, folder: &Path) -> Option<String> {
    let tracks = meeting::tracks(folder);
    let mut served = shell.served.lock().unwrap();
    if served.is_none() && !tracks.is_empty() {
        match player::Served::start() {
            Ok(started) => *served = Some(started),
            Err(error) => tracing::error!(detail = %error, "shell.player_refused"),
        }
    }
    served
        .as_ref()?
        .offer(tracks.into_iter().map(|track| (track.path, track.starts)).collect())
}

/// A meeting's diagnostic, for the page to copy: every program's log lines of its makings —
/// nothing of the meeting, which no line holds (D29).
#[tauri::command]
fn diagnostic(shell: State<'_, Shell>, name: String) -> Result<String, String> {
    if !shell.settings().developer {
        return Err("the developer mode is off".into());
    }
    session_folder(&shell, &name)?;
    let folder = log::folder().ok_or("this system names no folder for logs")?;
    Ok(log::diagnostic(&folder, &name))
}

/// A failure of the window's page, as it reports its own (D29): an error, its kind and where in
/// the page's code — never its sentence ([`log::page_failed`]).
#[tauri::command]
fn page_failed(kind: String, file: String, line: u32, column: u32) {
    log::page_failed(&kind, &file, line, column);
}

/// Opens the logs' folder in the system's file manager, made when nothing was logged yet.
#[tauri::command]
fn open_logs() -> Result<(), String> {
    let folder = log::folder().ok_or("this system names no folder for logs")?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    opening(&folder)
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
    let read = shell.reread()?;
    let named = read.minutes_provider.clone().zip(read.minutes_model.clone());
    let now = read.minutes_provider.as_deref().and_then(providers::host);
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
    let again = shell
        .asked_again
        .lock()
        .unwrap()
        .remove(&folder)
        .filter(|asked| asked.named == named)
        .map(|asked| asked.choice);
    write_minutes(&app, folder, Some(&host), again);
    look_again(&app);
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
    look_again(&app);
    Ok(())
}

/// Opens a session's folder in the system's file manager.
#[tauri::command]
fn open_folder(shell: State<'_, Shell>, name: String) -> Result<(), String> {
    opening(&session_folder(&shell, &name)?)
}

/// Opens a folder in the system's file manager.
fn opening(folder: &Path) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    // Waited for on a thread of its own, so that nothing of it is left in the system's table.
    let mut started = std::process::Command::new(opener)
        .arg(folder)
        .spawn()
        .map_err(|error| error.to_string())?;
    std::thread::spawn(move || started.wait());
    Ok(())
}

/// One of the models to download: its file, its size, its host, what it is good for — none when
/// this build has no sentence for it —, whether it is here.
#[derive(serde::Serialize)]
struct Offered {
    file: &'static str,
    bytes: u64,
    host: &'static str,
    good: Option<&'static str>,
    here: bool,
}

/// The models Settings offer: the list shipped, or the one read again on the user's gesture
/// until Wholeheard quits (D40, ADR-0010).
fn offered() -> Vec<registry::Listed> {
    offered_list().read().unwrap().clone()
}

fn offered_list() -> &'static std::sync::RwLock<Vec<registry::Listed>> {
    static LIST: std::sync::OnceLock<std::sync::RwLock<Vec<registry::Listed>>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| std::sync::RwLock::new(registry::shipped()))
}

/// Where the list is read again from — or, for whoever tries Wholeheard, `WHOLEHEARD_REGISTRY`.
fn registry_address() -> String {
    std::env::var("WHOLEHEARD_REGISTRY").unwrap_or_else(|_| registry::ADDRESS.to_owned())
}

/// Reads the list again, on the user's click in Settings — its host shown on the button. Taken,
/// it is offered until Wholeheard quits; refused, the list offered stays, and the page says so.
#[tauri::command]
async fn registry_refresh() -> Result<(), ()> {
    let address = registry_address();
    let host = registry::host(&address).to_owned();
    let read = tauri::async_runtime::spawn_blocking(move || registry::fetch(&address)).await;
    match read.unwrap_or_else(|error| Err(error.to_string())) {
        Ok(listed) => {
            tracing::info!(host = %host, models = listed.len(), "shell.registry_read");
            *offered_list().write().unwrap() = listed;
            Ok(())
        }
        Err(detail) => {
            tracing::warn!(host = %host, detail = %detail, "shell.registry_refused");
            Err(())
        }
    }
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
    /// A call another application starts is offered to be recorded (D30).
    offer: bool,
    /// Wholeheard starts with the user's session (D32): its entry, as the desktop reads it.
    autostart: bool,
    keep_audio: KeepAudio,
    model: PathBuf,
    /// Who hears: the model above, Apple's SpeechAnalyzer (D42), or a service the user named (D43),
    /// its address, model and the host agreed to; its key never: the keyring holds it.
    engine: Engine,
    transcription_provider: Option<String>,
    transcription_model: Option<String>,
    transcription_agreed: Option<String>,
    models: Vec<Model>,
    /// The short list of models to download, each said by what it is good for (D37), and the
    /// host it is read again from on the user's gesture (D40).
    offered: Vec<Offered>,
    registry_host: String,
    developer: bool,
    log: Log,
    unreadable: Option<String>,
}

/// The folder Wholeheard keeps its models in: the default model's.
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
            // One outside Wholeheard's folder says where it is: two files may share a name.
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
        offer: settings.offer,
        autostart: autostart::file().is_some_and(|entry| autostart::on(&entry)),
        keep_audio: settings.keep_audio,
        model: settings.model,
        engine: settings.engine,
        transcription_provider: settings.transcription_provider.clone(),
        transcription_model: settings.transcription_model.clone(),
        transcription_agreed: settings.transcription_agreed.clone(),
        models,
        registry_host: registry::host(&registry_address()).to_owned(),
        offered: offered()
            .iter()
            .enumerate()
            .map(|(index, listed)| Offered {
                file: model::file(&listed.published),
                bytes: published_at(index).bytes,
                host: published_at(index).host,
                good: listed.good.map(|good| words::offered_good(shell.language, good)),
                here: models_folder().join(model::file(&listed.published)).is_file(),
            })
            .collect(),
        developer: settings.developer,
        log: settings.log,
        unreadable,
    }
}

/// One choice the page changed.
#[derive(serde::Deserialize)]
struct Change {
    sessions: Option<PathBuf>,
    after: Option<After>,
    reminder: Option<bool>,
    offer: Option<bool>,
    autostart: Option<bool>,
    keep_audio: Option<KeepAudio>,
    model: Option<PathBuf>,
    /// Apple's SpeechAnalyzer, or Whisper again (D42); a model chosen is Whisper's.
    engine: Option<Engine>,
    developer: Option<bool>,
    log: Option<Log>,
}

/// Writes a change to the settings file, then holds it: the next recording follows it. Nothing
/// is written over a file that could not be read, and a model is only one found on this computer.
#[tauri::command]
fn settings_saved(app: AppHandle, shell: State<'_, Shell>, change: Change) -> Result<(), String> {
    let apple = change.engine == Some(Engine::SpeechAnalyzer);
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
    if let Some(offer) = change.offer {
        settings.offer = offer;
    }
    // The entry first: the settings say what Wholeheard wrote of it, once it is written.
    if let Some(on) = change.autostart {
        let entry = autostart::file().ok_or("this system names no folder for the session's start")?;
        autostart::set(&entry, on).map_err(|error| format!("{}: {error}", entry.display()))?;
        settings.autostart = Some(on);
    }
    if let Some(developer) = change.developer {
        settings.developer = developer;
    }
    if let Some(log) = change.log {
        settings.log = log;
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
        settings.engine = Engine::Whisper;
    }
    if let Some(engine) = change.engine {
        settings.engine = engine;
    }
    let file = settings::file().ok_or("this system names no folder for settings")?;
    settings::write(&file, &settings).map_err(|error| error.to_string())?;
    *shell.settings.lock().unwrap() = settings;
    // Meetings that waited for the default Whisper model are heard now by SpeechAnalyzer (D42).
    if apple {
        transcribe_next(&app);
    }
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
    let program = keyboard_settings().ok_or("this desktop's keyboard settings cannot be opened from Wholeheard")?;
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

/// Ollama on this computer — or, for whoever tries Wholeheard, where `WHOLEHEARD_OLLAMA` says.
fn ollama() -> String {
    std::env::var("WHOLEHEARD_OLLAMA").unwrap_or_else(|_| providers::OLLAMA.to_string())
}

/// Who can write the minutes, and who does: the page never receives a key, only whether one is
/// kept for the chosen provider.
#[derive(serde::Serialize)]
struct MinutesShown {
    ollama: Vec<String>,
    /// Whether Ollama answers here: one with no model is not one missing (D37).
    ollama_answers: bool,
    ollama_address: String,
    /// The local model recommended for the minutes, said so when Ollama holds it (#261).
    recommended: &'static str,
    online: [Online; 3],
    provider: Option<String>,
    model: Option<String>,
    key_kept: bool,
    /// Whether the address saved is this machine — read as the agreement reads it: what is sent
    /// there never leaves it, and the page says so whatever the model.
    provider_local: bool,
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
    let models = tauri::async_runtime::spawn_blocking(move || providers::ollama_listed(&base))
        .await
        .ok()
        .flatten();
    let provider_local = settings.minutes_provider.as_deref().is_some_and(providers::local);
    Ok(MinutesShown {
        provider_local,
        ollama_answers: models.is_some(),
        ollama: models.unwrap_or_default(),
        ollama_address: providers::ollama_address(&ollama()),
        recommended: providers::RECOMMENDED,
        online: providers::ONLINE,
        provider: settings.minutes_provider,
        model: settings.minutes_model,
        key_kept: kept,
    })
}

/// Who writes the minutes, chosen: a provider's address, a model, and — for one that asks for it —
/// a key, kept in the system's keyring and nowhere else.
#[tauri::command]
async fn minutes_saved(
    app: AppHandle,
    shell: State<'_, Shell>,
    provider: String,
    model: String,
    key: Option<String>,
) -> Result<Option<String>, String> {
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
    let not_kept = match key {
        Some(key) => {
            let kept = tauri::async_runtime::spawn_blocking(move || keys::keep(&host, &key)).await;
            kept.map_err(|error| error.to_string()).and_then(|kept| kept).err()
        }
        None => None,
    };
    look_again(&app);
    Ok(not_kept)
}

/// Who hears, when a service does (D43): its address and model saved, the user's agreement to its
/// host with them, its key kept in the keyring under the transcription's own account. A host on
/// this computer needs no agreement. Nothing is sent here.
#[tauri::command]
async fn transcription_saved(
    shell: State<'_, Shell>,
    provider: String,
    model: String,
    key: Option<String>,
    agreed: bool,
) -> Result<Option<String>, String> {
    let (host, key) = {
        let _saving = shell.saving.lock().unwrap();
        let mut settings = shell.reread()?;
        let (host, agreed_to) = transcription::service_named(&provider, &model, agreed).map_err(|refused| match refused {
            transcription::Refused::Address => format!("{provider} is no service's address"),
            transcription::Refused::Model => "a model is named".to_string(),
            transcription::Refused::NotAgreed(host) => format!("the meetings' sound is sent to {host}: agree to it first"),
        })?;
        let key = key.map(|key| key.trim().to_string()).filter(|key| !key.is_empty());
        if key.is_some()
            && let Some(why) = shell.keyring.lock().unwrap().clone()
        {
            return Err(format!("the system's keyring could not be opened ({why})"));
        }
        settings.engine = Engine::Service;
        settings.transcription_provider = Some(provider.trim().to_string());
        settings.transcription_model = Some(model.trim().to_string());
        settings.transcription_agreed = agreed_to;
        let file = settings::file().ok_or("this system names no folder for settings")?;
        settings::write(&file, &settings).map_err(|error| error.to_string())?;
        *shell.settings.lock().unwrap() = settings;
        tracing::info!(host = %host, "shell.transcription_service_saved");
        (host, key)
    };
    let not_kept = match key {
        Some(key) => {
            let account = transcription::key_account(&host);
            let kept = tauri::async_runtime::spawn_blocking(move || keys::keep(&account, &key)).await;
            kept.map_err(|error| error.to_string()).and_then(|kept| kept).err()
        }
        None => None,
    };
    Ok(not_kept)
}

/// *Tester* for who hears: `wholeheard-transcription --test` sends a sound of its own to the service
/// saved, its key in the environment; refused, the program's own sentence.
#[tauri::command]
async fn transcription_tested(shell: State<'_, Shell>) -> Result<(), String> {
    let settings = shell.reread()?;
    let (Some(provider), Some(model)) = (settings.transcription_provider, settings.transcription_model) else {
        return Err("no service is chosen yet".into());
    };
    let hearing = program("wholeheard-transcription", "WHOLEHEARD_TRANSCRIPTION");
    let output = tauri::async_runtime::spawn_blocking(move || {
        let host = providers::host(&provider).unwrap_or_default();
        let key = keys::key(&transcription::key_account(&host)).ok().flatten();
        tracing::info!(host = %host, model = %model, key = key.is_some(), "shell.transcription_testing");
        let mut command = std::process::Command::new(&hearing);
        command.args(["--test", "--provider", &provider, "--model", &model]);
        command.env_remove("WHOLEHEARD_PROVIDER_KEY");
        if let Some(key) = key {
            command.env("WHOLEHEARD_PROVIDER_KEY", key);
        }
        command.output()
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        tracing::info!("shell.transcription_tested");
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let said = said.lines().map(str::trim).rfind(|line| !line.is_empty()).unwrap_or("");
    let said = said.strip_prefix("wholeheard-transcription: ").unwrap_or(said).to_string();
    tracing::warn!(detail = %said, "shell.transcription_test_failed");
    Err(if said.is_empty() {
        format!("wholeheard-transcription stopped without saying why ({})", output.status)
    } else {
        said
    })
}

/// *Tester*: asks `wholeheard-summary` whether the model saved answers — one sentence of its own,
/// nothing of a meeting —, the key going from the keyring to its environment, never its
/// arguments. Refused, the program's own sentence.
#[tauri::command]
async fn minutes_tested(shell: State<'_, Shell>) -> Result<(), String> {
    let settings = shell.reread()?;
    let (Some(provider), Some(model)) = (settings.minutes_provider, settings.minutes_model) else {
        return Err("no model is chosen yet".into());
    };
    let summary = program("wholeheard-summary", "WHOLEHEARD_SUMMARY");
    let ran = tauri::async_runtime::spawn_blocking(move || {
        let key = providers::host(&provider).and_then(|host| keys::key(&host).ok().flatten());
        let (trace, making) = log::making();
        let _making = making.enter();
        tracing::info!(
            host = providers::host(&provider).unwrap_or_default(),
            model,
            key = key.is_some(),
            "shell.testing"
        );
        let mut command = std::process::Command::new(&summary);
        command
            .args(["--test", "--provider", &provider, "--model", &model])
            .env("WHOLEHEARD_TRACE", &trace);
        command.env_remove("WHOLEHEARD_PROVIDER_KEY");
        if let Some(key) = key {
            command.env("WHOLEHEARD_PROVIDER_KEY", key);
        }
        let output = command.output();
        match &output {
            Ok(output) if output.status.success() => tracing::info!("shell.tested"),
            Ok(output) => {
                tracing::error!(detail = %String::from_utf8_lossy(&output.stderr).trim(), status = ?output.status, "shell.test_failed")
            }
            Err(error) => tracing::error!(detail = %error, "shell.test_failed"),
        }
        output
    })
    .await
    .map_err(|error| error.to_string())?;
    match ran {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let said = String::from_utf8_lossy(&output.stderr);
            let said = said.lines().map(str::trim).rfind(|line| !line.is_empty()).unwrap_or("");
            let said = said.strip_prefix("wholeheard-summary: ").unwrap_or(said);
            Err(if said.is_empty() {
                format!("wholeheard-summary stopped without saying why ({})", output.status)
            } else {
                said.to_string()
            })
        }
        Err(error) => Err(format!("wholeheard-summary could not be started ({error})")),
    }
}

/// Why the trash refused, as the user is told it: the system's own wording names a file and
/// changes with it.
fn refusal(error: &trash::Error) -> Refusal {
    #[cfg(target_os = "linux")]
    use std::io::ErrorKind;
    match error {
        // The freedesktop trash's own error: the other systems' trash says no such thing.
        #[cfg(target_os = "linux")]
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

/// Minutes asked while others were being written, as they were asked.
struct Queued {
    folder: PathBuf,
    agreed: Option<String>,
    chosen: Option<(String, String)>,
}

/// A making of minutes is over: its meeting free, its failure kept for its page, and the next
/// one asked meanwhile started — those that cannot start yet (an agreement to give) left as said.
fn minutes_ended(app: &AppHandle, folder: &Path, failed: Option<(String, Option<summarising::Cause>)>) {
    let shell = app.state::<Shell>();
    // What says it is being written goes first, the meeting freed last: nothing started in
    // between is left without its model, nor the tray's entry greyed off.
    shell.minutes_by.lock().unwrap().take();
    if let Some((why, cause)) = failed {
        let why = if why.trim().is_empty() {
            "wholeheard-summary stopped without saying why".to_string()
        } else {
            why
        };
        shell.minutes_failed.lock().unwrap().insert(folder.to_path_buf(), (why, cause));
    } else {
        *shell.minutes_made.lock().unwrap().entry(folder.to_path_buf()).or_default() += 1;
    }
    minuting(app, false);
    *shell.writing_minutes.lock().unwrap() = None;
    if *shell.leaving.lock().unwrap() {
        return;
    }
    loop {
        let Some(next) = shell.minutes_waiting.lock().unwrap().pop_front() else {
            return;
        };
        // Started, or set aside (an agreement to give, a reason it cannot): the next one then.
        write_minutes_from(app, next.folder, next.agreed.as_deref(), next.chosen, true);
        if shell.writing_minutes.lock().unwrap().is_some() {
            return;
        }
    }
}

/// What is being made in the background, for the pages (D25): the minutes being written, by
/// what and for how long, those waiting their turn, the transcript being made and those waiting,
/// and why a meeting's last minutes were not written.
#[derive(serde::Serialize)]
struct Makings {
    minutes: Option<Making>,
    minutes_waiting: Vec<String>,
    transcript: Option<Hearing>,
    transcripts_waiting: Vec<String>,
    /// The line waits for the default Whisper model, offered in the menu, or coming.
    model_needed: bool,
    model_coming: bool,
    /// The window's offer of it: its size and host, the size said as the window says sizes (D36).
    model_offer_bytes: u64,
    model_offer_host: &'static str,
    /// Any model being fetched, and how far (D37).
    model_fetching: Option<Fetching>,
    /// A making not made, by its meeting's name: what its program said of it in developer mode,
    /// else nothing — the page says it in plain words (D29).
    transcripts_failed: HashMap<String, String>,
    transcripts_made: HashMap<String, u64>,
    failed: HashMap<String, String>,
    /// The minutes' cause and its gesture, in the user's language, when one was named (D29).
    failed_said: HashMap<String, String>,
    awaiting: HashMap<String, String>,
    made: HashMap<String, u64>,
}

#[derive(serde::Serialize)]
struct Fetching {
    file: String,
    held: u64,
    bytes: u64,
}

#[derive(serde::Serialize)]
struct Hearing {
    name: String,
    model: String,
    seconds: u64,
}

#[derive(serde::Serialize)]
struct Making {
    name: String,
    model: String,
    host: String,
    seconds: u64,
}

fn named(folder: &Path) -> String {
    folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[tauri::command]
fn makings(shell: State<'_, Shell>) -> Makings {
    let developer = shell.settings().developer;
    let said = |why: &String| words::shown_detail(why, developer);
    // Read under one hold: a second hold of the same lock in the struct below would never be
    // given, its first still held to the statement's end.
    let fetched = shell.fetched.lock().unwrap().clone();
    let (failed, failed_said) = {
        let failures = shell.minutes_failed.lock().unwrap();
        let failed = failures.iter().map(|(folder, (why, _))| (named(folder), said(why))).collect();
        let causes = failures
            .iter()
            .filter_map(|(folder, (_, cause))| Some((named(folder), words::cause((*cause)?, shell.language).to_string())))
            .collect();
        (failed, causes)
    };
    let writing = shell.writing_minutes.lock().unwrap().clone();
    let by = shell.minutes_by.lock().unwrap().clone();
    Makings {
        minutes: writing.zip(by).map(|(folder, (model, host, since))| Making {
            name: named(&folder),
            model,
            host,
            seconds: since.elapsed().as_secs(),
        }),
        minutes_waiting: shell
            .minutes_waiting
            .lock()
            .unwrap()
            .iter()
            .map(|queued| named(&queued.folder))
            .collect(),
        transcript: shell
            .hearing
            .lock()
            .unwrap()
            .as_deref()
            .zip(shell.hearing_by.lock().unwrap().clone())
            .map(|(folder, (model, since))| Hearing {
                name: named(folder),
                model,
                seconds: since.elapsed().as_secs(),
            }),
        transcripts_waiting: shell.waiting.lock().unwrap().iter().map(|(folder, _)| named(folder)).collect(),
        model_needed: default_model_missing(&shell.settings()),
        // The default model coming, not another: one fetch at a time, said as such (D37).
        model_coming: fetched.as_ref().is_some_and(|(file, ..)| file == model::file(&model::DEFAULT)),
        model_fetching: fetched.map(|(file, held, bytes)| Fetching { file, held, bytes }),
        model_offer_bytes: published().bytes,
        model_offer_host: published().host,
        transcripts_failed: shell
            .transcripts_failed
            .lock()
            .unwrap()
            .iter()
            .map(|(folder, why)| (named(folder), said(why)))
            .collect(),
        transcripts_made: shell
            .transcripts_made
            .lock()
            .unwrap()
            .iter()
            .map(|(folder, times)| (named(folder), *times))
            .collect(),
        failed,
        failed_said,
        made: shell
            .minutes_made
            .lock()
            .unwrap()
            .iter()
            .map(|(folder, times)| (named(folder), *times))
            .collect(),
        awaiting: shell
            .awaiting
            .lock()
            .unwrap()
            .iter()
            .map(|(folder, host)| (named(folder), host.clone()))
            .collect(),
    }
}

/// Minutes made again with a model, asked before the user agreed to send them.
struct AskedAgain {
    choice: (String, String),
    named: Option<(String, String)>,
}

/// Who writes the minutes changed, or a host agreed for the next ones: the meetings set aside
/// are looked at again, in the line — made now, or still waiting for the word, said anew.
fn look_again(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let aside: Vec<PathBuf> = shell.awaiting.lock().unwrap().drain(..).map(|(folder, _)| folder).collect();
    let named = shell
        .reread()
        .ok()
        .and_then(|settings| settings.minutes_provider.zip(settings.minutes_model));
    for folder in aside {
        let chosen = shell
            .asked_again
            .lock()
            .unwrap()
            .remove(&folder)
            .filter(|asked| asked.named == named)
            .map(|asked| asked.choice);
        write_minutes_from(app, folder, None, chosen, true);
    }
}

/// The same, `agreed_now` naming the host the user just agreed to send this meeting to, once:
/// the gate is passed only if that is still the host the settings name as it is read here.
fn write_minutes(app: &AppHandle, folder: PathBuf, agreed_now: Option<&str>, chosen: Option<(String, String)>) {
    write_minutes_from(app, folder, agreed_now, chosen, false);
}

/// `from_line`: taken from the line, where nobody asked just now — an agreement it needs is set
/// aside and said on its page, never a window opened over what the user is doing.
fn write_minutes_from(app: &AppHandle, folder: PathBuf, agreed_now: Option<&str>, chosen: Option<(String, String)>, from_line: bool) {
    let shell = app.state::<Shell>();
    let writing = shell.writing_minutes.lock().unwrap();
    if let Some(being) = writing.as_ref() {
        // One at a time: asked meanwhile, it waits its turn — said on its page —, once.
        // A Refaire… of the one being written (a model named) waits too, never asked for it
        // again otherwise; asked again while waiting, the newer choice is the one made.
        let mut waiting = shell.minutes_waiting.lock().unwrap();
        if let Some(queued) = waiting.iter_mut().find(|queued| queued.folder == folder) {
            if chosen.is_some() {
                queued.chosen = chosen;
            }
        } else if *being != folder || chosen.is_some() {
            // In line now: no longer set aside, and an earlier choice made for it is not the one.
            shell.awaiting.lock().unwrap().retain(|(aside, _)| *aside != folder);
            shell.asked_again.lock().unwrap().remove(&folder);
            waiting.push_back(Queued {
                folder,
                agreed: agreed_now.map(str::to_string),
                chosen,
            });
        }
        return;
    }
    let again = chosen.is_some();
    let asked_again = chosen.clone();
    drop(writing);
    shell.minutes_tried.lock().unwrap().insert(folder.clone(), chosen.clone());
    // Not started: said on the meeting's page as well as in a notification (D25).
    let refused = |why: String| {
        shell.awaiting.lock().unwrap().retain(|(aside, _)| *aside != folder);
        shell.minutes_failed.lock().unwrap().insert(folder.clone(), (why, None));
    };
    let settings = match shell.reread() {
        Ok(settings) => settings,
        Err(why) => {
            refused(why.clone());
            return tell(app, &words::trouble(&Trouble::Settings(why), shell.language));
        }
    };
    let named = settings.minutes_provider.clone().zip(settings.minutes_model.clone());
    // Made again with another model, this time only; or by whoever the settings name.
    let (provider, model) = match chosen.or(settings.minutes_provider.zip(settings.minutes_model)) {
        Some(choice) => choice,
        None => {
            refused("no one is chosen to write the minutes, in the settings".into());
            if from_line {
                return;
            }
            tell(app, &words::minutes(&words::Minutes::Unchosen, shell.language));
            return open_window_at(app, "#/settings");
        }
    };
    if !matches!(meeting::transcript(&folder), meeting::Read::Present(transcript) if transcript.complete) {
        refused("this meeting has no whole transcript".into());
        return tell(app, &words::minutes(&words::Minutes::NoTranscript, shell.language));
    }
    let Some(host) = providers::host(&provider) else {
        let why = format!("{provider} is no provider's address");
        tracing::warn!(detail = %why, "shell.minutes_refused");
        refused(why);
        return tell(app, &words::minutes(&words::Minutes::NoProvider(provider), shell.language));
    };
    if agreed_now != Some(host.as_str()) && !providers::local(&provider) && settings.minutes_agreed.as_deref() != Some(host.as_str()) {
        let mut awaiting = shell.awaiting.lock().unwrap();
        awaiting.retain(|(aside, _)| *aside != folder);
        awaiting.push((folder.clone(), host.clone()));
        drop(awaiting);
        // Made again with a model chosen on the page: once agreed, with that model, as asked.
        let mut asked = shell.asked_again.lock().unwrap();
        match asked_again {
            Some(choice) => asked.insert(folder.clone(), AskedAgain { choice, named }),
            None => asked.remove(&folder),
        };
        drop(asked);
        if from_line {
            return;
        }
        tell(app, &words::minutes(&words::Minutes::Waiting(host), shell.language));
        let name = folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return open_window_at(app, &format!("#/agree/{name}"));
    }
    let mut writing = shell.writing_minutes.lock().unwrap();
    if writing.is_some() {
        // Another started meanwhile, between the reading and now: this one waits its turn.
        drop(writing);
        return write_minutes_from(app, folder, agreed_now, again.then_some((provider, model)), from_line);
    }
    *writing = Some(folder.clone());
    drop(writing);
    // Started: out of the line, its agreement given, its last failure gone.
    shell.minutes_waiting.lock().unwrap().retain(|queued| queued.folder != folder);
    shell.awaiting.lock().unwrap().retain(|(aside, _)| *aside != folder);
    *shell.minutes_by.lock().unwrap() = Some((model.clone(), host.clone(), Instant::now()));
    shell.minutes_failed.lock().unwrap().remove(&folder);
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
                let why = format!("the minutes there could not be set aside ({error}): nothing was made");
                tracing::error!(detail = %why, "shell.minutes_failed");
                minutes_ended(&app, &folder, Some((why, Some(summarising::Cause::Unwritable))));
                return tell(
                    &app,
                    &words::minutes(&words::Minutes::failed(Some(summarising::Cause::Unwritable), again), shell.language),
                );
            }
        };
        let started = Summarising::start(
            &program("wholeheard-summary", "WHOLEHEARD_SUMMARY"),
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
                minutes_ended(&app, &folder, Some((error.to_string(), None)));
                return tell(&app, &words::minutes(&words::Minutes::failed(None, again), shell.language));
            }
        };
        let made = events
            .iter()
            .find_map(|event| match event {
                summarising::Event::Ended(made) => Some(made),
                summarising::Event::Sending { .. } => None,
            })
            .unwrap_or(summarising::Made::Failed(String::new(), None));
        shell.minuting.lock().unwrap().take();
        // Replaced, the earlier minutes go to the trash — refused, they stay beside, and it is said.
        let mut kept_beside = None;
        if let Some(previous) = previous {
            match made {
                summarising::Made::Written(_) if trash::delete(&previous).is_err() => {
                    kept_beside = previous.file_name().map(|name| name.to_string_lossy().into_owned());
                }
                summarising::Made::Written(_) => {}
                summarising::Made::Failed(..) => {
                    let _ = std::fs::remove_file(&previous);
                }
            }
        }
        minutes_ended(
            &app,
            &folder,
            match &made {
                summarising::Made::Failed(why, cause) => Some((why.clone(), *cause)),
                summarising::Made::Written(_) => None,
            },
        );
        if *shell.leaving.lock().unwrap() {
            return;
        }
        let said = match made {
            summarising::Made::Written(_) if kept_beside.is_some() => words::Minutes::ReadyKeptBeside(kept_beside.unwrap_or_default()),
            summarising::Made::Written(_) => words::Minutes::Ready,
            summarising::Made::Failed(_, cause) => words::Minutes::failed(cause, again),
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

/// Opens the full window on one of its pages, in place of the small one (D35).
fn open_window_at(app: &AppHandle, page: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval(format!("location.hash = {}", serde_json::to_string(page).unwrap_or_default()));
        return show_full(app);
    }
    left(app, settings::Window::Full);
    if let Some(small) = app.get_webview_window("small") {
        let _ = small.close();
    }
    build_full(app, page);
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
            Shown::Idle => (labels.start, true, &include_bytes!("../icons/tray.png")[..], "Wholeheard"),
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
/// Wholeheard, where its variable says (`WHOLEHEARD_CAPTURE`, `WHOLEHEARD_TRANSCRIPTION`), or on the path.
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
    let sessions = shell.settings().sessions;
    if let Err(error) = sessions::writable(&sessions) {
        tracing::error!(detail = %error, "shell.sessions_refused");
        return tell(
            app,
            &words::trouble(&Trouble::NoFolder(sessions.display().to_string()), shell.language),
        );
    }
    let started = Recording::start(&program("wholeheard-capture", "WHOLEHEARD_CAPTURE"), &sessions);
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
    // What the devices are: what is said of one gone, or back, depends on the other.
    let mut sides = words::Sides::default();
    for event in said {
        started |= matches!(event, Said::Started(..));
        shell.devices.lock().unwrap().heard(&event);
        if let Said::Started(folder, ..) = &event {
            *shell.recorded.lock().unwrap() = Some(folder.clone());
            *shell.meter.lock().unwrap() = (Some(Instant::now()), [0.0, 0.0]);
            small_recording(app, true);
        }
        if let Said::Level(source, level) = &event {
            shell.meter.lock().unwrap().1[usize::from(*source == Source::Others)] = *level;
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
            _ => sides.told(&event, shell.settings().reminder, shell.language),
        };
        if let Some(notice) = notice {
            tell(app, &notice);
        }
    }
    shell.recorded.lock().unwrap().take();
    *shell.meter.lock().unwrap() = (None, [0.0, 0.0]);
    *shell.devices.lock().unwrap() = recording::Devices::default();
    // The small window comes back to rest, below the other windows again (D35).
    small_recording(app, false);
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

/// Puts a session in line for its transcript, once — made again as a *Refaire…* asked, the
/// newer ask kept when it is asked again.
fn wait(shell: &Shell, folder: PathBuf, again: Option<Again>) {
    let mut waiting = shell.waiting.lock().unwrap();
    match waiting.iter_mut().find(|(queued, _)| *queued == folder) {
        Some((_, chosen)) if again.is_some() => *chosen = again,
        Some(_) => {}
        None => waiting.push_back((folder, again)),
    }
}

/// A transcript begun: what makes it, said on its page; its last failure gone.
fn hearing(shell: &Shell, folder: &Path, hearer: &Hearer) {
    *shell.hearing.lock().unwrap() = Some(folder.to_path_buf());
    *shell.hearing_by.lock().unwrap() = Some((hearer.name(), Instant::now()));
    shell.transcripts_failed.lock().unwrap().remove(folder);
}

/// A transcript ended: counted when written, its reason kept when not — said on its page.
fn heard(shell: &Shell, folder: &Path, made: &Made) {
    shell.hearing.lock().unwrap().take();
    shell.hearing_by.lock().unwrap().take();
    match made {
        Made::Written(_) => *shell.transcripts_made.lock().unwrap().entry(folder.to_path_buf()).or_default() += 1,
        Made::Failed(why) => {
            let why = if why.trim().is_empty() {
                "wholeheard-transcription stopped without saying why"
            } else {
                why
            };
            shell
                .transcripts_failed
                .lock()
                .unwrap()
                .insert(folder.to_path_buf(), why.to_string());
        }
    }
}

/// The next transcript in line, once one ends: made, or made again with the model a Refaire…
/// chose; one that cannot begin says why on its page, and the next one is taken.
fn transcribe_next(app: &AppHandle) {
    let shell = app.state::<Shell>();
    // At most once each: one that cannot begin is said on its page, and the next one is taken.
    let mut tries = shell.waiting.lock().unwrap().len();
    while tries > 0 {
        tries -= 1;
        // Taken from the line under its lock, one at a time, in order. The default model missing,
        // those made with it wait where they are — offered once already —, and the first
        // Refaire… behind them, its model there, is made.
        let missing = default_model_missing(&shell.settings());
        let next = {
            let mut waiting = shell.waiting.lock().unwrap();
            let at = waiting
                .iter()
                .position(|(_, again)| matches!(again, Some(Again::Model(_))) || !missing);
            at.and_then(|at| waiting.remove(at))
        };
        match next {
            None => return,
            Some((folder, None)) => transcribe(app, folder),
            Some((folder, Some(again))) => retranscribe_or_say(app, folder, again),
        }
        if shell.transcription.lock().unwrap().is_some() {
            return;
        }
    }
}

/// Made again as a Refaire… asked; one that cannot begin says why on its page.
fn retranscribe_or_say(app: &AppHandle, folder: PathBuf, again: Again) {
    if let Err(why) = retranscribe(app, folder.clone(), again) {
        app.state::<Shell>().transcripts_failed.lock().unwrap().insert(folder, why);
    }
}

/// *Refaire…* on a transcript: made again with the model picked, this time only — or, the context
/// changed, by whoever the settings name to hear, the minutes made again after it (D59). The
/// transcript there stays in place until the new one is whole — set aside for the run, since the
/// program goes on from a transcript it finds —, then goes to the system's trash; a run that
/// fails, or that Wholeheard ends as it quits, puts it back.
fn retranscribe(app: &AppHandle, folder: PathBuf, again: Again) -> Result<(), String> {
    let shell = app.state::<Shell>();
    let mut running = shell.transcription.lock().unwrap();
    if running.is_some() {
        // One at a time: it waits its turn, said on its page — the one being made too.
        drop(running);
        wait(&shell, folder, Some(again));
        return Ok(());
    }
    shell.transcripts_tried.lock().unwrap().insert(folder.clone(), Some(again.clone()));
    let settings = shell.reread()?;
    let (hearer, key) = match &again {
        Again::Model(model) if !settings::whisper_models(&models_folder(), &settings).contains(model) => {
            return Err(format!("{} is no model on this computer", model.display()));
        }
        Again::Model(model) => (Hearer::Model(model.clone()), None),
        Again::Settings => hearer_now(&shell, &settings),
    };
    if let Hearer::Model(model) = &hearer
        && !model.is_file()
    {
        return Err(format!("{} is no model on this computer", model.display()));
    }
    // Made again with the context: its minutes too, when it had some or the settings make them.
    let minutes_after = again == Again::Settings && (folder.join("minutes.md").is_file() || settings.after == After::Minutes);
    // Set aside under a name of its own, never written over; put back if the new one is not made.
    let current = folder.join("transcript.jsonl");
    let previous = match sessions::set_aside(&current, true) {
        Ok(previous) => Some(previous),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("the transcript there could not be set aside ({error}): nothing was made")),
    };
    let started = Transcription::start_with(
        &program("wholeheard-transcription", "WHOLEHEARD_TRANSCRIPTION"),
        &folder,
        &hearer,
        key.as_deref(),
    );
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
    // Who puts the earlier transcript back, or trashes it: this run's end, or Wholeheard quitting —
    // whichever comes first takes it, the other finds nothing.
    *shell.redoing.lock().unwrap() = previous.clone().map(|previous| (current.clone(), previous));
    hearing(&shell, &folder, &hearer);
    drop(running);
    transcribing(app, true);
    let app = app.clone();
    std::thread::spawn(move || {
        let made = made.recv().unwrap_or(Made::Failed(String::new()));
        let shell = app.state::<Shell>();
        // Ended and said so at once: none begun in between loses what it says of itself.
        let mut running = shell.transcription.lock().unwrap();
        running.take();
        heard(&shell, &folder, &made);
        drop(running);
        transcribing(&app, false);
        // Replaced, the earlier transcript goes to the trash — refused, it stays beside, and it is
        // said; not replaced, it is put back — failing that, it is said where it is.
        let mut kept_beside = None;
        if let Some((current, previous)) = shell.redoing.lock().unwrap().take() {
            let name = previous.file_name().map(|name| name.to_string_lossy().into_owned());
            let written = matches!(made, Made::Written(_));
            if let Some(not_put_back) = sessions::settle(&current, &previous, written, |previous| {
                trash::delete(previous).map_err(std::io::Error::other)
            }) {
                kept_beside = name.map(|name| (name, not_put_back));
            }
        }
        if *shell.leaving.lock().unwrap() {
            return;
        }
        tell(&app, &words::made(&made, shell.language));
        if let Some((name, not_put_back)) = kept_beside {
            tell(&app, &words::transcript_kept_beside(shell.language, &name, not_put_back));
        }
        // The minutes made again from the new transcript, by whoever the settings name now: the
        // ones there stay until the new ones are whole (`write_minutes_from`).
        if minutes_after && matches!(made, Made::Written(_)) {
            let named = shell
                .reread()
                .ok()
                .and_then(|settings| settings.minutes_provider.zip(settings.minutes_model));
            write_minutes(&app, folder, None, named);
        }
        // The transcripts waiting in line go on, as after any other.
        transcribe_next(&app);
    });
    Ok(())
}

/// Who hears now, as the settings name it — Apple's SpeechAnalyzer while this Mac offers it, a
/// service once agreed to (D42, D43) —, and the service's key: what a transcript is made with.
fn hearer_now(shell: &Shell, settings: &Settings) -> (Hearer, Option<String>) {
    // Apple's SpeechAnalyzer needs no model file: its own is on the Mac (D42). Chosen and no
    // longer offered here — an older macOS, the program gone —, Whisper hears, and it is logged.
    let language = shell.language.code();
    let hearer = Hearer::of(settings, language, || {
        let program = program("wholeheard-speech", "WHOLEHEARD_SPEECH");
        speech::status(&program, language).is_some_and(|status| status.offered && status.installed)
    });
    if settings.engine == Engine::SpeechAnalyzer && matches!(hearer, Hearer::Model(_)) {
        tracing::warn!("shell.speech_unavailable");
    }
    // A service not agreed to yet sends nothing: Whisper hears, and it is logged (D43).
    if settings.engine == Engine::Service && matches!(hearer, Hearer::Model(_)) {
        tracing::warn!("shell.service_not_agreed");
    }
    // Its key from the keyring, under the transcription's own account: never the minutes'.
    let key = match &hearer {
        Hearer::Service { provider, .. } => {
            providers::host(provider).and_then(|host| keys::key(&transcription::key_account(&host)).ok().flatten())
        }
        _ => None,
    };
    (hearer, key)
}

/// Makes the transcript of a session — after the one being made, if there is one. What is
/// missing — the model, the program — is said in one sentence, and on its page; the recording is
/// never touched. Whether it began.
fn transcribe(app: &AppHandle, folder: PathBuf) {
    let shell = app.state::<Shell>();
    let mut running = shell.transcription.lock().unwrap();
    if running.is_some() {
        wait(&shell, folder, None);
        return;
    }
    shell.transcripts_tried.lock().unwrap().insert(folder.clone(), None);
    let _ = shell.reread();
    let settings = shell.settings();
    let model = &settings.model;
    let (hearer, key) = hearer_now(&shell, &settings);
    if matches!(hearer, Hearer::Model(_)) && !model.is_file() {
        // The default model is offered, its size and host shown; one the user named is theirs.
        if *model == Settings::default().model {
            wait(&shell, folder, None);
            // Already coming: the session waits for it, and nothing is offered twice.
            if !*shell.fetching.lock().unwrap() {
                offer(app);
            }
            return;
        } else {
            let trouble = Trouble::NoModel(model.display().to_string());
            shell
                .transcripts_failed
                .lock()
                .unwrap()
                .insert(folder, format!("{} is no model on this computer", model.display()));
            tell(app, &words::trouble(&trouble, shell.language));
        }
        return;
    }
    let started = Transcription::start_with(
        &program("wholeheard-transcription", "WHOLEHEARD_TRANSCRIPTION"),
        &folder,
        &hearer,
        key.as_deref(),
    );
    let (transcription, made) = match started {
        Ok(started) => started,
        Err(error) => {
            shell.transcripts_failed.lock().unwrap().insert(folder, error.to_string());
            tell(app, &words::trouble(&Trouble::NoTranscriber(error.to_string()), shell.language));
            return;
        }
    };
    *running = Some(transcription);
    hearing(&shell, &folder, &hearer);
    transcribing(app, true);
    let heard_folder = folder;
    let app = app.clone();
    std::thread::spawn(move || {
        let made = made.recv().unwrap_or(Made::Failed(String::new()));
        let shell = app.state::<Shell>();
        let mut running = shell.transcription.lock().unwrap();
        running.take();
        heard(&shell, &heard_folder, &made);
        drop(running);
        transcribing(&app, false);
        // Ended by Wholeheard as it quits: nothing to tell, it goes on next time.
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
        transcribe_next(&app);
    });
}

/// The default model as published — or, for whoever tries Wholeheard, as `WHOLEHEARD_MODEL_PUBLISHED`
/// says: its address, its size in bytes and its SHA-256, separated by spaces.
fn published() -> Published {
    published_at(0)
}

/// One of the models offered, as published — or as `WHOLEHEARD_MODEL_PUBLISHED` says, its
/// entries separated by `;` in the order of the list offered.
fn published_at(index: usize) -> Published {
    // The list may have been read again since the index was taken: none there is the default.
    let listed = offered().get(index).map_or(model::DEFAULT, |listed| listed.published);
    published_named(index).unwrap_or(listed)
}

/// What `WHOLEHEARD_MODEL_PUBLISHED` says of one of the models offered, if it says it.
fn published_named(index: usize) -> Option<Published> {
    std::env::var("WHOLEHEARD_MODEL_PUBLISHED").ok().and_then(|named| {
        let named = named.split(';').nth(index)?.to_owned();
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
    })
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

/// What Apple's SpeechAnalyzer offers on this Mac, in the interface's language (D42): asked of
/// `wholeheard-speech` beside the shell, away from the window; none where it is not.
#[tauri::command]
async fn speech_shown(shell: State<'_, Shell>) -> Result<Option<speech::Status>, ()> {
    let language = shell.language.code();
    let program = program("wholeheard-speech", "WHOLEHEARD_SPEECH");
    Ok(tauri::async_runtime::spawn_blocking(move || speech::status(&program, language))
        .await
        .ok()
        .flatten())
}

/// Apple's model of the interface's language, fetched by macOS from Apple on the user's click (D42);
/// then SpeechAnalyzer is who hears. A refusal is logged with what the program said, and said
/// on the page in a sentence of its own.
#[tauri::command]
async fn speech_installed(shell: State<'_, Shell>) -> Result<(), ()> {
    // One download at a time: a second click while macOS fetches it asks nothing more.
    static INSTALLING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if INSTALLING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Err(());
    }
    let language = shell.language.code();
    let program = program("wholeheard-speech", "WHOLEHEARD_SPEECH");
    tracing::info!(language, "shell.speech_install");
    let installed = tauri::async_runtime::spawn_blocking(move || speech::install(&program, language)).await;
    INSTALLING.store(false, std::sync::atomic::Ordering::SeqCst);
    match installed.map_err(|error| error.to_string()).and_then(|done| done) {
        Ok(()) => {
            tracing::info!(language, "shell.speech_installed");
            Ok(())
        }
        Err(detail) => {
            tracing::warn!(detail = %detail, "shell.speech_install_failed");
            Err(())
        }
    }
}

/// The default Whisper model is the one heard with, and not on this computer: offered (D36). Not
/// when Apple's SpeechAnalyzer hears (D42), nor for a model the user named — theirs.
fn default_model_missing(settings: &Settings) -> bool {
    settings.engine == Engine::Whisper && settings.model == Settings::default().model && !settings.model.is_file()
}

/// The window's offer of the default model, where it says it is missing (D36): the tray's own
/// fetch. One the user named is theirs: nothing is fetched for it.
#[tauri::command]
fn model_fetch(app: AppHandle) {
    let model = app.state::<Shell>().settings().model;
    if model == Settings::default().model && !model.is_file() {
        fetch(&app, 0);
    }
}

/// The minutes' recommended model, fetched by Ollama on the user's click, unless a fetch is on (D37).
#[tauri::command]
fn ollama_pull_offered(app: AppHandle) {
    pull(&app);
}

/// Ollama's own pull of the minutes' recommended model, one fetch at a time with the Whisper ones:
/// how far said as theirs is, the tray's entry waiting meanwhile (D37).
fn pull(app: &AppHandle) {
    let shell = app.state::<Shell>();
    if std::mem::replace(&mut *shell.fetching.lock().unwrap(), true) {
        return;
    }
    let language = shell.language;
    *shell.fetched.lock().unwrap() = Some((providers::RECOMMENDED.to_owned(), 0, 0));
    let waiting = |app: &AppHandle, enabled: bool| {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || {
            let _ = app.state::<Shell>().download.set_enabled(enabled);
        });
    };
    waiting(app, false);
    let app = app.clone();
    std::thread::spawn(move || {
        let outcome = providers::ollama_pull(&ollama(), providers::RECOMMENDED, &mut |held, bytes| {
            if let Some((_, far, all)) = app.state::<Shell>().fetched.lock().unwrap().as_mut() {
                (*far, *all) = (held, bytes);
            }
        });
        let shell = app.state::<Shell>();
        *shell.fetched.lock().unwrap() = None;
        *shell.fetching.lock().unwrap() = false;
        if let Err(why) = &outcome {
            tracing::error!(detail = %why, "shell.model_pull_failed");
        }
        tell(&app, &words::pulled(&outcome, providers::RECOMMENDED, language));
        waiting(&app, true);
    });
}

/// One of the models Settings offer, fetched on the user's click, unless it is there (D37).
#[tauri::command]
fn model_offered_fetch(app: AppHandle, file: String) {
    let Some(index) = offered().iter().position(|listed| model::file(&listed.published) == file) else {
        return;
    };
    if !models_folder().join(&file).is_file() {
        fetch(&app, index);
    }
}

/// Fetches the default model, on the user's click — their agreement. Ready, the entry leaves
/// the menu and the waiting transcript is made; cut or refused, it is said, and the entry
/// takes it up again.
fn fetch(app: &AppHandle, index: usize) {
    let shell = app.state::<Shell>();
    if std::mem::replace(&mut *shell.fetching.lock().unwrap(), true) {
        return;
    }
    // The list read once: its address and the file it is kept as are one model's, whatever is
    // read again meanwhile.
    let listed = offered().get(index).map_or(model::DEFAULT, |listed| listed.published);
    let (wanted, language) = (published_named(index).unwrap_or(listed), shell.language);
    let file = model::file(&listed);
    let to = models_folder().join(file);
    // The tray's entry is the default model's: it says how far that one is, and waits while
    // another comes — one fetch at a time.
    let label = move |app: &AppHandle, text: Option<String>, enabled: bool| {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || {
            let shell = app.state::<Shell>();
            if let Some(text) = text {
                let _ = shell.download.set_text(text);
            }
            let _ = shell.download.set_enabled(enabled);
        });
    };
    let said = move |held: u64| (index == 0).then(|| words::fetching(language, held, wanted.bytes));
    *shell.fetched.lock().unwrap() = Some((file.to_owned(), 0, wanted.bytes));
    label(app, said(0), false);
    let app = app.clone();
    std::thread::spawn(move || {
        let mut shown = String::new();
        let outcome = model::fetch(&wanted, wanted.address, &to, &mut |held| {
            if let Some((_, far, _)) = app.state::<Shell>().fetched.lock().unwrap().as_mut() {
                *far = held;
            }
            let text = words::fetching(language, held, wanted.bytes);
            if text != shown {
                shown.clone_from(&text);
                label(&app, said(held), false);
            }
        });
        let shell = app.state::<Shell>();
        *shell.fetched.lock().unwrap() = None;
        *shell.fetching.lock().unwrap() = false;
        if let Err(why) = &outcome {
            tracing::error!(detail = %why, "shell.model_fetch_failed");
        }
        // The one the transcripts are made with: those in line follow it. Another is chosen
        // in Settings, the choice the user's (D37).
        let chosen = shell.settings().model == to;
        tell(&app, &words::fetched(&outcome, chosen, language));
        if outcome.is_err() {
            let text = words::download(language, &model::size(published().bytes), published().host);
            return label(&app, Some(text), true);
        }
        if index != 0 {
            return label(&app, None, true);
        }
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let shell = handle.state::<Shell>();
            let _ = shell.menu.remove(&shell.download);
        });
        // The meetings in line, none other: fetched from Settings, none may wait (D36).
        if chosen {
            transcribe_next(&app);
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
            .appname("Wholeheard")
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

/// Quits Wholeheard. A recording in progress is stopped first, as the user would stop it: its
/// files are whole before Wholeheard is gone. Asked a second time, Wholeheard leaves at once: a
/// recorder that does not end is not a reason to keep the user.
fn leave(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let mut recording = shell.recording.lock().unwrap();
    let asked_before = std::mem::replace(&mut *shell.leaving.lock().unwrap(), true);
    // A transcript being made is ended: run again, it goes on from where it was.
    if let Some(transcription) = shell.transcription.lock().unwrap().as_ref() {
        transcription.end();
    }
    // A transcript being made again is ended: the earlier one is put back now, before Wholeheard goes.
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
            tracing::warn!(detail = %why, "shell.shortcut_refused");
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
    // under this very name (`linux/dev.wholeheard.Wholeheard.desktop`).
    let id: ashpd::AppID = "dev.wholeheard.Wholeheard"
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
