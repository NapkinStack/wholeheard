//! The echo of loudspeakers, taken out of what an engine hears of the microphone — only where
//! it is measured (ADR-0004, rule 5). The canceller is WebRTC's, the one browsers and PipeWire
//! use, in its Rust port: fed the remote track as what the loudspeakers played, it learns how
//! the room returns it, and takes that out of the local track. Nothing is written here: what
//! it made is the step's to keep.

use sonora::{AudioProcessing, Config, StreamConfig, config::EchoCanceller};

use crate::step::RATE;

/// How much the canceller must take out, in decibels, for an echo to be measured: its own
/// estimate, the median of one a second. On the bench a headset gives 0.2 dB, loudspeakers
/// 7 to 8.5 dB (cycle 02, D10). Under it, the microphone goes to the engine as recorded: a
/// canceller run on audio that holds no echo can only damage it.
pub const MEASURED: f64 = 3.0;
/// The canceller hears ten milliseconds at a time.
const FRAME: usize = RATE / 100;

/// What the canceller found of a session's echo.
#[derive(Debug, Clone, PartialEq)]
pub struct Echo {
    /// What it took out, in decibels: the median of its estimate, one a second; `None` when
    /// the remote track never sounded long enough for one.
    pub taken_out: Option<f64>,
    /// How late the microphone heard the loudspeakers, in milliseconds, as it last estimated.
    pub delay: Option<i32>,
}

impl Echo {
    /// Whether there is an echo to take out.
    pub fn is_measured(&self) -> bool {
        self.taken_out.is_some_and(|decibels| decibels >= MEASURED)
    }
}

/// The local track with what the remote track's echo in it taken out, and what was found.
/// Both are one channel at [`RATE`], on the session's clock; the result is as long as the
/// local track. The same tracks give the same samples, and a track's beginning, cut at a
/// hundredth of a second, gives the beginning of what the whole gives: the canceller hears
/// forward only, ten milliseconds at a time.
pub fn cancelled(local: &[f32], remote: &[f32]) -> (Vec<f32>, Echo) {
    let config = Config {
        echo_canceller: Some(EchoCanceller::default()),
        ..Default::default()
    };
    let stream = StreamConfig::new(RATE as u32, 1);
    let mut canceller = AudioProcessing::builder()
        .config(config)
        .capture_config(stream)
        .render_config(stream)
        .build();
    let (mut played, mut heard) = ([0.0f32; FRAME], [0.0f32; FRAME]);
    let mut clean = Vec::with_capacity(local.len() + FRAME);
    let mut estimates = Vec::new();
    let mut delay = None;
    // The last frame is completed with silence; what follows the local track is cut off.
    let frames = local.len().div_ceil(FRAME);
    for frame in 0..frames {
        let piece = |track: &[f32]| {
            let mut samples = [0.0f32; FRAME];
            let from = (frame * FRAME).min(track.len());
            let until = (from + FRAME).min(track.len());
            samples[..until - from].copy_from_slice(&track[from..until]);
            samples
        };
        let (theirs, mine) = (piece(remote), piece(local));
        // A frame the canceller refuses is impossible at this rate and length: it is passed on as it was.
        if canceller.process_render_f32(&[&theirs], &mut [&mut played]).is_err()
            || canceller.process_capture_f32(&[&mine], &mut [&mut heard]).is_err()
        {
            heard = mine;
        }
        clean.extend_from_slice(&heard);
        if (frame + 1) % 100 == 0 {
            let said = canceller.statistics();
            estimates.extend(said.echo_return_loss_enhancement.filter(|decibels| decibels.is_finite()));
            delay = said.delay_ms.or(delay);
        }
    }
    clean.truncate(local.len());
    estimates.sort_by(f64::total_cmp);
    let taken_out = estimates.get(estimates.len() / 2).copied();
    (clean, Echo { taken_out, delay })
}
