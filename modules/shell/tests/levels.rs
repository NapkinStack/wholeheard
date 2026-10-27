//! How loud each track of a meeting is, for its diagnostic: numbers only, never a sample (D39).

use std::{fs, path::Path};

use wholeheard_shell::levels::levels;

/// A WAV of `channels` channels, `bits` deep — 16 or 24 as integers, 32 as floats — its data's
/// length said or left unknown, from one function of the frame's index per channel.
fn wav(path: &Path, rate: u32, channels: u16, bits: u16, unknown: bool, frames: usize, value: impl Fn(usize, usize) -> f32) {
    let float = bits == 32;
    let bytes = usize::from(bits / 8);
    let mut data = Vec::new();
    for frame in 0..frames {
        for channel in 0..usize::from(channels) {
            let v = value(frame, channel);
            match bits {
                16 => data.extend_from_slice(&((v * 32767.0) as i16).to_le_bytes()),
                24 => data.extend_from_slice(&((v * 8_388_607.0) as i32).to_le_bytes()[..3]),
                _ => data.extend_from_slice(&v.to_le_bytes()),
            }
        }
    }
    let extensible = bits == 24;
    let mut fmt = Vec::new();
    fmt.extend_from_slice(
        &(if extensible {
            0xFFFEu16
        } else if float {
            3
        } else {
            1
        })
        .to_le_bytes(),
    );
    fmt.extend_from_slice(&channels.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * u32::from(channels) * bytes as u32).to_le_bytes());
    fmt.extend_from_slice(&(channels * bytes as u16).to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());
    if extensible {
        fmt.extend_from_slice(&22u16.to_le_bytes());
        fmt.extend_from_slice(&bits.to_le_bytes());
        fmt.extend_from_slice(&0u32.to_le_bytes());
        fmt.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
    }
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(
        &(if unknown {
            u32::MAX
        } else {
            (4 + 8 + fmt.len() + 8 + data.len()) as u32
        })
        .to_le_bytes(),
    );
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    file.extend_from_slice(&fmt);
    file.extend_from_slice(b"data");
    file.extend_from_slice(&(if unknown { u32::MAX } else { data.len() as u32 }).to_le_bytes());
    file.extend_from_slice(&data);
    fs::write(path, file).unwrap();
}

fn sine(level: f32) -> impl Fn(usize, usize) -> f32 {
    move |frame, _| level * (frame as f32 * std::f32::consts::TAU * 440.0 / 48_000.0).sin()
}

fn line(said: &str, track: &str) -> serde_json::Value {
    said.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|line| line["track"] == track)
        .unwrap_or_else(|| panic!("no line for {track}: {said}"))
}

#[test]
fn each_track_says_its_form_and_each_channel_its_peak_its_mean_and_its_exact_zeros() {
    let folder = tempfile::tempdir().unwrap();
    // A voice at half scale on one channel, nothing at all on the other: 24 bits, as v2 writes.
    wav(&folder.path().join("local.wav"), 48_000, 2, 24, true, 48_000, |frame, channel| {
        if channel == 0 { sine(0.5)(frame, channel) } else { 0.0 }
    });
    // Floats, at a tenth of full scale: -20 dBFS at its peak, -23 its mean.
    wav(&folder.path().join("remote.wav"), 48_000, 1, 32, false, 96_000, sine(0.1));
    let said = levels(folder.path());

    let local = line(&said, "local");
    assert_eq!(
        (local["rate"].as_u64(), local["channels"].as_u64(), local["bits"].as_u64()),
        (Some(48_000), Some(2), Some(24)),
        "{local}"
    );
    assert!((local["seconds"].as_f64().unwrap() - 1.0).abs() < 0.01, "{local}");
    let peak = local["peak_dbfs"][0].as_f64().unwrap();
    let mean = local["rms_dbfs"][0].as_f64().unwrap();
    assert!((peak + 6.02).abs() < 0.2 && (mean + 9.03).abs() < 0.2, "{local}");
    assert!(
        local["peak_dbfs"][1].is_null() && local["zeros"][1].as_f64() == Some(1.0),
        "{local}"
    );
    assert!(local["zeros"][0].as_f64().unwrap() < 0.01, "{local}");

    let remote = line(&said, "remote");
    assert_eq!(remote["format"], "float", "{remote}");
    let (peak, mean) = (remote["peak_dbfs"][0].as_f64().unwrap(), remote["rms_dbfs"][0].as_f64().unwrap());
    assert!((peak + 20.0).abs() < 0.2 && (mean + 23.0).abs() < 0.2, "{remote}");
    assert!((remote["seconds"].as_f64().unwrap() - 2.0).abs() < 0.01, "{remote}");
}

#[test]
fn a_track_absent_or_unreadable_is_said_so_and_sixteen_bits_are_read() {
    let folder = tempfile::tempdir().unwrap();
    wav(&folder.path().join("local.wav"), 16_000, 1, 16, false, 16_000, sine(0.25));
    fs::write(folder.path().join("remote.wav"), b"RIFF not a wave").unwrap();
    let said = levels(folder.path());
    let local = line(&said, "local");
    assert!((local["peak_dbfs"][0].as_f64().unwrap() + 12.04).abs() < 0.2, "{local}");
    assert_eq!(line(&said, "remote")["read"], false, "{said}");
    let empty = tempfile::tempdir().unwrap();
    let said = levels(empty.path());
    assert_eq!(
        (
            line(&said, "local")["present"].as_bool(),
            line(&said, "remote")["present"].as_bool()
        ),
        (Some(false), Some(false)),
        "{said}"
    );
}
