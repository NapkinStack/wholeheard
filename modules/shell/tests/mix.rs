//! What the window plays of a meeting (ADR-0009): one sound, its tracks placed where the
//! session's clock puts them and added, as a WAV the loopback server serves by range. The
//! tracks are only read: the mix is made as it is asked for, and kept nowhere.

use std::{f64::consts::TAU, fs, path::Path};

use wholeheard_shell::{meeting, mix::Mix};

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
    // Read by range from anywhere, the resampled part is the same as read whole, give or take a step.
    for first in [100_001usize, 100_037, 123_457, 61_111, 139_990] {
        let piece = mix.read(44 + 2 * first as u64, 2 * 4_000).unwrap();
        let apart = piece
            .as_chunks::<2>()
            .0
            .iter()
            .zip(&mixed[first..])
            .map(|(pair, whole)| (i32::from(i16::from_le_bytes(*pair)) - i32::from(*whole)).abs())
            .max()
            .unwrap();
        assert!(apart == 0, "from frame {first}, a range differs from the whole by {apart}");
    }
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

#[test]
fn a_track_of_another_rate_is_placed_to_the_frame_and_read_the_same_by_any_range() {
    // Noise, and one burst a second in: the mix of a 44.1 kHz track is the same bytes whoever
    // reads it from where, and the burst is where the clock puts it.
    let folder = tempfile::tempdir().unwrap();
    let remote = folder.path().join("remote.wav");
    let mut seed = 7u64;
    let mut noise = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        ((seed >> 33) as f64 / f64::from(1u32 << 31) - 0.5) * 0.02
    };
    let frames: Vec<Vec<f64>> = (0..88_200).map(|n| vec![if n == 44_100 { 0.9 } else { noise() }]).collect();
    wav(&remote, 44_100, 1, 16, &frames, false);
    // A track of 48 kHz beside it, silent: the mix is at 48 kHz, the other brought up.
    let local = folder.path().join("local.wav");
    wav(&local, 48_000, 1, 16, &vec![vec![0.0]; 4_800], false);
    let mix = Mix::of(&[(local, 0.0), (remote, 0.5)]).unwrap();
    assert_eq!(mix.rate(), 48_000);
    let mixed = samples(&mix);
    let loudest = (0..mixed.len()).max_by_key(|&at| mixed[at].unsigned_abs()).unwrap();
    assert!((loudest as i64 - 72_000).abs() <= 2, "the burst is at frame {loudest}, not 72000");
    let whole = mix.read(0, mix.size()).unwrap();
    for from in [44 + 2 * 50_001u64, 44 + 2 * 61_111, 44 + 2 * 72_003, 44 + 2 * 99_999, 1_001] {
        assert!(
            mix.read(from, 4_000).unwrap() == whole[from as usize..from as usize + 4_000],
            "from byte {from}"
        );
    }
}

#[test]
fn what_no_track_is_gives_no_mix_and_no_read_breaks_it() {
    let folder = tempfile::tempdir().unwrap();
    let good = folder.path().join("good.wav");
    wav(&good, 48_000, 1, 16, &vec![vec![0.1]; 4_800], false);
    // A start that is no instant, or far beyond any meeting: the track is left out.
    for starts in [f64::INFINITY, f64::NAN, 1e15, -1e15] {
        assert!(Mix::of(&[(good.clone(), starts)]).is_none(), "{starts}");
    }
    // Rates no device gives, an extensible format whose `fmt ` is too short to say its own,
    // a block that is not its channels' samples: not tracks.
    let header = |rate: u32, tag: u16, block: u16, bits: u16, extra: &[u8]| {
        let mut file = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        file.extend_from_slice(&(16 + extra.len() as u32).to_le_bytes());
        file.extend_from_slice(&tag.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&rate.to_le_bytes());
        file.extend_from_slice(&rate.wrapping_mul(u32::from(block)).to_le_bytes());
        file.extend_from_slice(&block.to_le_bytes());
        file.extend_from_slice(&bits.to_le_bytes());
        file.extend_from_slice(extra);
        file.extend_from_slice(b"data\0\0\0\0");
        file.extend_from_slice(&[1u8; 400]);
        file
    };
    for (what, bytes) in [
        ("a rate of 1 Hz", header(1, 1, 2, 16, &[])),
        ("a rate past any device's", header(3_000_000_000, 1, 2, 16, &[])),
        ("extensible, its fmt too short", header(48_000, 0xFFFE, 2, 16, &[])),
        ("a block wider than its sample", header(48_000, 1, 4, 16, &[])),
    ] {
        let path = folder.path().join("odd.wav");
        fs::write(&path, bytes).unwrap();
        assert!(Mix::of(&[(path, 0.0)]).is_none(), "{what}");
    }
    // Read past its end, or nothing: nothing, never a panic.
    let mix = Mix::of(&[(good, 0.0)]).unwrap();
    assert!(mix.read(mix.size() + 10, 5).unwrap().is_empty());
    assert!(mix.read(3, 0).unwrap().is_empty());
}

