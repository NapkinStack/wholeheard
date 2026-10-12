//! What `session-audio/v1` promises of a session's folder, tried on sound made here: no
//! audio device, and no recording of anyone (rules/security.md, 2).

use std::{f32::consts::TAU, fs, path::Path};

use summit_capture::{Heard, RATE, SILENCE, Track, Watch, new_folder, write_session};

/// A tone of `hertz` at `level` of full scale, resting at `rest`, as a source would deliver it.
fn tone(rate: u32, channels: usize, seconds: f32, hertz: f32, level: f32, rest: f32) -> Vec<f32> {
    (0..(rate as f32 * seconds) as usize)
        .flat_map(|n| std::iter::repeat_n(rest + level * (TAU * hertz * n as f32 / rate as f32).sin(), channels))
        .collect()
}

/// The samples of a file, read as the contract tells a reader to: past the header, to the end.
fn samples(path: &Path) -> Vec<f32> {
    fs::read(path).unwrap()[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0)
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Feeds a track in pieces the size a system delivers, and leaves it unfinished.
fn recorded(path: &Path, rate: u32, channels: usize, sound: &[f32]) -> Track {
    let mut track = Track::create(path, rate, channels as u16).unwrap();
    for piece in sound.chunks(480 * channels) {
        track.push(piece).unwrap();
    }
    track
}

#[test]
fn a_source_at_any_rate_becomes_one_channel_at_sixteen_thousand() {
    let folder = tempfile::tempdir().unwrap();
    for (rate, channels) in [(48_000, 2), (44_100, 1), (16_000, 2), (96_000, 6)] {
        let path = folder.path().join(format!("{rate}-{channels}.wav"));
        let track = recorded(&path, rate, channels, &tone(rate, channels, 3.0, 440.0, 0.2, 0.0));
        let written = track.finish().unwrap();
        assert!(
            (written as i64 - 3 * i64::from(RATE)).abs() <= 2,
            "{rate} Hz: {written} samples for three seconds"
        );

        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), 1, "PCM");
        assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1, "one channel");
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), RATE);
        assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16, "bits");
        assert_eq!(
            u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as u64,
            written * 2,
            "the data's length, once finished"
        );

        // The same tone comes out: as loud, and at the same pitch.
        let heard = &samples(&path)[RATE as usize..];
        assert!((rms(heard) - 0.2 / 2f32.sqrt()).abs() < 0.01, "{rate} Hz: level {}", rms(heard));
        let crossings = heard.windows(2).filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0).count();
        assert!(
            (crossings as i64 - 880).abs() <= 2,
            "{rate} Hz: {crossings} cycles in two seconds of 440 Hz"
        );
    }
}

#[test]
fn a_source_resting_away_from_zero_is_centred_and_its_silence_is_silence() {
    // What a laptop's microphone did on Fedora: a third of full scale, for the whole minute.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("me.wav");
    let mut track = recorded(&path, 48_000, 2, &tone(48_000, 2, 3.0, 0.0, 0.0, -0.33));
    let heard = samples(&path);
    let loudest = heard.iter().fold(0f32, |loudest, s| loudest.max(s.abs()));
    assert!(loudest < SILENCE, "a silent room resting at -33 % is written as sound: {loudest}");
    assert!(track.loudest() < SILENCE);

    // And a voice on top of that offset is still there, around zero.
    let path = folder.path().join("voice.wav");
    recorded(&path, 48_000, 2, &tone(48_000, 2, 3.0, 300.0, 0.1, -0.33));
    let heard = &samples(&path)[RATE as usize..];
    let mean = heard.iter().sum::<f32>() / heard.len() as f32;
    assert!(mean.abs() < SILENCE, "resting level left in the file: {mean}");
    assert!(
        (rms(heard) - 0.1 / 2f32.sqrt()).abs() < 0.01,
        "the voice was changed: {}",
        rms(heard)
    );
}

#[test]
fn what_would_fold_back_is_removed_before_samples_are_dropped() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("high.wav");
    recorded(&path, 48_000, 1, &tone(48_000, 1, 2.0, 11_000.0, 0.5, 0.0));
    let heard = &samples(&path)[RATE as usize / 2..];
    assert!(rms(heard) < 0.01, "a tone above half the rate came back as sound: {}", rms(heard));
}

#[test]
fn the_file_reads_while_it_is_written_and_after_a_kill() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("others.wav");
    let mut track = Track::create(&path, 48_000, 2).unwrap();
    let second = tone(48_000, 2, 1.0, 440.0, 0.2, 0.0);
    for elapsed in 1..=3u64 {
        track.push(&second).unwrap();
        // Read by another program, the recording still open: everything pushed is there.
        let on_disk = fs::metadata(&path).unwrap().len();
        assert_eq!(on_disk, 44 + track.written() * 2);
        assert!((track.written() as i64 - (elapsed * u64::from(RATE)) as i64).abs() <= 2);
    }
    // Never finished — a kill: the header says "length unknown", and the sound reads to the end.
    std::mem::forget(track);
    let bytes = fs::read(&path).unwrap();
    assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), u32::MAX);
    let heard = samples(&path);
    assert!((heard.len() as i64 - 3 * i64::from(RATE)).abs() <= 2);
    assert!(rms(&heard[RATE as usize..]) > 0.1);
}

