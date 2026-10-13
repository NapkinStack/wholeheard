//! A recording, supervised: the program that records is started, listened to as
//! `contracts/recording/v1` describes it, and told to stop. The shell records nothing itself —
//! and a recording goes on if the shell does not.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};

use serde_json::Value;

/// One of a session's two sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Me,
    Others,
}

/// What a recording made known, in the order it happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Said {
    /// A source could not be opened: its file will be silence, the other is still recorded.
    Unavailable(Source, String),
    /// The recording runs, in this session's folder.
    Started(PathBuf),
    /// A source has given no sound for a while.
    Silent(Source),
    /// A source that was silent gives sound again.
    Heard(Source),
    /// The recording was stopped: its folder, and its length in seconds.
    Stopped(PathBuf, f64),
    /// The program ended without having said it stopped: its own sentence, when it gave one.
    Failed(String),
}

/// A line of the program's output as the contract describes it — or nothing: a line that is
/// no event, or an event this version does not know, is passed over.
pub fn read(line: &str) -> Option<Said> {
    let event: Value = serde_json::from_str(line).ok()?;
    let source = || match event["source"].as_str() {
        Some("me") => Some(Source::Me),
        Some("others") => Some(Source::Others),
        _ => None,
    };
    let folder = || event["folder"].as_str().map(PathBuf::from);
    match event["event"].as_str()? {
        "unavailable" => Some(Said::Unavailable(
            source()?,
            event["reason"].as_str().unwrap_or_default().to_string(),
        )),
        "started" => Some(Said::Started(folder()?)),
        "silent" => Some(Said::Silent(source()?)),
        "heard" => Some(Said::Heard(source()?)),
        "stopped" => Some(Said::Stopped(folder()?, event["seconds"].as_f64().unwrap_or(0.0))),
        _ => None,
    }
}

/// The lines of a stream, whatever bytes they hold: what is not text is read with its
/// unreadable bytes replaced, never taken for the stream's end — a reader that let go there
/// would close the stream under a program that is still recording.
fn lines(stream: impl Read) -> impl Iterator<Item = String> {
    let lines = BufReader::new(stream).split(b'\n').map_while(Result::ok);
    lines.map(|line| String::from_utf8_lossy(&line).trim_end_matches('\r').to_string())
}

/// A recording in progress: the program, and what it says. Dropped, it stops nothing: the
/// program records on, and is stopped by a signal.
pub struct Recording {
    program: Child,
    input: ChildStdin,
    /// What the recording says, as it says it. It ends on `Stopped` or `Failed`, then closes.
    pub said: Receiver<Said>,
}

impl Recording {
    /// Starts the program that records, into a new session of this folder of sessions.
    pub fn start(program: &Path, sessions: &Path) -> io::Result<Self> {
        let mut program = Command::new(program)
            .arg("record")
            .arg(sessions)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let (input, output, errors) = (program.stdin.take(), program.stdout.take(), program.stderr.take());
        let (Some(input), Some(output), Some(errors)) = (input, output, errors) else {
            return Err(io::Error::other("the program that records could not be listened to"));
        };
        // Its error stream is read as it comes, so that it never fills: the last sentence is
        // what it says of a failure.
        let complaint = thread::spawn(move || lines(errors).filter(|line| !line.trim().is_empty()).last().unwrap_or_default());
        let (tell, said) = mpsc::channel();
        thread::spawn(move || {
            let mut stopped = false;
            // Read to its end, so that the program is never held up; nothing follows "stopped".
            for event in lines(output).filter_map(|line| read(&line)) {
                if !stopped {
                    stopped = matches!(event, Said::Stopped(..));
                    let _ = tell.send(event);
                }
            }
            // Its output is closed: the program ended. Without "stopped", it failed.
            if !stopped {
                let _ = tell.send(Said::Failed(complaint.join().unwrap_or_default().trim().to_string()));
            }
        });
        Ok(Self { program, input, said })
    }

    /// Tells the program to stop: it writes its files whole, says `stopped`, and ends.
    pub fn stop(&mut self) -> io::Result<()> {
        self.input.write_all(b"stop\n")?;
        self.input.flush()
    }

    /// The program's number for the system: what stops a recording whose shell is gone.
    pub fn id(&self) -> u32 {
        self.program.id()
    }

    /// Whether the program has ended.
    pub fn ended(&mut self) -> bool {
        !matches!(self.program.try_wait(), Ok(None))
    }
}
