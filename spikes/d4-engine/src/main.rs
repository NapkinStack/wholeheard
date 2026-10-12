//! D4, before its engine is chosen: does a native engine — whisper.cpp, linked into a Rust
//! program — transcribe a French meeting as well and as fast as the Python one D2 used?
//!
//!     d4-engine <model.bin> <audio.wav: 16 kHz, one channel, 16-bit> [context...]
//!
//! It prints the transcript as `[mm:ss] text` lines and, on the error stream, the two
//! figures the cycle records: the time to load the model and the time to transcribe.

use std::{fs, time::Instant};

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, audio, context @ ..] = &args[..] else {
        eprintln!("usage: d4-engine <model.bin> <audio.wav> [context...]");
        std::process::exit(2);
    };
    let bytes = fs::read(audio).expect("the audio file");
    let samples: Vec<f32> = bytes[44..].as_chunks::<2>().0.iter().map(|pair| f32::from(i16::from_le_bytes(*pair)) / 32768.0).collect();
    let seconds = samples.len() as f64 / 16_000.0;

    let loading = Instant::now();
    let engine = WhisperContext::new_with_params(model, WhisperContextParameters::default()).expect("the model");
    let mut state = engine.create_state().expect("a state");
    let loaded = loading.elapsed().as_secs_f64();

    let prompt = context.join(" ");
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("fr"));
    let threads = std::env::var("D4_THREADS").ok().and_then(|n| n.parse().ok());
    params.set_n_threads(threads.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4)));
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    if !prompt.is_empty() {
        params.set_initial_prompt(&prompt);
    }
    let transcribing = Instant::now();
    state.full(params, &samples).expect("the transcription");
    let took = transcribing.elapsed().as_secs_f64();

    for segment in state.as_iter() {
        let start = segment.start_timestamp() / 100;
        println!("[{:02}:{:02}] {}", start / 60, start % 60, segment.to_str_lossy().expect("text").trim());
    }
    eprintln!("audio {seconds:.0} s · model loaded in {loaded:.1} s · transcribed in {took:.1} s — {:.0} % of its length", 100.0 * took / seconds);
}
