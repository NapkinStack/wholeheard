//! A transcript, made: the program that hears a session is started on its folder and waited
//! for. The shell reads none of what it hears — the program writes `transcript.jsonl`
//! (`contracts/transcript/v1`) in the session's folder, and says in one sentence what failed.

use std::{
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
    thread,
    time::Duration,
};
use tracing::{error, info};

/// How a transcript ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Made {
    /// Written whole, in this session's folder.
    Written(PathBuf),
    /// Not written, or not whole: the program's own sentence, when it gave one. Run again,
    /// the program goes on from where it was.
    Failed(String),
}

/// A transcript in the making. Dropped, it stops nothing; ended, the program is stopped, and
/// what it had written is kept for the next run.
pub struct Transcription {
    program: Arc<Mutex<Child>>,
}

impl Transcription {
    /// Starts the program that hears on a session's folder, with a model file. How it ended
    /// comes once on the receiver: `Written` only when it ended well with the transcript there.
    pub fn start(program: &Path, folder: &Path, model: &Path) -> io::Result<(Self, Receiver<Made>)> {
        let (trace, making) = crate::log::making();
        let _making = making.enter();
        let mut command = Command::new(program);
        command.arg(folder).arg("--model").arg(model).env("WHOLEHEARD_TRACE", &trace);
        info!(session = %named(folder), model = %named(model), "shell.transcript");
        // A group of its own: ended, whatever it started ends with it.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut command, 0);
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .inspect_err(|error| error!(detail = %error, "shell.transcript_failed"))?;
        let (Some(mut output), Some(errors)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(io::Error::other("the program that hears could not be listened to"));
        };
        // Its output is read to its end, so that it never fills and holds the program up.
        thread::spawn(move || io::copy(&mut output, &mut io::sink()));
        let program = Arc::new(Mutex::new(child));
        let (tell, made) = mpsc::channel();
        let (waited, folder) = (Arc::clone(&program), folder.to_path_buf());
        // Its last sentence on the error stream is why it failed, kept as it is read, apart from
        // the wait: a program of its own may keep that stream open after it ended.
        let (last, read) = (Arc::new(Mutex::new(String::new())), mpsc::channel());
        let (kept, (done, all_read)) = (Arc::clone(&last), read);
        thread::spawn(move || {
            last_line(errors, &kept);
            let _ = done.send(());
        });
        let making = making.clone();
        thread::spawn(move || {
            let _making = making.enter();
            // Asked, not waited on with the program held: `end` may need it meanwhile.
            let status = loop {
                match waited.lock().map(|mut child| child.try_wait()) {
                    Ok(Ok(None)) => thread::sleep(Duration::from_millis(100)),
                    Ok(Ok(Some(status))) => break Some(status),
                    _ => break None,
                }
            };
            let written = status.is_some_and(|status| status.success()) && folder.join("transcript.jsonl").is_file();
            // A second for what it wrote last to be read; then what was read is what it said.
            let _ = all_read.recv_timeout(Duration::from_secs(1));
            let why = last.lock().map(|said| said.clone()).unwrap_or_default();
            match written {
                true => info!("shell.transcript_written"),
                false => error!(detail = %crate::log::without(&why, folder.parent()), status = ?status, "shell.transcript_failed"),
            }
            let _ = tell.send(if written { Made::Written(folder) } else { Made::Failed(why) });
        });
        Ok((Self { program }, made))
    }

    /// Stops the program now: Wholeheard is quitting. What it had heard stays written.
    pub fn end(&self) {
        if let Ok(mut child) = self.program.lock() {
            // Only while it runs: once it is waited for, its group's number may be another's.
            #[cfg(unix)]
            if let (Ok(None), Ok(group)) = (child.try_wait(), i32::try_from(child.id())) {
                // SAFETY: a signal sent to the group this program leads, which it was started in.
                unsafe { libc::kill(-group, libc::SIGKILL) };
            }
            let _ = child.kill();
        }
    }
}

/// A file's or a folder's own name, never its path: the path says whose meetings and where.
pub(crate) fn named(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Reads a stream to its end, keeping its last line that is not blank as it goes.
pub(crate) fn last_line(stream: impl Read, kept: &Mutex<String>) {
    for line in BufReader::new(stream).split(b'\n').map_while(Result::ok) {
        let line = String::from_utf8_lossy(&line).trim().to_string();
        if let (false, Ok(mut kept)) = (line.is_empty(), kept.lock()) {
            *kept = line;
        }
    }
}

/// The last session of a folder of sessions: the one whose name comes last — a session is
/// named by when it started — among those that hold a `session.json`.
pub fn latest(sessions: &Path) -> Option<PathBuf> {
    let folders = std::fs::read_dir(sessions).ok()?.filter_map(Result::ok).map(|entry| entry.path());
    folders.filter(|folder| folder.join("session.json").is_file()).max()
}