/// A session of one track at 48 kHz: zeros, but for a tone 0.2 s long at each of `tones`, frames
/// of the file; `lines`, its `clock.jsonl`.
fn gapped(folder: &Path, rate: u32, frames: usize, tones: &[usize], lines: &str) {
    let mut sound = vec![vec![0.0; 2]; frames];
    for &at in tones {
        for (n, frame) in tone(1000.0, rate, 0.2, 0.5, 2).into_iter().enumerate() {
            sound[at + n] = frame;
        }
    }
    wav(&folder.join("local.wav"), rate, 2, 24, &sound, false);
    fs::write(
        folder.join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-14T14:05:12+02:00","tracks":{"local":{"device":"Mic","changed":[{"at":12.0,"device":null},{"at":20.0,"device":"Mic"}]}}}"#,
    )
    .unwrap();
    fs::write(folder.join("clock.jsonl"), lines).unwrap();
}

/// Each second at which the sound starts, in the mix.
fn onsets(mix: &Mix) -> Vec<f64> {
    let mut found = Vec::new();
    let mut quiet = true;
    let mut last = -1.0;
    for (n, sample) in samples(mix).iter().enumerate() {
        let at = n as f64 / f64::from(mix.rate());
        let loud = sample.unsigned_abs() > 3000;
        if loud && quiet && at - last > 0.1 {
            found.push(at);
        }
        if loud {
            last = at;
        }
        quiet = !loud;
    }
    found
}

fn mixed(folder: &Path) -> Mix {
    let tracks: Vec<_> = meeting::tracks(folder).into_iter().map(|track| (track.path, track.runs)).collect();
    Mix::placed(&tracks).unwrap()
}

#[test]
fn after_a_gap_a_sound_is_heard_where_the_clock_puts_it_and_the_track_is_left_as_it_was() {
    let folder = tempfile::tempdir().unwrap();
    let rate = 48_000;
    // Stopped at 12 s, frame 576 000; zeros up to 7 s of the 8 s the clock leaves, as the contract
    // says they stop short; its next point at 20 s, frame 912 000. The tones: at 5 s and at 26 s, not as
    // far into its run as the first is into the file, so that a run read from the file's start is seen.
    let lines = "{\"track\":\"local\",\"sample\":0,\"at\":0.0}\n\
                 {\"track\":\"local\",\"sample\":480000,\"at\":10.0}\n\
                 {\"track\":\"local\",\"silence\":[576000,912000]}\n\
                 {\"track\":\"local\",\"sample\":912000,\"at\":20.0}\n\
                 {\"track\":\"local\",\"sample\":1392000,\"at\":30.0}\n";
    gapped(folder.path(), rate, 1_440_000, &[240_000, 1_200_000], lines);
    let before = fs::read(folder.path().join("local.wav")).unwrap();
    let heard = onsets(&mixed(folder.path()));
    assert_eq!(heard.len(), 2, "{heard:?}");
    assert!((heard[0] - 5.0).abs() < 0.05, "before the gap: {heard:?}");
    assert!((heard[1] - 26.0).abs() < 0.05, "after the gap: {heard:?}");
    assert_eq!(
        fs::read(folder.path().join("local.wav")).unwrap(),
        before,
        "a track is never changed (ADR-0004)"
    );
}

