//! Apple's SpeechAnalyzer, on a Mac with macOS 26 (D42, ADR-0012), behind the step's interface:
//! each span is handed to `wholeheard-speech` — a Swift program of this module's — as a file of the
//! session's own form, and each line it prints is a segment. Nothing of the meeting is logged.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    Segment,
    step::{Engine, RATE},
};

/// SpeechTranscriber hears one locale, never a guess: the languages the shell speaks, each its own.
const LOCALES: [(&str, &str); 2] = [("fr", "fr-FR"), ("en", "en-US")];

pub struct SpeechAnalyzer {
    program: PathBuf,
    locale: &'static str,
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
        })
    }

    /// Where `wholeheard-speech` is: where `WHOLEHEARD_SPEECH` names it, or beside this program.
    pub fn program() -> PathBuf {
        std::env::var_os("WHOLEHEARD_SPEECH").map(PathBuf::from).unwrap_or_else(|| {
            let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf));
            beside.unwrap_or_default().join("wholeheard-speech")
        })
    }
}

impl Engine for SpeechAnalyzer {
    fn name(&self) -> (String, String) {
        ("SpeechAnalyzer".into(), format!("Apple {}", self.locale))
    }

    /// The context the user attached is not given: the program takes none yet (ADR-0012).
    fn hear(&mut self, sound: &[f32], _language: Option<&str>, _context: Option<&str>) -> io::Result<Vec<Segment>> {
        // The span's file, this run's alone, removed once heard, whatever the program said.
        static SPANS: AtomicUsize = AtomicUsize::new(0);
        let file = std::env::temp_dir().join(format!(
            "wholeheard-span-{}-{}.wav",
            std::process::id(),
            SPANS.fetch_add(1, Ordering::Relaxed)
        ));
        let output = write_span(&file, sound).and_then(|()| {
            Command::new(&self.program)
                .arg("transcribe")
                .arg(&file)
                .arg(self.locale)
                .stdin(Stdio::null())
                .output()
        });
        let _ = fs::remove_file(&file);
        let output = output?;
        if !output.status.success() {
            let said = String::from_utf8_lossy(&output.stderr);
            let why = said.lines().next().unwrap_or("it ended without saying why").trim().to_string();
            return Err(io::Error::other(format!("SpeechAnalyzer did not hear the span: {why}")));
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(segment)
            .collect()
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
fn write_span(path: &Path, sound: &[f32]) -> io::Result<()> {
    let bytes = (sound.len() * 2) as u32;
    let mut out = io::BufWriter::new(fs::File::create(path)?);
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
    out.flush()
}
