//! A session recorded as `session-audio/v2` describes it, read for an engine: tracks made
//! here — notes, no voice of anyone — at the rates and with the channels a system delivers.

use std::{fs, path::Path};

use summit_transcription::{
    step::RATE,
    tracks::{Clock, clock, heard},
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
    move |frame, _| level * (std::f64::consts::TAU * f64::from(hertz) * frame as f64 / f64::from(rate)).sin() as f32
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

/// A sound without its note at `hertz`: what a conversion added that was not played.
fn beside(sound: &[f32], hertz: f32) -> Vec<f32> {
    let wave = |at: usize| std::f64::consts::TAU * f64::from(hertz) * at as f64 / RATE as f64;
    let (mut sine, mut cosine) = (0.0f64, 0.0f64);
    for (at, sample) in sound.iter().enumerate() {
        sine += f64::from(*sample) * wave(at).sin();
        cosine += f64::from(*sample) * wave(at).cos();
    }
    let (sine, cosine) = (2.0 * sine / sound.len() as f64, 2.0 * cosine / sound.len() as f64);
    let without = |(at, sample): (usize, &f32)| (f64::from(*sample) - sine * wave(at).sin() - cosine * wave(at).cos()) as f32;
    sound.iter().enumerate().map(without).collect()
}

/// A sound's middle: away from where it starts and stops.
fn steady(sound: &[f32]) -> &[f32] {
    &sound[RATE / 2..sound.len() - RATE / 2]
}

#[test]
fn a_track_at_any_rate_with_any_channels_is_heard_as_one_channel_at_the_engines_rate() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    for (rate, channels) in [
        (48_000, 2),
        (44_100, 1),
        (16_000, 2),
        (96_000, 6),
        (8_000, 1),
        (22_050, 2),
        (32_000, 1),
    ] {
        track(&path, rate, channels, rate as usize * 3, note(rate, 440.0, 0.2));
        let sound = heard(&path, &Clock::default()).unwrap();
        assert!(
            (sound.len() as i64 - 3 * RATE as i64).abs() <= 1,
            "{rate} Hz: {} samples for three seconds",
            sound.len()
        );
        assert!((pitch(&sound) - 440.0).abs() < 3.0, "{rate} Hz: heard at {} Hz", pitch(&sound));
        assert!(
            (level(steady(&sound)) - 0.2 / 2f32.sqrt()).abs() < 0.003,
            "{rate} Hz: level {}",
            level(steady(&sound))
        );
    }
}

#[test]
fn a_sound_is_where_it_was_in_time_and_channels_are_heard_together() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    for rate in [8_000u32, 44_100, 48_000, 96_000] {
        // A note that starts a second and a quarter in, at a crest.
        let from = rate as usize * 5 / 4;
        track(&path, rate, 1, rate as usize * 3, |frame, _| {
            if frame >= from {
                note(rate, 1_000.0, 0.5)(frame - from + rate as usize / 4000, 0)
            } else {
                0.0
            }
        });
        let sound = heard(&path, &Clock::default()).unwrap();
        let starts = sound.iter().position(|sample| sample.abs() > 0.25).unwrap() as f64 / RATE as f64;
        assert!(
            (starts - 1.25).abs() < 0.0005,
            "{rate} Hz: a sound at 1.25 s is heard at {starts} s"
        );
    }
    // A voice on one channel of two is heard at half its level: channels are averaged, none is dropped.
    track(&path, 48_000, 2, 144_000, |frame, channel| {
        if channel == 1 { note(48_000, 440.0, 0.4)(frame, 0) } else { 0.0 }
    });
    assert!((level(steady(&heard(&path, &Clock::default()).unwrap())) - 0.2 / 2f32.sqrt()).abs() < 0.003);
}

