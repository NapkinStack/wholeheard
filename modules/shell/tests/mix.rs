//! What the window plays of a meeting (ADR-0009): one sound, its tracks placed where the
//! session's clock puts them and added, as a WAV the loopback server serves by range. The
//! tracks are only read: the mix is made as it is asked for, and kept nowhere.

use std::{f64::consts::TAU, fs, path::Path};

use wholeheard_shell::mix::Mix;

/// A WAV file: PCM of `bits` (16, 24 or 32), or 32-bit float when `bits` is 0; `frames`
/// interleaved; its lengths said as given, or as a recording killed leaves them.
fn wav(path: &Path, rate: u32, channels: u16, bits: u16, frames: &[Vec<f64>], killed: bool) {
    let (format, width) = if bits == 0 { (3u16, 4u16) } else { (1, bits / 8) };
    let mut data = Vec::new();
    for frame in frames {
        assert_eq!(frame.len(), usize::from(channels));
        for &sample in frame {
            match bits {
                0 => data.extend_from_slice(&(sample as f32).to_le_bytes()),
                16 => data.extend_from_slice(&((sample * 32767.0).round() as i16).to_le_bytes()),
                24 => data.extend_from_slice(&((sample * 8_388_607.0).round() as i32).to_le_bytes()[..3]),
                _ => data.extend_from_slice(&((sample * 2_147_483_647.0).round() as i32).to_le_bytes()),
            }
        }
    }
    let said = |length: usize| if killed { u32::MAX } else { length as u32 };
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&said(36 + data.len()).to_le_bytes());
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&16u32.to_le_bytes());
    file.extend_from_slice(&format.to_le_bytes());
    file.extend_from_slice(&channels.to_le_bytes());
    file.extend_from_slice(&rate.to_le_bytes());
    file.extend_from_slice(&(rate * u32::from(channels * width)).to_le_bytes());
    file.extend_from_slice(&(channels * width).to_le_bytes());
    file.extend_from_slice(&(width * 8).to_le_bytes());
    file.extend_from_slice(b"data");
    file.extend_from_slice(&said(data.len()).to_le_bytes());
    file.extend_from_slice(&data);
    fs::write(path, file).unwrap();
}

fn tone(hertz: f64, rate: u32, seconds: f64, level: f64, channels: usize) -> Vec<Vec<f64>> {
    (0..(seconds * f64::from(rate)) as usize)
        .map(|n| vec![level * (TAU * hertz * n as f64 / f64::from(rate)).sin(); channels])
        .collect()
}

