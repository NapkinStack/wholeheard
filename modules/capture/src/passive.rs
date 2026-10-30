//! The local side's stream on a headset's virtual input (ADR-0015): passive (`call::passive_value`), so
//! it never makes the source run — it is carried by the call that does, and nothing it does asks for
//! the headset's profile. cpal cannot say that of a stream, nor leave a stream out of its
//! `node.group`, so this one source is opened with the library cpal itself is built on.
//!
//! It has a thread and a main loop of its own, and delivers what it hears as cpal's streams do. A
//! stream that fails only says so: the recording goes on, and the graph says what comes next.
//!
//! Opening it never waits. On a source that is not running, PipeWire links it and negotiates nothing
//! (measured on 1.6.10, a `Format` of none, the link `paused`): a form comes when the source runs, that
//! is, when a call does. Until then it is healthy, and has no form to say.

use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use pipewire as pw;
use pw::spa::{
    self,
    param::{
        audio::{AudioFormat, AudioInfoRaw},
        format::{MediaSubtype, MediaType},
        format_utils,
    },
    pod::Pod,
};
use tracing::warn;
use wholeheard_capture::call;

use crate::Sound;

/// Closing takes the stream's thread a moment; one that does not answer is left behind, for a
/// stop or the next attach never waits on it.
const CLOSED_WITHIN: Duration = Duration::from_millis(500);

/// A form: frames a second, and channels.
type Form = (u32, u16);

/// The stream. Dropped, it has left the graph.
pub struct Passive {
    stop: pw::channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
    alive: Arc<AtomicBool>,
    form: Arc<OnceLock<Form>>,
}

impl Passive {
    /// Listens to the source named `target`, saying `passive`, and hands what it delivers to `sound`
    /// as source `index`. `form`: the one a track already has, into which the system converts, as it
    /// does when a device changes; none, the one it negotiates, read from `form`.
    pub fn open(target: &str, passive: &'static str, form: Option<Form>, (index, zero): (usize, Instant), sound: Sender<Sound>) -> Passive {
        let (stop, stopped) = pw::channel::channel::<()>();
        let alive = Arc::new(AtomicBool::new(true));
        let formed = Arc::new(OnceLock::new());
        let thread = {
            let (target, alive, formed, run) = (target.to_string(), alive.clone(), formed.clone(), tracing::Span::current());
            std::thread::spawn(move || {
                let _run = run.enter();
                let listening = Listening {
                    asked: form,
                    form: formed,
                    alive: alive.clone(),
                    index,
                    zero,
                    sound,
                };
                if let Err(detail) = listen(&target, passive, form, stopped, listening) {
                    alive.store(false, Ordering::SeqCst);
                    warn!(detail = %call::without_address(&detail), "capture.me_lost");
                }
            })
        };
        Passive {
            stop,
            thread: Some(thread),
            alive,
            form: formed,
        }
    }

    /// The form it received, once the source ran; none once it failed.
    pub fn form(&self) -> Option<Form> {
        self.form.get().copied().filter(|_| self.alive())
    }

    /// False once it failed: its source gone, or its format changed.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

impl Drop for Passive {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        let Some(thread) = self.thread.take() else { return };
        let asked = Instant::now();
        while !thread.is_finished() && asked.elapsed() < CLOSED_WITHIN {
            std::thread::sleep(Duration::from_millis(5));
        }
        if thread.is_finished() {
            let _ = thread.join();
        } else {
            warn!("capture.me_not_closed");
        }
    }
}

struct Listening {
    asked: Option<Form>,
    form: Arc<OnceLock<Form>>,
    alive: Arc<AtomicBool>,
    index: usize,
    zero: Instant,
    sound: Sender<Sound>,
}

fn listen(
    target: &str,
    passive: &'static str,
    form: Option<Form>,
    stopped: pw::channel::Receiver<()>,
    listening: Listening,
) -> Result<(), String> {
    let failed = |what: &str, error: pw::Error| format!("{what} ({error})");
    // cpal's host, alive for the whole recording, keeps the library initialised: this only asks.
    pw::init();
    let main_loop = pw::main_loop::MainLoopRc::new(None).map_err(|e| failed("no main loop", e))?;
    let context = pw::context::ContextRc::new(&main_loop, None).map_err(|e| failed("no context", e))?;
    let core = context.connect_rc(None).map_err(|e| failed("no connection", e))?;
    let _stopped = stopped.attach(main_loop.loop_(), {
        let main_loop = main_loop.clone();
        move |()| main_loop.quit()
    });

    let mut props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        // Its own name; the shell knows capture's streams by their program, not by this.
        *pw::keys::NODE_NAME => "wholeheard-capture-me",
        *pw::keys::TARGET_OBJECT => target,
    };
    for (key, value) in call::passive_props(passive) {
        props.insert(key, value);
    }
    let stream = pw::stream::StreamRc::new(core, "wholeheard-capture-me", props).map_err(|e| failed("no stream", e))?;
    let _listener = stream
        .add_local_listener_with_user_data(listening)
        .param_changed({
            let main_loop = main_loop.clone();
            move |_, listening, id, param| {
                let Some(param) = param else { return };
                if id != spa::param::ParamType::Format.as_raw() {
                    return;
                }
                if !matches!(format_utils::parse_format(param), Ok((MediaType::Audio, MediaSubtype::Raw))) {
                    return;
                }
                let mut info = AudioInfoRaw::new();
                if info.parse(param).is_err() {
                    return;
                }
                let now = (info.rate(), info.channels() as u16);
                if *listening.form.get_or_init(|| listening.asked.unwrap_or(now)) != now {
                    // Not the track's form: the graph is asked again, with the track's.
                    warn!("capture.me_form_changed");
                    listening.alive.store(false, Ordering::SeqCst);
                    main_loop.quit();
                }
            }
        })
        .state_changed({
            let main_loop = main_loop.clone();
            move |_, listening, _, state| {
                if matches!(state, pw::stream::StreamState::Error(_) | pw::stream::StreamState::Unconnected) {
                    // Its detail is the system's, and may name the source: never logged.
                    warn!("capture.me_lost");
                    listening.alive.store(false, Ordering::SeqCst);
                    main_loop.quit();
                }
            }
        })
        .process(|stream, listening| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let Some((_, channels)) = listening.form.get().copied() else {
                return;
            };
            let Some(data) = buffer.datas_mut().first_mut() else { return };
            let (offset, size) = (data.chunk().offset() as usize, data.chunk().size() as usize);
            let Some(bytes) = data.data().and_then(|bytes| bytes.get(offset..offset + size)) else {
                return;
            };
            // The instant this stream's thread was given it, so with that thread's latency, where
            // cpal's are placed by the stream's own clock. Nothing here waits.
            let at = listening.zero.elapsed().as_secs_f64();
            let _ = listening.sound.send((listening.index, at, call::whole_frames(bytes, channels)));
        })
        .register()
        .map_err(|e| failed("no listener", e))?;

    let mut format = AudioInfoRaw::new();
    format.set_format(AudioFormat::F32LE);
    if let Some((rate, channels)) = form {
        format.set_rate(rate);
        format.set_channels(u32::from(channels));
    }
    let values = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: format.into(),
        }),
    )
    .map_err(|_| "no format to ask for".to_string())?
    .0
    .into_inner();
    let mut params = [Pod::from_bytes(&values).ok_or("no format to ask for")?];
    stream
        .connect(
            spa::utils::Direction::Input,
            None,
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| failed("not connected", e))?;
    main_loop.run();
    Ok(())
}
