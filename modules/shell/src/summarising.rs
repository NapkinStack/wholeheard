//! Minutes, made: `wholeheard-summary` is started on a session's folder with a provider, a model and,
//! when one is kept, its key — and what it says is read as `contracts/summarising/v1` describes
//! it: where the request goes before it leaves, then whether the minutes are written. The
//! shell reads none of the minutes here; the program writes `minutes.md` (`minutes/v1`).

use std::{
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};
use tracing::{error, info};

/// What a making says, in its order.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Before anything leaves the machine: who receives the request, as the minutes name it.
    Sending { host: String, model: String, characters: u64 },
    /// The end, once.
    Ended(Made),
}

/// How a making ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Made {
    /// Written whole, in this session's folder.
    Written(PathBuf),
    /// Not written: the program's own sentence, or why there is none.
    Failed(String),
}

/// Minutes in the making. Ended, the program is stopped; the minutes that were there stay.
pub struct Summarising {
    program: Arc<Mutex<Child>>,
}

impl Summarising {
    /// Starts `wholeheard-summary` on a session's folder. The key goes to its environment only — a
    /// key in Wholeheard's own environment never does.
    pub fn start(program: &Path, folder: &Path, provider: &str, model: &str, key: Option<&str>) -> io::Result<(Self, Receiver<Event>)> {
        let (trace, making) = crate::log::making();
        let _making = making.enter();
        let mut command = Command::new(program);
        command
            .arg(folder)
            .args(["--provider", provider, "--model", model])
            .env("WHOLEHEARD_TRACE", &trace);
        let host = crate::providers::host(provider).unwrap_or_default();
        info!(session = %crate::transcription::named(folder), host, model, key = key.is_some(), "shell.minutes");
        command.env_remove("WHOLEHEARD_PROVIDER_KEY");
        if let Some(key) = key {
            command.env("WHOLEHEARD_PROVIDER_KEY", key);
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .inspect_err(|error| error!(detail = %error, "shell.minutes_failed"))?;
        let (Some(output), Some(errors)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(io::Error::other("the program that writes the minutes could not be listened to"));
        };
        let program = Arc::new(Mutex::new(child));
        let (tell, events) = mpsc::channel();
        // Its output, read to its end: `sending` passed on as it comes; the rest is said at the end.
        let (said, (read, all_read)) = (tell.clone(), mpsc::channel());
        let written = Arc::new(AtomicBool::new(false));
        let heard_written = Arc::clone(&written);
        thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if let Some(sending) = sending(&line) {
                    let _ = said.send(sending);
                } else if event(&line).as_deref() == Some("written") {
                    heard_written.store(true, Ordering::SeqCst);
                }
            }
            let _ = read.send(());
        });
        let last = Arc::new(Mutex::new(String::new()));
        let (kept, (errors_read, errors_all)) = (Arc::clone(&last), mpsc::channel());
        thread::spawn(move || {
            crate::transcription::last_line(errors, &kept);
            let _ = errors_read.send(());
        });
        let (waited, folder) = (Arc::clone(&program), folder.to_path_buf());
        let (making, sessions) = (making.clone(), folder.parent().map(Path::to_path_buf));
        thread::spawn(move || {
            let _making = making.enter();
            let status = loop {
                match waited.lock().map(|mut child| child.try_wait()) {
                    Ok(Ok(None)) => thread::sleep(Duration::from_millis(100)),
                    Ok(Ok(Some(status))) => break Some(status),
                    _ => break None,
                }
            };
            // A second for what it said last to be read.
            let _ = all_read.recv_timeout(Duration::from_secs(1));
            let _ = errors_all.recv_timeout(Duration::from_secs(1));
            let ended_well = status.is_some_and(|status| status.success());
            // Written: said so, by a program that ended well, the file there — minutes of before
            // standing alone are not the new ones.
            let made = if ended_well && written.load(Ordering::SeqCst) && folder.join("minutes.md").is_file() {
                Made::Written(folder)
            } else {
                let said = last.lock().map(|said| said.clone()).unwrap_or_default();
                let said = said.strip_prefix("wholeheard-summary: ").unwrap_or(&said).to_string();
                Made::Failed(match (said.is_empty(), status) {
                    (false, _) => said,
                    (true, Some(status)) => format!("wholeheard-summary stopped without saying why ({status})"),
                    (true, None) => "wholeheard-summary could not be waited for".into(),
                })
            };
            match &made {
                Made::Written(_) => info!("shell.minutes_written"),
                Made::Failed(why) => {
                    error!(detail = %crate::log::without(why, sessions.as_deref()), status = ?status, "shell.minutes_failed")
                }
            }
            let _ = tell.send(Event::Ended(made));
        });
        Ok((Self { program }, events))
    }

    /// Stops the program now: Wholeheard is quitting. Nothing half-written is left by it.
    pub fn end(&self) {
        if let Ok(mut child) = self.program.lock() {
            let _ = child.kill();
        }
    }
}

/// The event a line of the contract names; none for a line that is not one.
fn event(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    Some(value.get("event")?.as_str()?.to_string())
}

/// A `sending` line of the contract; any other line — a later version's event, or none — is not.
fn sending(line: &str) -> Option<Event> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event")?.as_str()? != "sending" {
        return None;
    }
    Some(Event::Sending {
        host: value.get("host")?.as_str()?.to_string(),
        model: value.get("model")?.as_str()?.to_string(),
        characters: value.get("characters")?.as_u64()?,
    })
}