#[test]
fn a_note_keeps_its_level_and_nothing_is_heard_that_was_not_played() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    // High notes of a voice, at rates whose ratio to the engine's is not a whole number,
    // and at one below it: what a careless conversion mirrors into the voice.
    for (rate, hertz) in [
        (44_100, 7_000.0),
        (22_050, 7_000.0),
        (48_000, 7_000.0),
        (96_000, 7_000.0),
        (8_000, 3_000.0),
        (11_025, 5_000.0),
        // And a low voice's own note: nothing here takes the bottom of a voice away.
        (48_000, 100.0),
        (44_100, 80.0),
    ] {
        track(&path, rate, 1, rate as usize * 3, note(rate, hertz, 0.5));
        let sound = heard(&path, &Clock::default()).unwrap();
        let (played, added) = (level(steady(&sound)), level(steady(&beside(&sound, hertz))));
        assert!(
            (20.0 * (played / (0.5 / 2f32.sqrt())).log10()).abs() < 1.0,
            "{rate} Hz: {hertz} Hz heard at {played}"
        );
        assert!(
            20.0 * (added / played).log10() < -60.0,
            "{rate} Hz: {} dB beside {hertz} Hz",
            20.0 * (added / played).log10()
        );
    }
    // And what the engine's rate cannot hold is removed, not folded back as a voice.
    for (rate, hertz) in [(48_000, 15_000.0), (96_000, 9_000.0), (44_100, 12_000.0)] {
        track(&path, rate, 1, rate as usize * 2, note(rate, hertz, 0.5));
        assert!(
            level(steady(&heard(&path, &Clock::default()).unwrap())) < 0.001,
            "{rate} Hz: {hertz} Hz was heard"
        );
    }
}

#[test]
fn a_source_that_rests_away_from_zero_is_heard_where_it_rests_and_the_track_is_not_touched() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote.wav");
    // Silent at its rest, the track is that rest from its first sample to its last: it does
    // not open or close on a step from zero that the source never made.
    for (rate, rest) in [(48_000u32, 0.3f32), (44_100, -0.7), (8_000, 0.01)] {
        track(&path, rate, 2, rate as usize * 2, |_, _| rest);
        let before = fs::read(&path).unwrap();
        let sound = heard(&path, &Clock::default()).unwrap();
        let apart = sound.iter().map(|sample| (sample - rest).abs()).fold(0.0, f32::max);
        assert!(apart < 0.0005, "{rate} Hz: a rest of {rest} is heard {apart} away from it");
        // And reading a track leaves every byte of it as it was.
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    // A voice around a rest is heard at its own level, around that rest: nothing is centred.
    track(&path, 48_000, 1, 96_000, |frame, _| 0.05 + note(48_000, 440.0, 0.2)(frame, 0));
    let voice = heard(&path, &Clock::default()).unwrap();
    let rest = voice.iter().sum::<f32>() / voice.len() as f32;
    assert!((rest - 0.05).abs() < 0.001, "a rest of 0.05 is heard at {rest}");
    let around: Vec<f32> = voice.iter().map(|sample| sample - rest).collect();
    assert!((level(steady(&around)) - 0.2 / 2f32.sqrt()).abs() < 0.003);
    // A track that ends away from where it opened closes where it ended.
    track(&path, 48_000, 1, 96_000, |frame, _| if frame < 48_000 { 0.0 } else { 0.4 });
    let stepped = heard(&path, &Clock::default()).unwrap();
    let (opens, closes) = (stepped[0], stepped[stepped.len() - 1]);
    assert!(
        opens.abs() < 0.0005 && (closes - 0.4).abs() < 0.0005,
        "opens on {opens}, closes on {closes}"
    );
}

#[test]
fn a_track_that_opens_and_ends_on_a_voice_is_that_voice_from_its_first_sample_to_its_last() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    // A low note on its crest at the first frame and at the last, around a rest.
    let voice = |at: f64| 0.1 + 0.3 * (std::f64::consts::TAU * 300.0 * at).cos() as f32;
    for rate in [48_000u32, 44_100, 16_000] {
        track(&path, rate, 1, rate as usize, |frame, _| voice(frame as f64 / f64::from(rate)));
        let sound = heard(&path, &Clock::default()).unwrap();
        let apart = |at: usize| (sound[at] - voice(at as f64 / RATE as f64)).abs();
        let worst = (0..320).chain(sound.len() - 320..sound.len()).map(apart).fold(0.0, f32::max);
        assert!(worst < 0.002, "{rate} Hz: {worst} from the voice at an edge");
    }
}

/// Where a burst starts in a sound, in seconds.
fn starts(sound: &[f32]) -> f64 {
    sound.iter().position(|sample| sample.abs() > 0.1).unwrap() as f64 / RATE as f64
}

fn at(points: &[(u64, f64)]) -> Clock {
    Clock {
        points: points.to_vec(),
        silences: Vec::new(),
    }
}

