//! What `session-audio/v2` promises of a session's folder, tried on sound made here: no
//! audio device, and no recording of anyone (rules/security.md, 2).

use std::{f32::consts::TAU, fs, path::Path};

use summit_capture::v2::Track;

/// A tone at `level` of full scale, resting at `rest`, each channel a little weaker than the
/// one before: channels that differ show a mix.
fn tone(rate: u32, channels: usize, seconds: f32, level: f32, rest: f32) -> Vec<f32> {
    (0..(rate as f32 * seconds) as usize)
        .flat_map(|n| {
            let sound = (TAU * 440.0 * n as f32 / rate as f32).sin();
            (0..channels).map(move |channel| rest + level * sound / (channel + 1) as f32)
        })
        .collect()
}

/// A track read as the contract tells a reader to: the audio found by the file's chunks, the
/// lengths ignored, to the end of the file. Returns the rate, the channels and the samples.
fn read(path: &Path) -> (u32, usize, Vec<f32>) {
    let bytes = fs::read(path).unwrap();
    assert_eq!((&bytes[0..4], &bytes[8..12]), (&b"RIFF"[..], &b"WAVE"[..]));
    let (mut at, mut format) = (12, None);
    while &bytes[at..at + 4] != b"data" {
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if &bytes[at..at + 4] == b"fmt " {
            format = Some(bytes[at + 8..at + 8 + size].to_vec());
        }
        at += 8 + size;
    }
    let format = format.expect("a format before the audio");
    assert_eq!(u16::from_le_bytes([format[0], format[1]]), 0xFFFE, "the extensible form");
    assert_eq!(u16::from_le_bytes([format[14], format[15]]), 24, "bits");
    assert_eq!(format[24..26], [1, 0], "PCM");
    let samples = bytes[at + 8..]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0)
        .collect();
    (
        u32::from_le_bytes(format[4..8].try_into().unwrap()),
        usize::from(u16::from_le_bytes([format[2], format[3]])),
        samples,
    )
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
fn a_track_holds_what_its_source_delivered_at_its_rate_with_its_channels() {
    let folder = tempfile::tempdir().unwrap();
    for (rate, channels) in [(48_000, 2), (44_100, 1), (16_000, 2), (96_000, 6)] {
        let path = folder.path().join(format!("{rate}-{channels}.wav"));
        // Resting at a third of full scale, as D1's microphone did: it stays there.
        let sound = tone(rate, channels, 2.0, 0.2, 0.33);
        let frames = recorded(&path, rate, channels, &sound).finish().unwrap();
        assert_eq!(frames as usize, sound.len() / channels);

        let (read_rate, read_channels, samples) = read(&path);
        assert_eq!((read_rate, read_channels), (rate, channels));
        assert_eq!(samples.len(), sound.len());
        let furthest = samples.iter().zip(&sound).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(furthest <= 0.5 / 8_388_608.0, "{rate} Hz, {channels} channels: off by {furthest}");
    }
}

#[test]
fn a_voice_at_a_ten_thousandth_of_full_scale_keeps_its_shape() {
    // D1's weak microphone: in 16 bits it has seven levels to live on, in 24 hundreds.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("weak.wav");
    let sound = tone(48_000, 1, 1.0, 0.0001, 0.0);
    recorded(&path, 48_000, 1, &sound).finish().unwrap();
    let steps: std::collections::BTreeSet<i32> = read(&path).2.iter().map(|s| (s * 8_388_608.0).round() as i32).collect();
    assert!(steps.len() > 300, "{} levels for a weak voice", steps.len());
}

#[test]
fn a_track_reads_while_it_is_written_and_after_a_kill() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let sound = tone(48_000, 2, 1.0, 0.2, 0.0);
    let track = recorded(&path, 48_000, 2, &sound);
    // Never finished: what a kill leaves. Everything pushed is already in the file.
    assert_eq!(read(&path).2.len(), sound.len());
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes[4..8], [0xFF; 4], "a length that says: not known yet");
    drop(track);
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn a_sample_once_written_never_changes_and_only_the_header_is_written_twice() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let mut track = recorded(&path, 48_000, 2, &tone(48_000, 2, 0.5, 0.2, 0.0));
    let before = fs::read(&path).unwrap();
    track.push(&tone(48_000, 2, 0.5, 0.1, 0.0)).unwrap();
    assert_eq!(fs::read(&path).unwrap()[..before.len()], before[..]);
    let frames = track.finish().unwrap();
    let after = fs::read(&path).unwrap();
    assert_eq!(after[68..before.len()], before[68..]);
    assert_eq!(u32::from_le_bytes(after[64..68].try_into().unwrap()) as u64, frames * 2 * 3);
    assert_eq!(u32::from_le_bytes(after[4..8].try_into().unwrap()) as usize, after.len() - 8);
}

