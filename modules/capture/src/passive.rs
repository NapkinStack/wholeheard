//! The local side's stream on a headset's virtual input (ADR-0015): `node.passive=in-follow`, so it
//! never makes the source run — it is carried by the call that does, and nothing it does asks for
//! the headset's profile. cpal cannot say that of a stream, nor leave a stream out of its
//! `node.group`, so this one source is opened with the library cpal itself is built on.
//!
//! It has a thread and a main loop of its own, and delivers what it hears as cpal's streams do. A
//! stream that fails only says so: the recording goes on, and the graph says what comes next.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
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

/// A link and its format take the session manager a moment; beyond this, the graph is asked again.
const NEGOTIATED_WITHIN: Duration = Duration::from_secs(2);

/// A form: frames a second, and channels.
type Form = (u32, u16);

/// The stream. Dropped, it has left the graph.
pub struct Passive {
    stop: pw::channel::Sender<()>,
    thread: Option<JoinHandle<()>>,
    alive: Arc<AtomicBool>,
}

impl Passive {
    /// Listens to the source named `target`, and hands what it delivers to `sound` as source
    /// `index`. `form`: the one a track already has, into which the system converts, as it does
    /// when a device changes; none, the one it negotiates, returned.
    pub fn open(target: &str, form: Option<Form>, index: usize, zero: Instant, sound: Sender<Sound>) -> Result<(Passive, Form), String> {
        let (stop, stopped) = pw::channel::channel::<()>();
        let (told, given) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let thread = {
            let (target, alive, run) = (target.to_string(), alive.clone(), tracing::Span::current());
            std::thread::spawn(move || {
                let _run = run.enter();
                let listening = Listening {
                    asked: form,
                    form: None,
                    told: told.clone(),
                    alive: alive.clone(),
                    index,
                    zero,
                    sound,
                };
                if let Err(detail) = listen(&target, form, stopped, listening) {
                    alive.store(false, Ordering::SeqCst);
                    let _ = told.send(Err(detail));
                }
            })
        };
        let stream = Passive {
            stop,
            thread: Some(thread),
            alive,
        };
        let negotiated = given
            .recv_timeout(NEGOTIATED_WITHIN)
            .unwrap_or_else(|_| Err("no format was negotiated in time".to_string()));
        // Dropped on the way out, when it failed: it leaves the graph.
        Ok((stream, negotiated?))
    }

    /// False once it failed: its source gone, or its format changed.
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

impl Drop for Passive {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Listening {
    asked: Option<Form>,
    form: Option<Form>,
    told: Sender<Result<Form, String>>,
    alive: Arc<AtomicBool>,
    index: usize,
    zero: Instant,
    sound: Sender<Sound>,
}

fn listen(target: &str, form: Option<Form>, stopped: pw::channel::Receiver<()>, listening: Listening) -> Result<(), String> {
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
    for (key, value) in call::PASSIVE_PROPS {
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
                if listening.form.is_some_and(|form| form != now) || listening.asked.is_some_and(|asked| asked != now) {
                    // Not the track's form: the graph is asked again, with the track's.
                    warn!("capture.me_form_changed");
                    let _ = listening.told.send(Err("another format was negotiated".to_string()));
                    listening.alive.store(false, Ordering::SeqCst);
                    main_loop.quit();
                    return;
                }
                if listening.form.is_none() {
                    listening.form = Some(now);
                    let _ = listening.told.send(Ok(now));
                }
            }
        })
        .state_changed({
            let main_loop = main_loop.clone();
            move |_, listening, _, state| {
                if matches!(state, pw::stream::StreamState::Error(_) | pw::stream::StreamState::Unconnected) {
                    // Its detail is the system's, and may name the source: never logged.
                    warn!("capture.me_lost");
                    let _ = listening.told.send(Err("the stream failed".to_string()));
                    listening.alive.store(false, Ordering::SeqCst);
                    main_loop.quit();
                }
            }
        })
        .process(|stream, listening| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let Some((_, channels)) = listening.form else { return };
            let Some(data) = buffer.datas_mut().first_mut() else { return };
            let (offset, size) = (data.chunk().offset() as usize, data.chunk().size() as usize);
            let Some(bytes) = data.data().and_then(|bytes| bytes.get(offset..offset + size)) else {
                return;
            };
            // The instant it was handed over; this thread is the system's, and nothing here waits.
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
