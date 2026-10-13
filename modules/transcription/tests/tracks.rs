//! A session recorded as `session-audio/v2` describes it, read for an engine: tracks made
//! here — notes, no voice of anyone — at the rates and with the channels a system delivers.

use std::{f32::consts::TAU, fs, path::Path};

use summit_transcription::{
    Speaker,
    step::{RATE, session},
    tracks::{first_point, heard},
};

/// A track as the capture writes one: 24 bits, the extensible header, lengths "unknown" as
/// while recording. `sound` gives each channel's sample at a frame.
fn track(path: &Path, rate: u32, channels: u16, frames: usize, sound: impl Fn(usize, usize) -> f32) {
    let mut bytes = Vec::from(*b"RIFF\xff\xff\xff\xffWAVEfmt ");
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&0xFFFEu16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 3).to_le_bytes());
    bytes.extend_from_slice(&(channels * 3).to_le_bytes());
    bytes.extend_from_slice(&[24, 0, 22, 0, 24, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
    bytes.extend_from_slice(b"data\xff\xff\xff\xff");
    for frame in 0..frames {
        for channel in 0..usize::from(channels) {
            let value = (sound(frame, channel) * 8_388_608.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
            bytes.extend_from_slice(&value.to_le_bytes()[..3]);
        }
    }
    fs::write(path, bytes).unwrap();
}

fn note(rate: u32, hertz: f32, level: f32) -> impl Fn(usize, usize) -> f32 {
    move |frame, _| level * (TAU * hertz * frame as f32 / rate as f32).sin()
}

/// The note a sound holds, in Hz, by its zero crossings while it sounds.
fn pitch(sound: &[f32]) -> f32 {
    let loud: Vec<usize> = (0..sound.len()).filter(|&at| sound[at].abs() > 0.02).collect();
    let (first, last) = (loud[0], loud[loud.len() - 1]);
    let crossings = (first..last).filter(|&at| sound[at] < 0.0 && sound[at + 1] >= 0.0).count();
    crossings as f32 * RATE as f32 / (last - first) as f32
}

fn level(sound: &[f32]) -> f32 {
    (sound.iter().map(|s| s * s).sum::<f32>() / sound.len() as f32).sqrt()
}

#[test]
fn a_track_at_any_rate_with_any_channels_is_heard_as_one_channel_at_the_engines_rate() {
    let folder = tempfile::tempdir().unwrap();
    for (rate, channels) in [(48_000, 2), (44_100, 1), (16_000, 2), (96_000, 6), (8_000, 1)] {
        let path = folder.path().join("local.wav");
        track(&path, rate, channels, rate as usize * 3, note(rate, 440.0, 0.2));
        let sound = heard(&path, None).unwrap();
        assert!(
            (sound.len() as i64 - 3 * RATE as i64).abs() <= 2,
            "{rate} Hz: {} samples for three seconds",
            sound.len()
        );
        assert!((pitch(&sound) - 440.0).abs() < 3.0, "{rate} Hz: heard at {} Hz", pitch(&sound));
        let steady = &sound[RATE / 2..sound.len() - RATE / 2];
        assert!(
            (level(steady) - 0.2 / 2f32.sqrt()).abs() < 0.01,
            "{rate} Hz: level {}",
            level(steady)
        );
    }
}

#[test]
fn what_an_engine_cannot_hold_is_removed_and_does_not_fold_back_as_a_voice() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    // 15 kHz at 48,000 samples a second: dropped two samples in three as it is, a 1 kHz note.
    track(&path, 48_000, 1, 48_000, note(48_000, 15_000.0, 0.5));
    assert!(level(&heard(&path, None).unwrap()) < 0.005);
}

#[test]
fn a_track_is_placed_on_the_sessions_clock_by_its_first_point() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote.wav");
    track(&path, 48_000, 2, 48_000, |_, _| 0.25);
    let starts = |sound: &[f32]| sound.iter().position(|sample| sample.abs() > 0.1).unwrap() as f64 / RATE as f64;

    // Its first frame heard a second and a half after the session started.
    let late = heard(&path, Some((0, 1.5))).unwrap();
    assert!((starts(&late) - 1.5).abs() < 0.005 && late.len() == RATE * 5 / 2);
    // A point further in says the same of frame 0: frame 48,000 at 2 s is frame 0 at 1 s.
    assert!((starts(&heard(&path, Some((48_000, 2.0))).unwrap()) - 1.0).abs() < 0.005);
    // Started a quarter of a second before the session: that much is left out.
    let early = heard(&path, Some((0, -0.25))).unwrap();
    assert!((early.len() as i64 - RATE as i64 * 3 / 4).abs() <= 2 && early[0].abs() > 0.1);
    // No point at all: it starts with the session.
    assert!(starts(&heard(&path, None).unwrap()) < 0.005);

    fs::write(
        folder.path().join("clock.jsonl"),
        "{\"track\":\"local\",\"sample\":0,\"at\":0.02}\n{\"track\":\"remote\",\"silence\":[0,480]}\n\
         {\"track\":\"remote\",\"sample\":480,\"at\":0.07}\n{\"track\":\"remote\",\"sample\":240480,\"at\":5.07}\n{\"track\":\"rem",
    )
    .unwrap();
    assert_eq!(first_point(folder.path(), "remote"), Some((480, 0.07)));
    assert_eq!(first_point(folder.path(), "local"), Some((0, 0.02)));
    assert_eq!(first_point(folder.path(), "other"), None);
}