#[test]
fn a_track_of_a_lower_rate_is_placed_again_too() {
    let folder = tempfile::tempdir().unwrap();
    let rate = 16_000;
    let lines = "{\"track\":\"local\",\"sample\":0,\"at\":0.0}\n\
                 {\"track\":\"local\",\"silence\":[64000,96000]}\n\
                 {\"track\":\"local\",\"sample\":96000,\"at\":7.0}\n";
    gapped(folder.path(), rate, 160_000, &[16_000, 120_000], lines);
    // Its rate raised to that of a second track, silent.
    fs::write(folder.path().join("session.json"), r#"{"contract":"session-audio/v2","started":"2026-10-14T14:05:12+02:00","tracks":{"local":{"device":"Mic"},"remote":{"device":"Speaker"}}}"#).unwrap();
    wav(&folder.path().join("remote.wav"), 48_000, 1, 16, &vec![vec![0.0]; 48_000], false);
    let heard = onsets(&mixed(folder.path()));
    assert!((heard[0] - 1.0).abs() < 0.05 && (heard[1] - 8.5).abs() < 0.05, "{heard:?}");
}

#[test]
fn a_clock_moved_by_little_leaves_the_sound_whole_and_one_put_back_is_never_heard_twice() {
    let folder = tempfile::tempdir().unwrap();
    let rate = 48_000;
    // 10 ms off the line: no hole of 10 ms to hear; the frames sit one after the other.
    let lines = "{\"track\":\"local\",\"sample\":0,\"at\":0.0}\n{\"track\":\"local\",\"sample\":96000,\"at\":2.01}\n";
    let long = tone(1000.0, rate, 3.9, 0.5, 2);
    wav(&folder.path().join("local.wav"), rate, 2, 24, &long, false);
    fs::write(
        folder.path().join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-14T14:05:12+02:00","tracks":{"local":{"device":"Mic"}}}"#,
    )
    .unwrap();
    fs::write(folder.path().join("clock.jsonl"), lines).unwrap();
    let whole = samples(&mixed(folder.path()));
    assert_eq!(whole.len(), long.len(), "nothing added");
    assert!(whole[95_000..97_000].iter().any(|s| s.unsigned_abs() > 3000));
    let quiet = whole[95_000..97_000]
        .windows(48)
        .filter(|w| w.iter().all(|s| s.unsigned_abs() < 100))
        .count();
    assert_eq!(quiet, 0, "no hole");

    // Put 1 s back: the second run takes its place from there, the first stops; never added.
    let lines = "{\"track\":\"local\",\"sample\":0,\"at\":0.0}\n{\"track\":\"local\",\"sample\":96000,\"at\":1.0}\n";
    fs::write(folder.path().join("clock.jsonl"), lines).unwrap();
    let back = samples(&mixed(folder.path()));
    assert_eq!(back.len(), long.len() - 48_000, "ends where the clock ends it");
    assert!(back.iter().all(|s| s.unsigned_abs() <= 16_400), "never two sounds added");
}

const STARTED: &str = r#"{"contract":"session-audio/v2","started":"2026-10-14T14:05:12+02:00","tracks":{"local":{"device":"Mic"}}}"#;

/// A session of one track of `rate` 48 kHz whose tones are at these file seconds, its clock as given.
fn timed(folder: &Path, seconds: f64, tones: &[f64], lines: &str) {
    let rate = 48_000u32;
    let at = |s: f64| (s * f64::from(rate)) as usize;
    gapped(folder, rate, at(seconds), &tones.iter().map(|s| at(*s)).collect::<Vec<_>>(), lines);
    fs::write(folder.join("session.json"), STARTED).unwrap();
}

fn point(frame: u64, at: f64) -> String {
    format!("{{\"track\":\"local\",\"sample\":{frame},\"at\":{at}}}\n")
}

#[test]
fn a_point_off_by_more_than_twenty_milliseconds_places_its_frames_one_by_less_does_not() {
    // A tone at file second 3, one run from file second 2 on. Off by 30 ms: heard 30 ms later.
    let folder = tempfile::tempdir().unwrap();
    let off = |seconds: f64| point(0, 0.0) + &point(96_000, 2.0 + seconds);
    timed(folder.path(), 4.0, &[3.0], &off(0.030));
    let heard = onsets(&mixed(folder.path()));
    assert!((heard[0] - 3.03).abs() < 0.002, "{heard:?}");
    // Off by 10 ms: where it was.
    timed(folder.path(), 4.0, &[3.0], &off(0.010));
    let heard = onsets(&mixed(folder.path()));
    assert!((heard[0] - 3.0).abs() < 0.002, "{heard:?}");
}

#[test]
fn a_gap_under_a_second_leaves_no_silence_in_the_clock_and_is_placed_all_the_same() {
    // Under 1.5 s the contract says no `silence`: the next point alone shows it. What comes after is
    // the next run's, never played by the run before it in the hole.
    let folder = tempfile::tempdir().unwrap();
    timed(folder.path(), 4.0, &[2.1], &(point(0, 0.0) + &point(96_000, 2.4)));
    let heard = onsets(&mixed(folder.path()));
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert!((heard[0] - 2.5).abs() < 0.002, "{heard:?}");
}

#[test]
fn every_run_after_it_stops_a_run_never_two_sounds_and_a_run_with_nothing_left_counts_for_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let rate = 48_000u32;
    let lines = point(0, 1.0) + &point(480_000, 21.0) + &point(528_000, 6.0);
    let long = tone(1000.0, rate, 14.0, 0.5, 2);
    wav(&folder.path().join("local.wav"), rate, 2, 24, &long, false);
    fs::write(folder.path().join("session.json"), STARTED).unwrap();
    fs::write(folder.path().join("clock.jsonl"), lines).unwrap();
    let all = samples(&mixed(folder.path()));
    assert!(all.iter().all(|s| s.unsigned_abs() <= 16_400), "never two sounds added");
    // The first stops at 6 s of the clock, the last plays 3 s from there: nothing plays at 21 s.
    assert_eq!(all.len(), 9 * 48_000, "ends with the last sound the clock keeps");
}

