//! D1, first half — a throwaway probe, not product code.
//!
//! The question: can one program write the microphone and the system's sound as two files,
//! on Linux, Windows and macOS, with the same code? It opens the default input for "me" and
//! the default **output** as an input for "others": the audio library turns that into the
//! system's own way of capturing what plays — the sink's monitor under PipeWire, WASAPI
//! loopback on Windows, a process tap on macOS 14.6 and later.
//!
//! It records for a few seconds, then prints what D1 asks to be recorded: per source, whether
//! a file was written, how long it is, and whether it holds sound or silence.
//!
//!     d1-capture [seconds] [folder] [--tone]     defaults: 10 seconds, the current folder
//!
//! `--tone` plays a quiet note on the default output during the run: with it, silence in
//! "others" cannot mean that nothing was playing.
//!
//! Run it during a real call, with people who know they are recorded, and delete the files
//! afterwards: no recording of a real meeting enters the repository (rules/security.md, 2).

use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, FromSample, Sample, SampleFormat, Stream, SupportedStreamConfig,
};

type Writer = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

/// What one source recorded, filled in from the audio thread.
struct Take {
    name: &'static str,
    path: PathBuf,
    device: String,
    config: String,
    sample_rate: u32,
    channels: u16,
    samples: Arc<AtomicU64>,
    peak: Arc<AtomicU32>, // the loudest sample seen, as the bits of an f32 in 0..=1
    writer: Writer,
    _stream: Stream,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tone = args.iter().any(|a| a == "--tone");
    let mut rest = args.iter().filter(|a| !a.starts_with("--"));
    let seconds: u64 = rest.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let folder = PathBuf::from(rest.next().cloned().unwrap_or_else(|| ".".into()));

    let host = pick_host();
    println!("System      : {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("Audio host  : {}", host.id().name());
    println!("Devices     :");
    if let Ok(devices) = host.devices() {
        for device in devices {
            let id = device.id().map(|id| id.to_string()).unwrap_or_else(|_| "unnamed".into());
            let way = match (device.supports_input(), device.supports_output()) {
                (true, true) => "in+out",
                (true, false) => "in",
                (false, true) => "out",
                _ => "?",
            };
            println!("  {way:<6} {id}");
        }
    }

    // "others" is tried on every way this host offers to hear what plays: the first run on
    // Fedora opened the default output and got twenty seconds of zeros, with no error.
    let mut takes: Vec<(&'static str, Result<Take, String>)> = vec![(
        "me",
        host.default_input_device()
            .ok_or_else(|| "no default input device".to_string())
            .and_then(|device| open("me", &device, false, &folder)),
    )];
    for (name, id) in [("others-sink", "pipewire:sink_default"), ("others-out", "pipewire:output_default")] {
        if let Some(device) = id.parse().ok().and_then(|id| host.device_by_id(&id)) {
            takes.push((name, open(name, &device, true, &folder)));
        }
    }
    if takes.len() == 1 {
        takes.push((
            "others",
            host.default_output_device()
                .ok_or_else(|| "no default output device".to_string())
                .and_then(|device| open("others", &device, true, &folder)),
        ));
    }

    // A known sound, so that silence in "others" cannot mean "nothing was playing".
    let _tone = if tone {
        match host.default_output_device().ok_or_else(|| "no default output".to_string()).and_then(|d| play_tone(&d)) {
            Ok(stream) => {
                println!("Tone        : a quiet 440 Hz note plays on the default output for the whole run");
                Some(stream)
            }
            Err(reason) => {
                println!("Tone        : NOT PLAYED — {reason}");
                None
            }
        }
    } else {
        None
    };

    println!("Recording   : {seconds} s");
    std::thread::sleep(Duration::from_secs(seconds));

    let mut me_heard = false;
    let mut others_heard = false;
    for (name, take) in takes {
        let outcome = take.map(finish);
        if let Ok(outcome) = &outcome {
            if name == "me" {
                me_heard = outcome.heard;
            } else {
                others_heard |= outcome.heard;
            }
        }
        report(name, &outcome);
    }
    let both = me_heard && others_heard;
    println!(
        "\nD1, two files of a real call: {}",
        if both { "MET on this system" } else { "NOT MET on this system — see above" }
    );
    std::process::exit(if both { 0 } else { 2 });
}

/// A sine at a tenth of full scale on the default output, until the stream is dropped.
fn play_tone(device: &Device) -> Result<Stream, String> {
    let config = device.default_output_config().map_err(|e| format!("no output configuration ({e})"))?;
    let (rate, channels) = (config.sample_rate() as f32, config.channels() as usize);
    let mut phase = 0f32;
    let errors = |error: cpal::Error| eprintln!("  [tone] stream: {error}");
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            config.into(),
            move |data: &mut [f32], _: &_| {
                for frame in data.chunks_mut(channels.max(1)) {
                    let value = (phase * std::f32::consts::TAU).sin() * 0.1;
                    phase = (phase + 440.0 / rate) % 1.0;
                    frame.fill(value);
                }
            },
            errors,
            None,
        ),
        other => return Err(format!("output format {other} not handled by this probe")),
    }
    .map_err(|e| format!("the tone could not be opened ({e})"))?;
    stream.play().map_err(|e| format!("the tone could not start ({e})"))?;
    Ok(stream)
}