#[test]
fn a_track_is_placed_on_the_sessions_clock_by_its_first_point() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote.wav");
    track(&path, 48_000, 2, 48_000, note(48_000, 1_000.0, 0.5));

    // Its first frame heard a second and a half after the session started.
    let late = heard(&path, &at(&[(0, 1.5)])).unwrap();
    assert!((starts(&late) - 1.5).abs() < 0.002 && late.len() == RATE * 5 / 2);
    // A point further in says the same of frame 0: frame 24,000 at 1.5 s puts frame 0 at 1 s.
    assert!((starts(&heard(&path, &at(&[(24_000, 1.5)])).unwrap()) - 1.0).abs() < 0.002);
    // Started a quarter of a second before the session: that much is left out.
    let early = heard(&path, &at(&[(0, -0.25)])).unwrap();
    assert!((early.len() as i64 - RATE as i64 * 3 / 4).abs() <= 1 && starts(&early) < 0.002);
    // A point for a frame the track does not hold places nothing.
    assert_eq!(heard(&path, &at(&[(1_000_000, 500.0)])).unwrap().len(), RATE);
    // No point at all: it starts with the session.
    assert!(starts(&heard(&path, &Clock::default()).unwrap()) < 0.002);
}

#[test]
fn the_sound_after_a_source_stopped_is_where_the_clock_says_it_was_heard() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("remote.wav");
    // A burst from 1 s to 2 s of the track, another from 4 s to 5 s.
    let burst = |frame: usize, _| match frame {
        48_000..96_000 | 192_000..240_000 => note(48_000, 1_000.0, 0.5)(frame, 0),
        _ => 0.0,
    };
    track(&path, 48_000, 1, 288_000, burst);
    let second = |sound: &[f32]| 3.0 + starts(&sound[3 * RATE..]);

    // The source stopped a second, between the bursts, too short for zeros: a point says where it came back.
    let stopped = heard(&path, &at(&[(0, 0.0), (144_000, 4.0), (240_000, 6.0)])).unwrap();
    assert!((starts(&stopped) - 1.0).abs() < 0.002 && (second(&stopped) - 5.0).abs() < 0.002);
    assert!(stopped[3 * RATE..4 * RATE].iter().all(|sample| sample.abs() < 0.0005));
    assert_eq!(stopped.len(), 7 * RATE);
    // A clock's points are taken in the order of their frames, whatever order they are given in.
    assert_eq!(stopped, heard(&path, &at(&[(240_000, 6.0), (144_000, 4.0), (0, 0.0)])).unwrap());
    // Zeros of the capture's, then a point half a second after them, as the capture writes one.
    let behind = Clock {
        points: vec![(0, 0.0), (144_000, 3.5)],
        silences: vec![(120_000, 144_000)],
    };
    assert!((second(&heard(&path, &behind).unwrap()) - 4.5).abs() < 0.002);
    // Points that agree with the ones before — a ten-thousandth apart — move nothing.
    let agreeing = heard(&path, &at(&[(0, 0.0), (96_000, 2.0001), (240_000, 4.9999)])).unwrap();
    assert_eq!(agreeing, heard(&path, &at(&[(0, 0.0)])).unwrap());
    // Five thousandths of a second is where a source is said to have stopped: four move nothing, six do.
    let near = |off: f64| heard(&path, &at(&[(0, 0.0), (144_000, 3.0 + off)])).unwrap().len();
    assert_eq!((near(0.004), near(0.006)), (6 * RATE, 6 * RATE + RATE * 6 / 1000));
    // A point that puts sound before what is already placed takes its place from there.
    let back = heard(&path, &at(&[(0, 0.0), (144_000, 2.5)])).unwrap();
    assert!((3.0 + starts(&back[3 * RATE..]) - 3.5).abs() < 0.002 && back.len() == 11 * RATE / 2);
}

