//! D1, second half — a throwaway probe, not product code.
//!
//! The question: on this system, does a web-view application get a window that renders, a
//! tray icon, and a global shortcut that fires while another window has the focus? The
//! window says what it got; nothing is recorded.

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Default)]
struct Probe {
    tray: Mutex<(bool, String)>,
    shortcut: Mutex<(bool, String)>,
    fired: AtomicU32,
    // The desktop's own way under Wayland: the first run on Fedora showed the shortcut library
    // accepting Ctrl+Alt+R and never firing it — a refusal would have been more honest.
    portal: Mutex<(bool, String)>,
    portal_fired: AtomicU32,
}

#[tauri::command]
fn status(probe: tauri::State<Probe>) -> serde_json::Value {
    let tray = probe.tray.lock().unwrap().clone();
    let shortcut = probe.shortcut.lock().unwrap().clone();
    serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "session": std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        "desktop": std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        "tray_ok": tray.0, "tray": tray.1,
        "shortcut_ok": shortcut.0, "shortcut": shortcut.1,
        "fired": probe.fired.load(Ordering::Relaxed),
        "portal_ok": probe.portal.lock().unwrap().0,
        "portal": probe.portal.lock().unwrap().1.clone(),
        "portal_fired": probe.portal_fired.load(Ordering::Relaxed),
    })
}

fn main() {
    tauri::Builder::default()
        .manage(Probe::default())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        app.state::<Probe>().fired.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let probe = app.state::<Probe>();

            let tray = (|| -> Result<(), Box<dyn std::error::Error>> {
                let show = MenuItem::with_id(app, "show", "Show the probe", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show, &quit])?;
                let icon = app.default_window_icon().ok_or("no icon in the application")?.clone();
                TrayIconBuilder::new()
                    .icon(icon)
                    .tooltip("Summit — D1 probe")
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        _ => {}
                    })
                    .build(app)?;
                Ok(())
            })();
            *probe.tray.lock().unwrap() = match tray {
                Ok(()) => (true, "created — is it visible in the top bar? The application cannot tell".into()),
                Err(error) => (false, format!("refused: {error}")),
            };

            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR);
            *probe.shortcut.lock().unwrap() = match app.global_shortcut().register(shortcut) {
                Ok(()) => (true, "Ctrl+Alt+R accepted by the system's shortcut library".into()),
                Err(error) => (false, format!("refused: {error}")),
            };
            *probe.portal.lock().unwrap() = (false, "not tried on this system: the portal is a Linux desktop's".into());
            #[cfg(target_os = "linux")]
            {
                *probe.portal.lock().unwrap() = (false, "asking the desktop…".into());
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let said = portal(&handle).await;
                    if let Err(error) = said {
                        *handle.state::<Probe>().portal.lock().unwrap() = (false, format!("refused: {error}"));
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![status])
        .run(tauri::generate_context!())
        .expect("the probe could not start");
}

/// Ask the desktop portal for the same shortcut, and count each time it fires. The desktop
/// may show a dialog first: binding a shortcut system-wide is the user's to allow.
#[cfg(target_os = "linux")]
async fn portal(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use futures_util::StreamExt;

    // An application that is not sandboxed must say who it is before the portal answers:
    // the first try was refused with "An app id is required". The desktop accepts the name
    // only when a desktop entry of that name is installed — `dev.summit.d1shell.desktop`.
    let id: ashpd::AppID = "dev.summit.d1shell".parse()?;
    ashpd::register_host_app(id).await.map_err(|error| format!("could not declare the application's id: {error}"))?;

    let shortcuts = GlobalShortcuts::new().await?;
    let session = shortcuts.create_session(Default::default()).await?;
    let wanted = NewShortcut::new("record", "Start or stop a recording").preferred_trigger("CTRL+ALT+r");
    let bound = shortcuts.bind_shortcuts(&session, &[wanted], None, Default::default()).await?.response()?;
    let got: Vec<String> = bound
        .shortcuts()
        .iter()
        .map(|shortcut| format!("{} = {}", shortcut.id(), shortcut.trigger_description()))
        .collect();
    *app.state::<Probe>().portal.lock().unwrap() = if got.is_empty() {
        (false, "the desktop bound nothing: the dialog was dismissed, or no key was set".into())
    } else {
        (true, format!("bound by the desktop: {}", got.join(", ")))
    };
    let mut activated = shortcuts.receive_activated().await?;
    while activated.next().await.is_some() {
        app.state::<Probe>().portal_fired.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}