#[test]
fn the_earliest_run_after_it_stops_a_run_wherever_it_stands_in_the_clock() {
    let folder = tempfile::tempdir().unwrap();
    let rate = 48_000u32;
    // The first run holds 10 s; the second starts at 5 s of the clock, the last at 20 s.
    let lines = point(0, 0.0) + &point(480_000, 5.0) + &point(720_000, 20.0);
    let long = tone(1000.0, rate, 16.0, 0.5, 2);
    wav(&folder.path().join("local.wav"), rate, 2, 24, &long, false);
    fs::write(folder.path().join("session.json"), STARTED).unwrap();
    fs::write(folder.path().join("clock.jsonl"), lines).unwrap();
    assert!(
        samples(&mixed(folder.path())).iter().all(|s| s.unsigned_abs() <= 16_400),
        "never two sounds added"
    );
}

#[test]
fn a_clock_read_in_any_order_a_frame_said_twice_a_point_beyond_any_meeting_or_the_file_change_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let clean = point(0, 0.0) + &point(96_000, 3.0);
    timed(folder.path(), 4.0, &[2.5], &clean);
    let expected = samples(&mixed(folder.path()));
    for lines in [
        point(96_000, 3.0) + &point(0, 0.0),
        clean.clone() + &point(96_000, 9.0),
        clean.clone() + &point(144_000, 9e9),
        clean.clone() + &point(9_000_000, 3.0 + 9_000_000.0 / 48_000.0 + 5.0),
    ] {
        fs::write(folder.path().join("clock.jsonl"), &lines).unwrap();
        assert_eq!(samples(&mixed(folder.path())), expected, "{lines}");
    }
}

#[test]
fn the_same_track_placed_otherwise_is_another_mix_and_an_empty_track_still_is_one() {
    let folder = tempfile::tempdir().unwrap();
    timed(folder.path(), 4.0, &[], "");
    let path = folder.path().join("local.wav");
    // The same run's place on the clock and length, from another frame of the file.
    let one = Mix::placed(&[(path.clone(), vec![(0, 0.0), (96_000, 3.0)])]).unwrap();
    let other = Mix::placed(&[(path.clone(), vec![(0, 0.0), (48_000, 3.0), (144_000, 4.0)])]).unwrap();
    assert!(!one.same(&other));
    assert!(one.same(&Mix::placed(&[(path.clone(), vec![(0, 0.0), (96_000, 3.0)])]).unwrap()));
    // Each run in the same place and as long, one read from another frame: a clock written again.
    timed(folder.path(), 9.0, &[], "");
    let path = folder.path().join("local.wav");
    let here = Mix::placed(&[(path.clone(), vec![(0, 0.0), (96_000, 1.0), (200_000, 2.0)])]).unwrap();
    let there = Mix::placed(&[(path, vec![(0, 0.0), (72_000, 1.0), (200_000, 2.0)])]).unwrap();
    assert!(!here.same(&there));
    let empty = folder.path().join("empty.wav");
    wav(&empty, 48_000, 1, 16, &[], false);
    assert_eq!(Mix::placed(&[(empty, vec![(0, 0.0)])]).unwrap().frames(), 0);
}
