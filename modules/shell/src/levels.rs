//! How loud each track of a meeting is, for its diagnostic (D39): its form — rate, channels,
//! depth, length — and for each channel its peak, its mean and its share of exact zeros, in
//! decibels of full scale. Numbers only: nothing a person said can be heard back from them.

use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use serde_json::{Value, json};

/// The tracks a session's folder holds, each by its file: version 1 names them `me.wav` and
/// `others.wav`, version 2 — and a folder whose `session.json` does not read — `local.wav` and
/// `remote.wav`.
fn tracks(folder: &Path) -> [(&'static str, &'static str); 2] {
    let contract = std::fs::read_to_string(folder.join("session.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|session| session["contract"].as_str().map(str::to_string));
    match contract.as_deref() {
        Some("session-audio/v1") => [("local", "me.wav"), ("remote", "others.wav")],
        _ => [("local", "local.wav"), ("remote", "remote.wav")],
    }
}

/// One JSON line a track, the local one first.
pub fn levels(folder: &Path) -> String {
    tracks(folder)
        .iter()
        .map(|(track, file)| {
            let path = folder.join(file);
            let line = if !path.is_file() {
                json!({ "type": "levels", "track": track, "present": false })
            } else {
                match measured(&path) {
                    Ok(mut line) => {
                        line["type"] = json!("levels");
                        line["track"] = json!(track);
                        line
                    }
                    Err(_) => json!({ "type": "levels", "track": track, "present": true, "read": false }),
                }
            };
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The form a WAV says, and where its samples start and how many bytes they take.
struct Form {
    float: bool,
    channels: usize,
    rate: u32,
    bits: u16,
    start: u64,
    length: u64,
}

fn form(file: &mut File) -> io::Result<Form> {
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a WAV this reads");
    let size = file.metadata()?.len();
    let mut head = [0u8; 12];
    file.read_exact(&mut head)?;
    if &head[..4] != b"RIFF" || &head[8..] != b"WAVE" {
        return Err(bad());
    }
    let mut at = 12u64;
    let mut found: Option<(bool, usize, u32, u16)> = None;
    while at + 8 <= size {
        file.seek(SeekFrom::Start(at))?;
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk)?;
        let length = u64::from(u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]));
        match &chunk[..4] {
            b"fmt " => {
                let mut fmt = vec![0u8; length.min(64) as usize];
                file.read_exact(&mut fmt)?;
                if fmt.len() < 16 {
                    return Err(bad());
                }
                let tag = u16::from_le_bytes([fmt[0], fmt[1]]);
                // The extensible form says its own kind in its subformat's first two bytes.
                let kind = if tag == 0xFFFE && fmt.len() >= 26 {
                    u16::from_le_bytes([fmt[24], fmt[25]])
                } else {
                    tag
                };
                let channels = usize::from(u16::from_le_bytes([fmt[2], fmt[3]]));
                let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
                let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
                let float = kind == 3;
                if channels == 0 || !(kind == 1 && matches!(bits, 16 | 24 | 32) || float && bits == 32) {
                    return Err(bad());
                }
                found = Some((float, channels, rate, bits));
            }
            b"data" => {
                let (float, channels, rate, bits) = found.ok_or_else(bad)?;
                // A length not known yet — a recording still growing — reads to the end.
                let length = if length == u64::from(u32::MAX) {
                    size - at - 8
                } else {
                    length.min(size - at - 8)
                };
                return Ok(Form {
                    float,
                    channels,
                    rate,
                    bits,
                    start: at + 8,
                    length,
                });
            }
            _ => {}
        }
        at += 8 + length + (length & 1);
    }
    Err(bad())
}

fn measured(path: &Path) -> io::Result<Value> {
    let mut file = File::open(path)?;
    let form = form(&mut file)?;
    file.seek(SeekFrom::Start(form.start))?;
    let width = usize::from(form.bits / 8);
    let frame = width * form.channels;
    let frames = form.length / frame as u64;
    let mut reader = BufReader::with_capacity(1 << 20, file.take(frames * frame as u64));
    let (mut peak, mut squares, mut zeros) = (vec![0f64; form.channels], vec![0f64; form.channels], vec![0u64; form.channels]);
    let mut buffer = vec![0u8; frame * 4096];
    loop {
        let read = read_whole_frames(&mut reader, &mut buffer, frame)?;
        if read == 0 {
            break;
        }
        for (at, sample) in buffer[..read].chunks_exact(width).enumerate() {
            let value = match (form.float, width) {
                (true, _) => f64::from(f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])),
                (false, 2) => f64::from(i16::from_le_bytes([sample[0], sample[1]])) / 32_768.0,
                (false, 3) => f64::from(i32::from_le_bytes([0, sample[0], sample[1], sample[2]]) >> 8) / 8_388_608.0,
                (false, _) => f64::from(i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])) / 2_147_483_648.0,
            };
            let channel = at % form.channels;
            // What is no number is no sound, and no exact zero either: left out of both.
            if !value.is_finite() {
                continue;
            }
            let value = value.abs();
            peak[channel] = peak[channel].max(value);
            squares[channel] += value * value;
            zeros[channel] += u64::from(value == 0.0);
        }
    }
    let decibels = |x: f64| {
        if x > 0.0 {
            json!((20.0 * x.log10() * 10.0).round() / 10.0)
        } else {
            Value::Null
        }
    };
    let count = frames.max(1) as f64;
    Ok(json!({
        "present": true,
        "read": true,
        "format": if form.float { "float" } else { "pcm" },
        "bits": form.bits,
        "rate": form.rate,
        "channels": form.channels,
        "seconds": (frames as f64 / f64::from(form.rate.max(1)) * 100.0).round() / 100.0,
        "peak_dbfs": peak.iter().map(|&p| decibels(p)).collect::<Vec<_>>(),
        "rms_dbfs": squares.iter().map(|&s| decibels((s / count).sqrt())).collect::<Vec<_>>(),
        "zeros": zeros.iter().map(|&z| (z as f64 / count * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
    }))
}

/// Fills `buffer` with as many whole frames as there are, and says how many bytes.
fn read_whole_frames(reader: &mut impl Read, buffer: &mut [u8], frame: usize) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled - filled % frame)
}
