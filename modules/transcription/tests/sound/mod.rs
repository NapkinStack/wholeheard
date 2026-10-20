//! Sound made for the tests of the echo: voiced turns, and a room that returns them. No
//! voice of anyone.

use wholeheard_transcription::step::RATE;

/// A voice in turns of 1.5 s every 4 s from `offset`, at `level`: a side that speaks, then
/// listens. Voiced as speech is — a pitch that moves, its harmonics up to 7 kHz, four
/// syllables a second; each `seed` its own pitch. White noise is no voice: the canceller keeps
/// a fifth of its energy, a loss no speech on the bench showed.
pub fn speaking(seconds: f64, offset: f64, level: f32, seed: u64) -> Vec<f32> {
    let tau = std::f32::consts::TAU;
    let pitch = 100.0 + 37.0 * seed as f32;
    (0..(seconds * RATE as f64) as usize)
        .map(|n| {
            let t = n as f32 / RATE as f32;
            let at = t - offset as f32;
            if at < 0.0 || at % 4.0 >= 1.5 {
                return 0.0;
            }
            let syllable = 0.5 + 0.5 * (tau * 4.0 * at).sin();
            let moving = pitch * (1.0 + 0.05 * (tau * 0.7 * at).sin());
            let voiced: f32 = (1..=20)
                .map(|k| (k as f32, moving * k as f32))
                .take_while(|(_, hz)| *hz < 7000.0)
                .map(|(k, hz)| (tau * hz * t).sin() / k)
                .sum();
            level * syllable * voiced / 2.0
        })
        .collect()
}

/// What a microphone hears of loudspeakers: 46 ms later, weaker, ringing a little.
pub fn room(played: &[f32], gain: f32) -> Vec<f32> {
    let late = RATE * 46 / 1000;
    (0..played.len())
        .map(|n| {
            (0..4)
                .filter_map(|ring| {
                    n.checked_sub(late + ring * 40)
                        .map(|at| played[at] * gain * 0.5f32.powi(ring as i32))
                })
                .sum()
        })
        .collect()
}

pub fn energy(sound: &[f32]) -> f64 {
    sound.iter().map(|sample| f64::from(*sample).powi(2)).sum()
}
