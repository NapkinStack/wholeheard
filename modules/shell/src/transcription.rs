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
        let mut child = Command::new(program)
            .arg(folder)
            .arg("--model")
            .arg(model)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let (Some(mut output), Some(errors)) = (child.stdout.take(), child.stderr.take()) else {
            return Err(io::Error::other("the program that hears could not be listened to"));
        };
        // Its output is read to its end, so that it never fills and holds the program up.
        thread::spawn(move || io::copy(&mut output, &mut io::sink()));
        let program = Arc::new(Mutex::new(child));
        let (tell, made) = mpsc::channel();
        let (waited, folder) = (Arc::clone(&program), folder.to_path_buf());
        // Its last sentence on the error stream is why it failed, read apart from the wait: a
        // program of its own may keep that stream open after it ended.
        let (said, why) = mpsc::channel();
        thread::spawn(move || said.send(last_line(errors)));
        thread::spawn(move || {
            // Asked, not waited on with the program held: `end` may need it meanwhile.
            let status = loop {
                match waited.lock().map(|mut child| child.try_wait()) {
                    Ok(Ok(None)) => thread::sleep(Duration::from_millis(100)),
                    Ok(Ok(Some(status))) => break Some(status),
                    _ => break None,
                }
            };
            let written = status.is_some_and(|status| status.success()) && folder.join("transcript.jsonl").is_file();
            let why = why.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
            let _ = tell.send(if written { Made::Written(folder) } else { Made::Failed(why) });
        });
        Ok((Self { program }, made))
    }

    /// Stops the program now: Summit is quitting. What it had heard stays written.
    pub fn end(&self) {
        if let Ok(mut child) = self.program.lock() {
            let _ = child.kill();
        }
    }
}

/// The last line of a stream that is not blank, read to the stream's end.
fn last_line(stream: impl Read) -> String {
    let lines = BufReader::new(stream).split(b'\n').map_while(Result::ok);
    let lines = lines.map(|line| String::from_utf8_lossy(&line).trim().to_string());
    lines.filter(|line| !line.is_empty()).last().unwrap_or_default()
}

/// The last session of a folder of sessions: the one whose name comes last — a session is
/// named by when it started — among those that hold a `session.json`.
pub fn latest(sessions: &Path) -> Option<PathBuf> {
    let folders = std::fs::read_dir(sessions).ok()?.filter_map(Result::ok).map(|entry| entry.path());
    folders.filter(|folder| folder.join("session.json").is_file()).max()
}