#[test]
fn a_sample_once_written_never_changes() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("me.wav");
    let mut track = Track::create(&path, 48_000, 1).unwrap();
    track.push(&tone(48_000, 1, 1.0, 440.0, 0.2, 0.0)).unwrap();
    let before = fs::read(&path).unwrap();
    track.push(&tone(48_000, 1, 1.0, 880.0, 0.2, 0.0)).unwrap();
    track.pad_to(3 * u64::from(RATE)).unwrap();
    let after = fs::read(&path).unwrap();
    assert_eq!(&after[44..before.len()], &before[44..], "the first second was rewritten");
}

#[test]
fn a_source_that_delivers_nothing_is_written_as_zeros_on_the_same_clock() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("others.wav");
    let mut track = Track::create(&path, 48_000, 2).unwrap();
    track.pad_to(u64::from(RATE)).unwrap();
    assert_eq!(track.written(), u64::from(RATE));
    track.pad_to(100).unwrap();
    assert_eq!(track.written(), u64::from(RATE), "padding shortened the file");
    track.finish().unwrap();
    assert!(samples(&path).iter().all(|&sample| sample == 0.0));
}

#[test]
fn a_source_opens_without_the_click_of_its_first_samples() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("me.wav");
    // Full scale from the first sample, as the microphone opened on Fedora.
    recorded(&path, 48_000, 1, &tone(48_000, 1, 1.0, 1_000.0, 1.0, 0.0));
    let heard = samples(&path);
    assert!(
        heard[..16].iter().all(|s| s.abs() < 0.05),
        "the file opens on a click: {:?}",
        &heard[..16]
    );
    assert!(rms(&heard[RATE as usize / 2..]) > 0.6);
}

#[test]
fn a_silent_source_is_said_once_after_ten_seconds_and_once_when_it_comes_back() {
    let mut watch = Watch::new(10.0);
    let mut said = Vec::new();
    for second in 0..40 {
        let loudest = if (5..25).contains(&second) { 0.0 } else { 0.2 };
        match watch.after(1.0, loudest) {
            Heard::Nothing => {}
            event => said.push((second, event)),
        }
    }
    // Silent from second 5: told at the end of second 14, ten seconds in; heard again at 25.
    assert_eq!(said, vec![(14, Heard::Silent), (25, Heard::Again)]);
    // A pause shorter than the limit is not said.
    let mut watch = Watch::new(10.0);
    assert!((0..9).all(|_| watch.after(1.0, 0.0) == Heard::Nothing));
    assert_eq!(watch.after(1.0, 0.2), Heard::Nothing);
}

#[test]
fn a_session_has_its_folder_and_says_whether_it_was_stopped() {
    let sessions = tempfile::tempdir().unwrap();
    let first = new_folder(sessions.path(), "2026-10-13_14-05").unwrap();
    let second = new_folder(sessions.path(), "2026-10-13_14-05").unwrap();
    assert_eq!(first.file_name().unwrap(), "2026-10-13_14-05");
    assert_eq!(second.file_name().unwrap(), "2026-10-13_14-05-2");

    write_session(&first, "2026-10-13T14:05:12+02:00", None, "Digital \"Mic\"", "Speaker").unwrap();
    let recording: serde_json::Value = serde_json::from_slice(&fs::read(first.join("session.json")).unwrap()).unwrap();
    assert_eq!(recording["contract"], "session-audio/v1");
    assert_eq!(recording["started"], "2026-10-13T14:05:12+02:00");
    assert_eq!(recording["sources"]["me"]["device"], "Digital \"Mic\"");
    assert!(recording.get("stopped").is_none(), "a session still recording says it was stopped");

    write_session(
        &first,
        "2026-10-13T14:05:12+02:00",
        Some("2026-10-13T15:01:40+02:00"),
        "Mic",
        "Speaker",
    )
    .unwrap();
    let stopped: serde_json::Value = serde_json::from_slice(&fs::read(first.join("session.json")).unwrap()).unwrap();
    assert_eq!(stopped["stopped"], "2026-10-13T15:01:40+02:00");
    assert!(!first.join("session.json.part").exists());
}

#[test]
fn an_hour_of_recording_fits_the_cycles_limit() {
    // Two files, 16-bit, one channel, 16 kHz: 230 MB an hour, under the 250 MB of D3.
    let an_hour = 2 * (44 + 3600 * u64::from(RATE) * 2);
    assert!(an_hour <= 250_000_000, "{an_hour} bytes an hour");
}