/// The mix's samples, read whole, after its 44 bytes of header.
fn samples(mix: &Mix) -> Vec<i16> {
    let all = mix.read(0, mix.size()).unwrap();
    all[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

/// The note a stretch of samples holds, in Hz, by its rising zero crossings.
fn note(samples: &[i16], rate: u32) -> f64 {
    let rising = samples.windows(2).filter(|pair| pair[0] < 0 && pair[1] >= 0).count();
    rising as f64 * f64::from(rate) / samples.len() as f64
}

#[test]
fn two_tracks_are_one_sound_each_where_the_clock_puts_it() {
    let folder = tempfile::tempdir().unwrap();
    let (local, remote) = (folder.path().join("local.wav"), folder.path().join("remote.wav"));
    // Local: two channels of 24 bits, the first louder; remote: one of 16 bits, half a second in.
    let made: Vec<Vec<f64>> = (0..48_000).map(|n| vec![0.4 * ((n % 100) as f64 / 100.0), 0.2]).collect();
    wav(&local, 48_000, 2, 24, &made, false);
    wav(&remote, 48_000, 1, 16, &vec![vec![0.25]; 48_000], false);
    let mix = Mix::of(&[(local, 0.0), (remote, 0.5)]).unwrap();

    // One channel of 16 bits at the tracks' rate, as long as the last track ends: 1.5 s.
    assert_eq!((mix.rate(), mix.frames()), (48_000, 72_000));
    assert_eq!(mix.size(), 44 + 72_000 * 2);
    let head = mix.read(0, 44).unwrap();
    assert_eq!(&head[0..4], b"RIFF");
    assert_eq!(&head[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes([head[22], head[23]]), 1, "one channel");
    assert_eq!(u32::from_le_bytes(head[24..28].try_into().unwrap()), 48_000);
    assert_eq!(u16::from_le_bytes([head[34], head[35]]), 16);
    assert_eq!(
        u32::from_le_bytes(head[40..44].try_into().unwrap()),
        72_000 * 2,
        "the data's length, true"
    );

    // Each frame: a track's channels averaged, the tracks added.
    let mixed = samples(&mix);
    let near = |at: usize, wanted: f64| {
        let got = f64::from(mixed[at]) / 32767.0;
        assert!((got - wanted).abs() < 0.001, "frame {at}: {got}, not {wanted}");
    };
    near(10, (0.4 * 0.10 + 0.2) / 2.0);
    near(24_010, (0.4 * 0.10 + 0.2) / 2.0 + 0.25);
    // Past local's end, remote alone; and nothing before remote's start but local.
    near(60_000, 0.25);
    near(23_990, (0.4 * 0.90 + 0.2) / 2.0);
}

#[test]
fn any_range_of_it_is_the_same_bytes_as_the_whole() {
    let folder = tempfile::tempdir().unwrap();
    let (local, remote) = (folder.path().join("local.wav"), folder.path().join("remote.wav"));
    wav(&local, 48_000, 2, 24, &tone(440.0, 48_000, 3.0, 0.3, 2), false);
    wav(&remote, 48_000, 2, 0, &tone(880.0, 48_000, 2.0, 0.3, 2), false);
    let mix = Mix::of(&[(local, 0.0), (remote, 1.25)]).unwrap();
    let whole = mix.read(0, mix.size()).unwrap();
    assert_eq!(whole.len() as u64, mix.size());
    // From the header's middle, on an odd byte, over the second track's start, to the very end.
    for (from, length) in [(0, 10), (21, 100), (1001, 77_777), (44 + 2 * 59_999, 9), (mix.size() - 3, 3)] {
        assert_eq!(
            mix.read(from, length).unwrap(),
            &whole[from as usize..(from + length) as usize],
            "{from}+{length}"
        );
    }
}

#[test]
fn a_track_of_another_rate_is_heard_at_its_own_pitch_and_length() {
    let folder = tempfile::tempdir().unwrap();
    let (local, remote) = (folder.path().join("local.wav"), folder.path().join("remote.wav"));
    wav(&local, 48_000, 1, 16, &tone(440.0, 48_000, 1.0, 0.3, 1), false);
    wav(&remote, 44_100, 2, 24, &tone(1000.0, 44_100, 2.0, 0.3, 2), false);
    let mix = Mix::of(&[(local, 0.0), (remote, 1.0)]).unwrap();
    // The highest rate is the mix's: nothing is brought down.
    assert_eq!(mix.rate(), 48_000);
    assert!((mix.frames() as i64 - 3 * 48_000).abs() <= 2, "{}", mix.frames());
    let mixed = samples(&mix);
    let local_note = note(&mixed[4_800..43_200], 48_000);
    let remote_note = note(&mixed[52_800..139_200], 48_000);
    assert!((local_note - 440.0).abs() < 3.0, "{local_note}");
    assert!((remote_note - 1000.0).abs() < 3.0, "{remote_note}");
    // Read by range, the resampled part is the same as read whole, give or take a step.
    let piece = mix.read(44 + 2 * 100_001, 2 * 4_000).unwrap();
    let apart = piece
        .as_chunks::<2>()
        .0
        .iter()
        .zip(&mixed[100_001..104_001])
        .map(|(pair, whole)| (i32::from(i16::from_le_bytes([pair[0], pair[1]])) - i32::from(*whole)).abs())
        .max()
        .unwrap();
    assert!(apart <= 2, "a range differs from the whole by {apart}");
}

#[test]
fn a_track_cut_by_a_crash_is_read_to_its_end_and_no_track_is_no_mix() {
    let folder = tempfile::tempdir().unwrap();
    let local = folder.path().join("local.wav");
    // Its header says lengths it never had; its last frame is cut.
    wav(&local, 16_000, 1, 32, &vec![vec![0.5]; 16_000], true);
    let mut bytes = fs::read(&local).unwrap();
    bytes.extend_from_slice(&[1, 2]);
    fs::write(&local, bytes).unwrap();
    let mix = Mix::of(&[(local.clone(), 0.0)]).unwrap();
    assert_eq!((mix.rate(), mix.frames()), (16_000, 16_000));
    assert!(samples(&mix).iter().all(|&sample| (sample - 16_384).abs() <= 1));

    // Nothing that reads as a track: no mix. A track that starts before the session, cut there.
    fs::write(folder.path().join("not.wav"), b"RIFF....WAVEjunk").unwrap();
    assert!(Mix::of(&[(folder.path().join("not.wav"), 0.0)]).is_none());
    assert!(Mix::of(&[]).is_none());
    let early = Mix::of(&[(local, -0.25)]).unwrap();
    assert_eq!(early.frames(), 12_000);
}
