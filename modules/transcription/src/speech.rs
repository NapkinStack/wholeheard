//! Apple's SpeechAnalyzer, on a Mac with macOS 26 (D42, ADR-0012), behind the step's interface:
//! each span is handed to `wholeheard-speech` — a Swift program of this module's — as a file of the
//! session's own form, and each line it prints is a segment. Nothing of the meeting is logged.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{
    Segment,
    step::{Engine, RATE},
};

/// SpeechTranscriber hears one locale, never a guess: the languages the shell speaks, each its own.
const LOCALES: [(&str, &str); 2] = [("fr", "fr-FR"), ("en", "en-US")];

/// The most the program may say of one span: a few kilobytes of text in truth.
const SAID: u64 = 4 << 20;

pub struct SpeechAnalyzer {
    program: PathBuf,
    locale: &'static str,
    /// Where each span's file is written: the session's own `derived/`, where its other derived
    /// sound is, so that a step killed mid-span leaves nothing of the meeting elsewhere.
    spans: PathBuf,
    /// How long a span may take, beyond twice its own length, before the program is ended.
    patience: Duration,
    told_of_context: bool,
}

impl SpeechAnalyzer {
    /// The engine run through `program`, hearing `language` — French or English.
    pub fn at(program: &Path, language: &str) -> Result<SpeechAnalyzer, String> {
        let locale = LOCALES
            .iter()
            .find(|(code, _)| *code == language)
            .map(|(_, locale)| *locale)
            .ok_or("SpeechAnalyzer hears French or English: the language is fr or en")?;
        if !program.is_file() {
            return Err(format!("{} is not here: SpeechAnalyzer is heard on a Mac", program.display()));
        }
        Ok(SpeechAnalyzer {
            program: program.to_path_buf(),
            locale,
            spans: std::env::temp_dir(),
            patience: Duration::from_secs(60),
            told_of_context: false,
        })
    }

    /// Each span's file written in `folder`, made if it is not there.
    pub fn spans_in(mut self, folder: &Path) -> io::Result<Self> {
        fs::create_dir_all(folder)?;
        self.spans = folder.to_path_buf();
        Ok(self)
    }

    /// How long a span may take beyond twice its length: a minute out of the box.
    pub fn patient(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    /// Where `wholeheard-speech` is: where `WHOLEHEARD_SPEECH` names it, or beside this program.
    pub fn program() -> PathBuf {
        std::env::var_os("WHOLEHEARD_SPEECH").map(PathBuf::from).unwrap_or_else(|| {
            let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
            beside.unwrap_or_default().join("wholeheard-speech")
        })
    }

    /// The program run on one span's file, ended once `within` has passed; what it said, read
    /// as it says it and no more than `SAID`.
    fn run(&self, file: &Path, within: Duration) -> io::Result<(bool, String, String)> {
        let mut child = Command::new(&self.program)
            .arg("transcribe")
            .arg(file)
            .arg(self.locale)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let reading = |stream: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut text = String::new();
                if let Some(stream) = stream {
                    let _ = stream.take(SAID).read_to_string(&mut text);
                }
                text
            })
        };
        let out = reading(child.stdout.take().map(|out| Box::new(out) as Box<dyn Read + Send>));
        let err = reading(child.stderr.take().map(|err| Box::new(err) as Box<dyn Read + Send>));
        let given = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if given.elapsed() > within {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "SpeechAnalyzer did not answer in time for the span",
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        Ok((status.success(), out.join().unwrap_or_default(), err.join().unwrap_or_default()))
    }
}

impl Engine for SpeechAnalyzer {
    fn name(&self) -> (String, String) {
        ("SpeechAnalyzer".into(), format!("Apple {}", self.locale))
    }

    /// The context the user attached is not given: the program takes none yet (ADR-0012); said
    /// once in the log.
    fn hear(&mut self, sound: &[f32], _language: Option<&str>, context: Option<&str>) -> io::Result<Vec<Segment>> {
        if context.is_some() && !std::mem::replace(&mut self.told_of_context, true) {
            tracing::info!("transcription.context_unused");
        }
        // The span's file, this run's alone — made new, never one already there —, removed once
        // heard, whatever the program said.
        static SPANS: AtomicUsize = AtomicUsize::new(0);
        let file = self.spans.join(format!(
            "speechanalyzer-span-{}-{}.wav",
            std::process::id(),
            SPANS.fetch_add(1, Ordering::Relaxed)
        ));
        let within = self.patience + Duration::from_secs_f64(2.0 * sound.len() as f64 / RATE as f64);
        let written = fs::OpenOptions::new().write(true).create_new(true).open(&file).and_then(|made| {
            let mut out = io::BufWriter::new(made);
            write_span(&mut out, sound)?;
            out.flush()
        });
        let output = written.and_then(|()| self.run(&file, within));
        let _ = fs::remove_file(&file);
        let (succeeded, out, err) = output?;
        if !succeeded {
            let why = err.lines().next().unwrap_or("it ended without saying why").trim().to_string();
            return Err(io::Error::other(format!("SpeechAnalyzer did not hear the span: {why}")));
        }
        out.lines().filter(|line| !line.trim().is_empty()).map(segment).collect()
    }
}

/// One line of the program: `{"start": s, "end": s, "text": "…"}`.
fn segment(line: &str) -> io::Result<Segment> {
    let refused = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "SpeechAnalyzer's program said a line that is not a segment",
        )
    };
    let value: serde_json::Value = serde_json::from_str(line).map_err(|_| refused())?;
    match (value["start"].as_f64(), value["end"].as_f64(), value["text"].as_str()) {
        (Some(start), Some(end), Some(text)) if start <= end => Ok(Segment {
            start,
            end,
            text: text.to_string(),
        }),
        _ => Err(refused()),
    }
}

/// A span as a session's first form holds a track: one channel, 16 kHz, 16-bit.
pub(crate) fn write_span(out: &mut impl Write, sound: &[f32]) -> io::Result<()> {
    let bytes = (sound.len() * 2) as u32;
    out.write_all(b"RIFF")?;
    out.write_all(&(36 + bytes).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&(RATE as u32).to_le_bytes())?;
    out.write_all(&(RATE as u32 * 2).to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for sample in sound {
        out.write_all(&((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}