#[test]
fn where_a_source_delivered_nothing_no_step_is_heard_that_it_never_made() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let rests = |sound: &[f32], rest: f32| sound.iter().all(|sample| (sample - rest).abs() < 0.0005);
    for rate in [48_000u32, 44_100, 8_000] {
        let second = u64::from(rate);
        // A source resting at 0.3: zeros of the capture's as it opens, in its middle and as
        // it ends; heard half a second into the session, and stopped a second at 4 s.
        let zeros = [(0, second / 2), (2 * second, 3 * second), (5 * second, 6 * second)];
        let sound = |frame: usize, _| {
            if zeros.iter().any(|(from, to)| (*from..*to).contains(&(frame as u64))) {
                0.0
            } else {
                0.3
            }
        };
        track(&path, rate, 2, 6 * rate as usize, sound);
        let clock = Clock {
            points: vec![(0, 0.5), (4 * second, 5.5)],
            silences: zeros.to_vec(),
        };
        let all = heard(&path, &clock).unwrap();
        assert_eq!(all.len(), RATE * 15 / 2, "{rate} Hz");
        assert!(
            rests(&all, 0.3),
            "{rate} Hz: a step was heard, {:?}",
            all.iter().copied().reduce(f32::min)
        );
        // Zeros the clock does not name are the room's, and are heard.
        let unnamed = Clock {
            silences: zeros[..1].to_vec(),
            ..at(&clock.points)
        };
        let unnamed = heard(&path, &unnamed).unwrap();
        assert!(unnamed[3 * RATE].abs() < 0.01 && !rests(&unnamed, 0.3), "{rate} Hz");
    }
    // A sound in frames the clock names is a sound: only zeros are the capture's.
    track(&path, 48_000, 1, 96_000, |frame, _| 0.3 + note(48_000, 440.0, 0.2)(frame, 0));
    let named = Clock {
        points: Vec::new(),
        silences: vec![(0, 96_000)],
    };
    assert_eq!(heard(&path, &named).unwrap(), heard(&path, &Clock::default()).unwrap());
    // Zeros on one channel only are not the capture's: the other channel's sound is heard.
    track(&path, 48_000, 2, 96_000, |_, channel| [0.0, 0.4][channel]);
    assert!(rests(&heard(&path, &named).unwrap(), 0.2));
    // A source that left at 0.2 and came back a second later at -0.1, with no zeros in its
    // track: the same line, and neither side heard in the other.
    for rate in [48_000u32, 8_000] {
        track(&path, rate, 1, 2 * rate as usize, |frame, _| [0.2, -0.1][frame / rate as usize]);
        let stop = heard(&path, &at(&[(0, 0.0), (u64::from(rate), 2.0)])).unwrap();
        assert!(
            rests(&stop[..RATE], 0.2) && rests(&stop[2 * RATE..], -0.1) && stop.len() == 3 * RATE,
            "{rate} Hz"
        );
        assert!((stop[3 * RATE / 2] - 0.05).abs() < 0.0005 && stop.windows(2).all(|pair| pair[1] <= pair[0] + 0.00001));
    }
    // The same with the capture's zeros between the two, the clock placing what follows from
    // the middle of them, its silences given out of order and a point given twice: one line.
    track(&path, 48_000, 1, 144_000, |frame, _| [0.2, 0.0, -0.1][frame / 48_000]);
    let within = Clock {
        points: vec![(0, 0.0), (72_000, 2.5), (0, 9.0)],
        silences: vec![(72_000, 96_000), (48_000, 72_000)],
    };
    let within = heard(&path, &within).unwrap();
    assert!(rests(&within[..RATE], 0.2) && rests(&within[7 * RATE / 2..], -0.1) && within.len() == 4 * RATE);
    assert!(within.windows(2).all(|pair| pair[1] <= pair[0] + 0.00001) && (within[2 * RATE] - 0.05).abs() < 0.03);
    // A first run of one frame, heard late: the track opens where its source rests, not on zeros.
    track(&path, 48_000, 1, 48_000, |_, _| 0.3);
    assert!(rests(&heard(&path, &at(&[(0, 0.4), (1, 1.0)])).unwrap(), 0.3));
    // A zero the source delivered right after the capture's is the room's: it is heard.
    track(
        &path,
        16_000,
        1,
        16_000,
        |frame, _| if (4_000..=8_000).contains(&frame) { 0.0 } else { 0.3 },
    );
    let after = Clock {
        points: Vec::new(),
        silences: vec![(4_000, 8_000)],
    };
    assert!(heard(&path, &after).unwrap()[8_000] < 0.1);
    track(&path, 48_000, 1, 144_000, |frame, _| [0.2, 0.0, -0.1][frame / 48_000]);
    let line = heard(
        &path,
        &Clock {
            points: Vec::new(),
            silences: vec![(48_000, 96_000)],
        },
    )
    .unwrap();
    assert!(rests(&line[..RATE], 0.2) && rests(&line[2 * RATE..], -0.1) && (line[3 * RATE / 2] - 0.05).abs() < 0.0005);
    assert!(
        line.windows(2).all(|pair| pair[1] <= pair[0] + 0.00001),
        "the line does not only go down"
    );
}