#[test]
fn a_track_still_written_or_cut_by_a_crash_is_heard_to_its_last_whole_frame() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    track(&path, 48_000, 2, 48_000, note(48_000, 440.0, 0.2));
    let whole = heard(&path, None).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes.truncate(bytes.len() - 4); // the last frame lost two of its three bytes on one channel
    fs::write(&path, bytes).unwrap();
    let cut = heard(&path, None).unwrap();
    assert!(whole.len() - cut.len() <= 1 && cut[..] == whole[..cut.len()]);
}

#[test]
fn a_file_that_is_not_a_track_is_said_and_not_converted() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let says = |expected: &str| {
        let said = heard(&path, None).unwrap_err().to_string();
        assert!(said.contains(expected), "{said}");
    };
    says("local.wav cannot be read");
    fs::write(&path, b"not sound").unwrap();
    says("local.wav is not a track of a session");
    // What the first version held: one channel, 16 bits.
    fs::write(
        &path,
        b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff\0\0",
    )
    .unwrap();
    says("PCM in 24 bits: it has 1 channel(s), 16000 Hz, 16 bits");
}

#[test]
fn a_session_as_delivered_is_its_tracks_and_one_it_does_not_hold_is_a_side_that_said_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let record = |tracks: &str| {
        let text = format!(
            r#"{{"contract":"session-audio/v2","started":"2026-10-14T14:05:12+02:00","stopped":"2026-10-14T14:06:12+02:00","tracks":{tracks}}}"#
        );
        fs::write(folder.path().join("session.json"), text).unwrap();
    };
    track(&folder.path().join("local.wav"), 48_000, 2, 96_000, note(48_000, 440.0, 0.2));
    track(&folder.path().join("remote.wav"), 44_100, 1, 44_100, note(44_100, 880.0, 0.2));
    fs::write(
        folder.path().join("clock.jsonl"),
        "{\"track\":\"remote\",\"sample\":0,\"at\":1.0}\n",
    )
    .unwrap();

    record(r#"{"local":{"device":"Microphone"},"remote":{"device":"Speaker"}}"#);
    let (stopped, _, [(me, local), (others, remote)]) = session(folder.path()).unwrap();
    assert!(stopped && (me, others) == (Speaker::Me, Speaker::Others));
    assert!((pitch(&local) - 440.0).abs() < 3.0 && (pitch(&remote) - 880.0).abs() < 6.0);
    assert!((local.len() as i64 - 2 * RATE as i64).abs() <= 2 && (remote.len() as i64 - 2 * RATE as i64).abs() <= 2);

    // A microphone that could not be opened: no entry, and its file — were one there — is not read.
    record(r#"{"remote":{"device":"Speaker"}}"#);
    let (_, _, [(_, local), (_, remote)]) = session(folder.path()).unwrap();
    assert!(local.is_empty() && !remote.is_empty());

    // A track the session names and does not hold is said.
    fs::remove_file(folder.path().join("remote.wav")).unwrap();
    assert!(
        session(folder.path())
            .unwrap_err()
            .to_string()
            .contains("remote.wav cannot be read")
    );
}