/// PipeWire when the build has it and the session runs it; the system's default otherwise.
fn pick_host() -> cpal::Host {
    #[cfg(all(target_os = "linux", feature = "pipewire"))]
    if let Ok(host) = cpal::host_from_id(cpal::HostId::PipeWire) {
        return host;
    }
    cpal::default_host()
}

fn open(name: &'static str, device: &Device, playback: bool, folder: &Path) -> Result<Take, String> {
    let label = device.id().map(|id| id.to_string()).unwrap_or_else(|_| "unnamed".into());
    // An output device has no input configuration of its own: its output format is the
    // format of what plays through it, which is what the capture delivers.
    let config: SupportedStreamConfig = if playback {
        device.default_output_config()
    } else {
        device.default_input_config()
    }
    .map_err(|error| format!("{label}: no default configuration ({error})"))?;

    let format = config.sample_format();
    let spec = hound::WavSpec {
        channels: config.channels() as u16,
        sample_rate: config.sample_rate() as u32,
        bits_per_sample: (format.sample_size() * 8) as u16,
        sample_format: if format.is_float() { hound::SampleFormat::Float } else { hound::SampleFormat::Int },
    };
    let path = folder.join(format!("{name}.wav"));
    let writer: Writer = Arc::new(Mutex::new(Some(
        hound::WavWriter::create(&path, spec).map_err(|error| format!("{}: {error}", path.display()))?,
    )));
    let samples = Arc::new(AtomicU64::new(0));
    let peak = Arc::new(AtomicU32::new(0f32.to_bits()));
    let errors = move |error: cpal::Error| eprintln!("  [{name}] stream: {error}");

    macro_rules! stream {
        ($t:ty) => {{
            let (writer, samples, peak) = (writer.clone(), samples.clone(), peak.clone());
            device.build_input_stream(
                config.clone().into(),
                move |data: &[$t], _: &_| write::<$t>(data, &writer, &samples, &peak),
                errors,
                None,
            )
        }};
    }
    let stream = match format {
        SampleFormat::I8 => stream!(i8),
        SampleFormat::I16 => stream!(i16),
        SampleFormat::I32 => stream!(i32),
        SampleFormat::F32 => stream!(f32),
        other => return Err(format!("{label}: sample format {other} not handled by this probe")),
    }
    .map_err(|error| format!("{label}: the stream could not be opened ({error})"))?;
    stream.play().map_err(|error| format!("{label}: the stream could not start ({error})"))?;

    Ok(Take {
        name,
        path,
        device: label,
        config: format!("{} Hz, {} channel(s), {}", spec.sample_rate, spec.channels, format),
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples,
        peak,
        writer,
        _stream: stream,
    })
}

fn write<T>(data: &[T], writer: &Writer, samples: &AtomicU64, peak: &AtomicU32)
where
    T: Sample + hound::Sample,
    f32: FromSample<T>,
{
    let mut loudest = f32::from_bits(peak.load(Ordering::Relaxed));
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in data {
                loudest = loudest.max(f32::from_sample(sample).abs());
                let _ = writer.write_sample(sample);
            }
            samples.fetch_add(data.len() as u64, Ordering::Relaxed);
        }
    }
    peak.store(loudest.to_bits(), Ordering::Relaxed);
}

/// One line of D1's record for a source.
struct Outcome {
    line: String,
    heard: bool,
}

fn finish(take: Take) -> Outcome {
    let Take { name, path, device, config, sample_rate, channels, samples, peak, writer, _stream } = take;
    drop(_stream);
    let written = writer.lock().ok().and_then(|mut guard| guard.take()).map(|w| w.finalize().is_ok());
    let frames = samples.load(Ordering::Relaxed) / u64::from(channels.max(1));
    let seconds = frames as f64 / f64::from(sample_rate.max(1));
    let peak = f32::from_bits(peak.load(Ordering::Relaxed));
    // Below one thousandth of full scale is a file of silence: the permission was refused,
    // nothing was playing, or the capture reached the wrong device.
    let heard = written == Some(true) && frames > 0 && peak > 0.001;
    let verdict = match (written, frames, heard) {
        (Some(true), 0, _) => "a file with no sound in it: the stream delivered nothing",
        (Some(true), _, false) => "a file of SILENCE: permission refused, nothing playing, or the wrong device",
        (Some(true), _, true) => "a file with sound",
        _ => "the file could not be written",
    };
    Outcome {
        line: format!(
            "{name:<7}: {verdict}\n          file   {}\n          device {device}\n          format {config}\n          length {seconds:.1} s, loudest sample {:.1} % of full scale",
            path.display(),
            peak * 100.0
        ),
        heard,
    }
}

fn report<T>(name: &str, result: &Result<T, String>)
where
    T: Line,
{
    match result {
        Ok(value) => println!("{}", value.line()),
        Err(reason) => println!("{name:<7}: NOT RECORDED — {reason}"),
    }
}

trait Line {
    fn line(&self) -> String;
}
impl Line for Outcome {
    fn line(&self) -> String {
        self.line.clone()
    }
}
impl Line for Take {
    fn line(&self) -> String {
        format!("{:<7}: opened on {} ({})", self.name, self.device, self.config)
    }
}