#[test]
fn a_frame_delivered_in_two_pieces_is_written_once_whole_and_no_channel_moves() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let mut track = Track::create(&path, 48_000, 2).unwrap();
    // Left goes up, right goes down, five samples at a time: every other piece cuts a frame.
    let sound: Vec<f32> = (1..=20).flat_map(|n| [n as f32 / 100.0, -(n as f32) / 100.0]).collect();
    for piece in sound.chunks(5) {
        track.push(piece).unwrap();
    }
    assert_eq!(track.frames(), 20);
    let samples = read(&path).2;
    assert!(samples.chunks(2).all(|frame| frame[0] > 0.0 && frame[1] < 0.0), "a channel moved");
    track.push(&[0.5]).unwrap();
    assert_eq!(track.frames(), 20, "half a frame is not a frame yet");
    // The source stops there and comes back: its half frame is not the start of the next.
    track.silence_to(30).unwrap();
    track.push(&[0.3, -0.3]).unwrap();
    let samples = read(&path).2;
    assert_eq!(samples.len(), 62);
    assert!(samples[60] > 0.0 && samples[61] < 0.0, "a channel moved after a gap");
}

#[test]
fn what_no_file_of_24_bits_can_hold_is_written_as_the_nearest_it_can() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let mut track = Track::create(&path, 48_000, 1).unwrap();
    track.push(&[1.5, -1.5, 1.0, -1.0, f32::NAN, f32::INFINITY, 0.5]).unwrap();
    let top = 8_388_607.0 / 8_388_608.0;
    assert_eq!(read(&path).2, [top, -1.0, top, -1.0, 0.0, top, 0.5]);
    assert!(track.loudest() <= 2.0, "what is beyond full scale is measured as full scale");
    // What is no number is not a sound: a source at rest stays silent through it.
    let mut resting = Track::create(&folder.path().join("resting.wav"), 48_000, 1).unwrap();
    resting.push(&[0.33, 0.33, f32::NAN, 0.33, 0.33]).unwrap();
    assert!(resting.loudest() < 0.001);
    assert!(
        Track::create(&folder.path().join("wide.wav"), 48_000, 30_000).is_err(),
        "more channels than a header can say"
    );
}

#[test]
fn a_write_the_system_refuses_leaves_the_track_whole_frames() {
    // A device that is full: every write fails. Nothing of a refused push stays in the track.
    let Ok(mut track) = Track::create(std::path::Path::new("/dev/full"), 48_000, 2) else {
        return; // No such device on this system: the promise is tried where there is one.
    };
    assert!(track.push(&[0.1, 0.1]).is_err());
    assert_eq!(track.frames(), 0);
    assert!(track.silence_to(48_000).is_err());
    assert_eq!(track.frames(), 0);
}

#[test]
fn zeros_written_for_a_source_that_gave_nothing_are_said() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote.wav");
    let mut track = recorded(&path, 48_000, 2, &tone(48_000, 2, 0.5, 0.2, 0.0));
    assert_eq!(track.silence_to(10_000).unwrap(), None, "never shortens");
    assert_eq!(track.silence_to(48_000).unwrap(), Some((24_000, 48_000)));
    assert_eq!(track.frames(), 48_000);
    let samples = read(&path).2;
    assert_eq!(samples.len(), 96_000);
    assert!(samples[48_000..].iter().all(|sample| *sample == 0.0));
}

#[test]
fn how_loud_a_source_is_is_measured_from_where_it_rests_and_the_track_is_untouched() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    // D1's microphone: silent, and resting at a third of full scale.
    let mut track = recorded(&path, 48_000, 2, &vec![0.33; 9600]);
    assert!(track.loudest() < 0.001, "a source at rest is told loud");
    assert!(
        read(&path).2.iter().all(|sample| (sample - 0.33).abs() < 1e-6),
        "the track was centred"
    );
    // A voice on one channel of two, around that rest.
    let voice: Vec<f32> = tone(48_000, 1, 0.5, 0.2, 0.33)
        .into_iter()
        .flat_map(|sample| [0.33, sample])
        .collect();
    track.push(&voice).unwrap();
    assert!((track.loudest() - 0.2).abs() < 0.02);
    assert_eq!(track.loudest(), 0.0, "since the last call");
}