#[test]
fn the_clocks_lines_are_a_tracks_own_in_the_order_of_its_frames_and_only_instants_of_a_session() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(
        folder.path().join("clock.jsonl"),
        "\u{feff}{\"track\":\"local\",\"sample\":0,\"at\":0.02}\n{\"track\":\"remote\",\"silence\":[960,1440]}\n\
         {\"track\":\"remote\",\"sample\":240480,\"at\":5.07}\n{\"track\":\"remote\",\"sample\":480,\"at\":0.07}\n\
         not json\n{\"track\":\"remote\",\"sample\":9,\"at\":1e9}\n{\"track\":\"remote\",\"sample\":8,\"at\":-1e9}\n{\"track\":\"remote\",\"sample\":-4,\"at\":1}\n\
         {\"track\":\"remote\",\"sample\":10,\"at\":null}\n{\"track\":\"remote\",\"sample\":480,\"at\":9.0}\n\
         {\"track\":\"remote\",\"silence\":[0,480]}\n{\"track\":\"remote\",\"silence\":[7,7]}\n\
         {\"track\":\"remote\",\"silence\":[9]}\n{\"track\":\"local\",\"silence\":[3,5]}\n{\"track\":\"rem",
    )
    .unwrap();
    let remote = clock(folder.path(), "remote");
    assert_eq!(remote.points, [(480, 0.07), (240_480, 5.07)]);
    assert_eq!(remote.silences, [(0, 480), (960, 1440)]);
    assert_eq!(
        clock(folder.path(), "local"),
        Clock {
            points: vec![(0, 0.02)],
            silences: vec![(3, 5)]
        }
    );
    assert_eq!(clock(folder.path(), "other"), Clock::default());
    assert_eq!(clock(Path::new("/nowhere"), "local"), Clock::default());
}

#[test]
fn a_track_still_written_or_cut_by_a_crash_is_heard_to_its_last_whole_frame() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    track(&path, 48_000, 2, 48_000, note(48_000, 440.0, 0.2));
    let whole = heard(&path, &Clock::default()).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes.truncate(bytes.len() - 4); // the last frame lost two of its three bytes on one channel
    fs::write(&path, bytes).unwrap();
    let cut = heard(&path, &Clock::default()).unwrap();
    assert!(whole.len() - cut.len() <= 1);
    let apart = cut[..cut.len() - 64]
        .iter()
        .zip(&whole)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(apart < 1e-4, "a frame lost at the end moved the sound before it by {apart}");
}

#[test]
fn a_file_that_is_not_a_track_is_said_with_its_name_and_not_converted() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("local.wav");
    let says = |expected: &str| {
        let said = heard(&path, &Clock::default()).unwrap_err().to_string();
        assert!(said.contains(expected), "{said}");
    };
    says("local.wav cannot be read");
    fs::create_dir(&path).unwrap();
    says("local.wav is not a file");
    fs::remove_dir(&path).unwrap();
    fs::write(&path, b"not sound").unwrap();
    says("local.wav is not a track of a session");
    // What the first version held: one channel, 16 bits.
    fs::write(
        &path,
        b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff\0\0",
    )
    .unwrap();
    says("PCM in 24 bits: it has 1 channel(s), 16000 Hz, 16 bits");
    // 24 bits that are not PCM: said as that.
    fs::write(
        &path,
        b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x03\0\x01\0\x80\xbb\0\0\x80\x32\x02\0\x03\0\x18\0data\xff\xff\xff\xff",
    )
    .unwrap();
    says("24 bits, and is not PCM");
    // A rate no source of a meeting delivers is not given the memory it would take.
    for rate in [1u32, 999, 1_000_001, u32::MAX] {
        let mut bytes = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(b"\0\0\0\0\x03\0\x18\0data\xff\xff\xff\xff\0\0\0");
        fs::write(&path, bytes).unwrap();
        says(&format!("it has 1 channel(s), {rate} Hz, 24 bits"));
    }
    // The slowest and the fastest a header may say are read.
    for rate in [1_000u32, 1_000_000] {
        track(&path, rate, 1, rate as usize / 10, |_, _| 0.1);
        assert_eq!(heard(&path, &Clock::default()).unwrap().len(), 1_600, "{rate} Hz");
    }
    // A long chunk before the audio is passed over: the track is one.
    track(&path, 48_000, 1, 4_800, note(48_000, 440.0, 0.2));
    let mut bytes = fs::read(&path).unwrap();
    let list: Vec<u8> = b"LIST"
        .iter()
        .copied()
        .chain(100_001u32.to_le_bytes())
        .chain(vec![7u8; 100_002])
        .collect();
    bytes.splice(12..12, list);
    fs::write(&path, bytes).unwrap();
    assert_eq!(heard(&path, &Clock::default()).unwrap().len(), 1_600);
}
