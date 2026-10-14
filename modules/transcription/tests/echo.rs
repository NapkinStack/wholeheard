//! The echo taken out where it is measured. Sound made here: voiced sound that comes and goes
//! as a voice does, no voice of anyone.

mod sound;

use sound::{energy, room, speaking};
use summit_transcription::{echo::cancelled, step::RATE};

/// What a sound holds from `from` on, where `when` says, of the session's seconds.
fn where_(sound: &[f32], from: usize, when: impl Fn(f64) -> bool) -> f64 {
    let kept: Vec<f32> = (from..sound.len())
        .filter(|n| when(*n as f64 / RATE as f64))
        .map(|n| sound[n])
        .collect();
    energy(&kept)
}

#[test]
fn an_echo_of_loudspeakers_is_measured_and_taken_out_and_the_local_voice_is_kept() {
    let remote = speaking(40.0, 0.0, 0.3, 1);
    let voice = speaking(40.0, 2.0, 0.2, 2);
    let local: Vec<f32> = voice.iter().zip(room(&remote, 0.5)).map(|(mine, echo)| mine + echo).collect();
    let (clean, found) = cancelled(&local, &remote);
    assert!(found.is_measured(), "{found:?}");
    assert_eq!(clean.len(), local.len());
    // After the canceller has learnt the room — its first ten seconds — where only the remote
    // speaks, at most a quarter of the echo's energy is left; where only the local speaks, its
    // voice is kept within a fifth.
    let learnt = 10 * RATE;
    let remote_only = |at: f64| at % 4.0 < 1.5;
    let local_only = |at: f64| (2.0..3.5).contains(&(at % 4.0));
    let left = where_(&clean, learnt, remote_only) / where_(&local, learnt, remote_only);
    assert!(left < 0.25, "left {:.1} % of the echo", 100.0 * left);
    let kept = where_(&clean, learnt, local_only) / where_(&voice, learnt, local_only);
    assert!((0.8..1.2).contains(&kept), "the local voice kept at {:.1} %", 100.0 * kept);
}

#[test]
fn a_microphone_that_does_not_hear_the_loudspeakers_holds_no_echo() {
    let remote = speaking(40.0, 0.0, 0.3, 1);
    let local = speaking(40.0, 2.0, 0.2, 2);
    let (_, found) = cancelled(&local, &remote);
    assert!(!found.is_measured(), "{found:?}");
    // And a remote track that never sounds gives the canceller's floor, no echo.
    let (_, found) = cancelled(&local, &vec![0.0; local.len()]);
    assert!(!found.is_measured(), "{found:?}");
}

#[test]
fn the_beginning_of_a_track_cut_anywhere_gives_the_beginning_of_what_the_whole_gives() {
    let remote = speaking(20.0, 0.0, 0.3, 3);
    let local: Vec<f32> = speaking(20.0, 2.0, 0.2, 4)
        .iter()
        .zip(room(&remote, 0.5))
        .map(|(a, b)| a + b)
        .collect();
    let (whole, _) = cancelled(&local, &remote);
    let (again, _) = cancelled(&local, &remote);
    assert_eq!(whole, again);
    // Anywhere: on a frame's edge, or within one — the bench's tracks end 44 samples into a frame.
    for cut in [160, 777 * 160, 1234 * 160, 1, 12_345, 500 * 160 + 44] {
        let (part, _) = cancelled(&local[..cut], &remote);
        assert_eq!(part, whole[..cut], "cut at sample {cut}");
    }
}
