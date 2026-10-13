//! `summit` — Summit as the user meets it: an icon in the tray, a global shortcut, and
//! notifications. No window. It starts the program that records and stops it; it records
//! nothing itself, and a recording goes on if this process does not.
//!
//! Everything it decides is in the library beside it — `recording`, `settings`, `words` —
//! and tried there. Here is only what needs a desktop.

use std::{
    path::PathBuf,
    sync::{Mutex, mpsc::Receiver},
    time::{Duration, Instant},
};

use summit_shell::{
    recording::{Recording, Said},
    settings::{self, Settings},
    words::{self, Language, Notice, Trouble},
};
use tauri::{
    AppHandle, Manager, RunEvent, Wry,
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_notification::NotificationExt;

const TRAY: &str = "summit";

/// What the shell holds while it runs.
struct Shell {
    settings: Settings,
    language: Language,
    /// The recording in progress, when there is one.
    recording: Mutex<Option<Recording>>,
    /// The menu's entry that starts and stops, relabelled as the recording goes.
    toggle: MenuItem<Wry>,
    /// The user asked to quit while a recording ran: it is stopped first, then Summit leaves.
    leaving: Mutex<bool>,
    /// When the recording was last started or told to stop: a second press in the same
    /// half-second is the same gesture, not a second one.
    pressed: Mutex<Option<Instant>>,
    /// The recording was told to stop and has not ended yet.
    stopping: Mutex<bool>,
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
        .setup(|app| {
            let language = Language::system();
            let labels = words::menu(language);
            let read = settings::file().map_or_else(|| Ok(Settings::default()), |file| settings::read(&file));
            let toggle = MenuItem::with_id(app, "toggle", labels.start, true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle, &quit])?;
            TrayIconBuilder::with_id(TRAY)
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Summit")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => toggle_recording(app),
                    "quit" => leave(app),
                    _ => {}
                })
                .build(app)?;
            app.manage(Shell {
                settings: read.clone().unwrap_or_default(),
                language,
                recording: Mutex::new(None),
                toggle,
                leaving: Mutex::new(false),
                pressed: Mutex::new(None),
                stopping: Mutex::new(false),
            });
            if let Err(why) = read {
                tell(app.handle(), &words::trouble(&Trouble::Settings(why), language));
            }
            shortcut(app.handle());
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
        if let Some(tray) = app.tray_by_id(TRAY) {
            let _ = tray.set_icon(Image::from_bytes(icon).ok());
            let _ = tray.set_tooltip(Some(tooltip));
        }
    });
}

/// The program that records: beside this one, as an installation puts it — or, for whoever
/// builds Summit, where `SUMMIT_CAPTURE` says, or on the path.
fn recorder() -> PathBuf {
    let name = if cfg!(windows) { "summit-capture.exe" } else { "summit-capture" };
    let beside = std::env::current_exe().ok().and_then(|own| Some(own.parent()?.join(name)));
    std::env::var_os("SUMMIT_CAPTURE")
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
    let started = std::fs::create_dir_all(&shell.settings.sessions).and_then(|()| Recording::start(&recorder(), &shell.settings.sessions));
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
    for event in said {
        started |= matches!(event, Said::Started(_));
        let notice = match &event {
            // Ended before it had started: nothing was recorded, and that is the trouble.
            Said::Failed(why) if !started => Some(words::trouble(&Trouble::NoRecorder(why.clone()), shell.language)),
            _ => words::of(&event, shell.settings.reminder, shell.language),
        };
        if let Some(notice) = notice {
            tell(app, &notice);
        }
    }
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
}

/// Quits Summit. A recording in progress is stopped first, as the user would stop it: its
/// files are whole before Summit is gone. Asked a second time, Summit leaves at once: a
/// recorder that does not end is not a reason to keep the user.
fn leave(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let mut recording = shell.recording.lock().unwrap();
    let asked_before = std::mem::replace(&mut *shell.leaving.lock().unwrap(), true);
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
    if bound.response().map_err(said)?.shortcuts().is_empty() {
        return Err("no key was bound".into());
    }
    let mut pressed = shortcuts.receive_activated().await.map_err(said)?;
    while pressed.next().await.is_some() {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || toggle_recording(&handle));
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
        .map_err(|error| error.to_string())
}
