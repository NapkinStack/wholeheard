//! Where a call plays, read from PipeWire's graph as `pw-dump` prints it (D41, ADR-0011): the
//! graphs here are written for the test, shaped as the decider's of #436 was — no device opened.

use serde_json::{Value, json};
use wholeheard_capture::call::{self, Output};

const OURS: u64 = 999;

fn node(id: u64, class: &str, props: Value) -> Value {
    let mut props = props;
    props["media.class"] = class.into();
    json!({ "id": id, "type": "PipeWire:Interface:Node", "info": { "props": props } })
}

fn link(id: u64, from: u64, to: u64) -> Value {
    json!({ "id": id, "type": "PipeWire:Interface:Link", "info": { "output-node-id": from, "input-node-id": to } })
}

fn default_sink(name: &str) -> Value {
    json!({
        "id": 30, "type": "PipeWire:Interface:Metadata", "props": { "metadata.name": "default" },
        "metadata": [{ "subject": 0, "key": "default.audio.sink", "type": "Spa:String:JSON", "value": { "name": name } }]
    })
}

/// The built-in speaker the default, earbuds beside it; a call (process 143) holding the earbuds'
/// microphone and playing on `plays_on`; capture's own two streams, the remote on the speaker.
fn graph(plays_on: &[u64]) -> Vec<Value> {
    let mut graph = vec![
        node(
            49,
            "Audio/Sink",
            json!({ "node.name": "alsa_speaker", "node.description": "Built-in Speaker" }),
        ),
        node(
            136,
            "Audio/Sink",
            json!({ "node.name": "bluez_earbuds", "node.description": "Earbuds" }),
        ),
        node(
            134,
            "Audio/Source",
            json!({ "node.name": "bluez_earbuds_mic", "node.description": "Earbuds" }),
        ),
        node(
            131,
            "Stream/Input/Audio",
            json!({ "application.process.id": 143, "application.name": "a call" }),
        ),
        node(
            80,
            "Stream/Output/Audio",
            json!({ "application.process.id": 143, "application.name": "a call" }),
        ),
        node(
            500,
            "Stream/Input/Audio",
            json!({ "application.process.id": OURS, "stream.capture.sink": "true" }),
        ),
        node(501, "Stream/Input/Audio", json!({ "application.process.id": OURS })),
        default_sink("alsa_speaker"),
        link(1, 134, 131),
        link(2, 49, 500),
        link(3, 134, 501),
    ];
    for (n, to) in plays_on.iter().enumerate() {
        // One link a channel: two for a stereo stream.
        graph.push(link(10 + 2 * n as u64, 80, *to));
        graph.push(link(11 + 2 * n as u64, 80, *to));
    }
    graph
}

fn read(graph: Vec<Value>) -> Result<Option<Output>, String> {
    call::output(&Value::Array(graph).to_string(), OURS)
}

#[test]
fn a_call_that_plays_on_another_output_than_the_default_is_followed_there() {
    let found = read(graph(&[136])).unwrap().expect("the earbuds");
    assert_eq!(
        found,
        Output {
            name: "bluez_earbuds".into(),
            description: "Earbuds".into()
        }
    );
}

#[test]
fn a_followed_output_without_a_description_is_named_as_the_graph_names_it() {
    // As cpal and `call::linked` name it (#458): one name for one device, never its node's.
    let mut nameless = graph(&[136]);
    for object in &mut nameless {
        if let Some(props) = object.pointer_mut("/info/props").and_then(Value::as_object_mut)
            && props.get("node.name").and_then(Value::as_str) == Some("bluez_earbuds")
        {
            props.remove("node.description");
        }
    }
    let found = read(nameless).unwrap().expect("the earbuds");
    assert_eq!((found.name.as_str(), found.description.as_str()), ("bluez_earbuds", "unknown"));
}

#[test]
fn a_call_on_the_default_several_outputs_or_none_leaves_the_default() {
    assert_eq!(read(graph(&[49])).unwrap(), None, "on the default");
    assert_eq!(read(graph(&[49, 136])).unwrap(), None, "on two outputs");
    assert_eq!(read(graph(&[])).unwrap(), None, "playing nothing yet");
    // Two outputs, neither the default: no one to follow.
    let mut two = graph(&[136, 137]);
    two.push(node(
        137,
        "Audio/Sink",
        json!({ "node.name": "usb_headset", "node.description": "Headset" }),
    ));
    assert_eq!(read(two).unwrap(), None, "on two outputs, neither the default");
}

#[test]
fn no_call_leaves_the_default() {
    // The application plays and holds no microphone: a video, music — no call.
    let mut quiet = graph(&[136]);
    quiet.retain(|object| object["id"] != 131);
    assert_eq!(read(quiet).unwrap(), None);
}

#[test]
fn what_records_an_output_is_no_call() {
    // Another recorder of what plays — a stream recording a sink — takes no microphone.
    let mut recorder = graph(&[136]);
    for object in &mut recorder {
        if object["id"] == 131 {
            object["info"]["props"]["stream.capture.sink"] = "true".into();
        }
    }
    assert_eq!(read(recorder).unwrap(), None);
}

#[test]
fn capture_itself_is_no_call() {
    // capture holds a microphone and plays nothing; were it to, it would not follow itself.
    let mut ours = graph(&[]);
    ours.push(node(502, "Stream/Output/Audio", json!({ "application.process.id": OURS })));
    ours.push(link(40, 502, 136));
    assert_eq!(read(ours).unwrap(), None);
}

#[test]
fn the_remote_stream_is_capture_s_own_recording_of_an_output() {
    let dump = Value::Array(graph(&[136])).to_string();
    assert_eq!(call::remote_stream(&dump, OURS), Some(500));
    assert_eq!(call::remote_stream(&dump, 12345), None);
}

#[test]
fn a_graph_that_does_not_read_is_said_so() {
    assert!(call::output("not json", OURS).is_err());
    assert!(call::output("{}", OURS).is_err());
}

#[test]
fn a_graph_with_no_default_follows_the_one_output_the_call_plays_on() {
    let mut without = graph(&[136]);
    without.retain(|object| object["type"] != "PipeWire:Interface:Metadata");
    assert_eq!(read(without).unwrap().map(|output| output.name), Some("bluez_earbuds".into()));
}

#[test]
fn a_stream_says_its_process_through_its_client_when_it_does_not_itself() {
    // A native PipeWire client — pw-loopback, cpal — names its process on the client alone.
    let client = |id: u64, pid: u64| json!({ "id": id, "type": "PipeWire:Interface:Client", "info": { "props": { "application.process.id": pid, "pipewire.sec.pid": pid } } });
    let mut native = graph(&[136]);
    for object in &mut native {
        let props = &mut object["info"]["props"];
        if let Some(pid) = props.get("application.process.id").and_then(Value::as_u64) {
            let owner = if pid == OURS { 71 } else { 70 };
            props.as_object_mut().unwrap().remove("application.process.id");
            props["client.id"] = owner.into();
        }
    }
    native.push(client(70, 143));
    native.push(client(71, OURS));
    let dump = Value::Array(native.clone()).to_string();
    assert_eq!(read(native).unwrap().map(|output| output.name), Some("bluez_earbuds".into()));
    assert_eq!(call::remote_stream(&dump, OURS), Some(500));
}

#[test]
fn clients_of_pulseaudio_are_not_one_process() {
    // A microphone held by one Pulse client and music played by another: no call to follow.
    let pulse = |id: u64| json!({ "id": id, "type": "PipeWire:Interface:Client", "info": { "props": { "pipewire.sec.pid": 4242 } } });
    let mut graph = graph(&[136]);
    for object in &mut graph {
        let owner = if object["id"] == 131 { 72 } else { 73 };
        let props = &mut object["info"]["props"];
        if props.get("application.process.id").and_then(Value::as_u64) == Some(143) {
            props.as_object_mut().unwrap().remove("application.process.id");
            props["client.id"] = owner.into();
        }
    }
    graph.push(pulse(72));
    graph.push(pulse(73));
    assert_eq!(read(graph).unwrap(), None);
}

#[test]
fn each_of_capture_s_streams_is_named_by_the_node_it_is_fed_from() {
    // The microphone stream fed by the earbuds' microphone, the remote one by the speaker's monitor.
    let dump = Value::Array(graph(&[136])).to_string();
    assert_eq!(
        call::linked(&dump, OURS),
        [Some("Earbuds".to_string()), Some("Built-in Speaker".to_string())]
    );
    // Not in the graph, or fed by nothing: nothing to name.
    assert_eq!(call::linked(&dump, 12345), [None, None]);
    let mut unlinked = graph(&[]);
    unlinked.retain(|object| object["id"] != 3);
    assert_eq!(call::linked(&Value::Array(unlinked).to_string(), OURS)[0], None);
    // A node without a description is "unknown", as cpal names it: one name for one device.
    let mut nameless = graph(&[136]);
    for object in &mut nameless {
        if let Some(props) = object.pointer_mut("/info/props").and_then(Value::as_object_mut)
            && props.get("node.name").and_then(Value::as_str) == Some("bluez_earbuds_mic")
        {
            props.remove("node.description");
        }
    }
    assert_eq!(
        call::linked(&Value::Array(nameless).to_string(), OURS)[0].as_deref(),
        Some("unknown")
    );
}

// ---- The microphone a call holds, and what `me` records (D66, ADR-0015).

use wholeheard_capture::call::{Me, Microphone, Step};

const AIRPODS: &str = "bluez_input.00:00:5E:00:53:01";

fn default_source(name: &str) -> Value {
    json!({
        "id": 31, "type": "PipeWire:Interface:Metadata", "props": { "metadata.name": "default" },
        "metadata": [{ "subject": 0, "key": "default.audio.source", "type": "Spa:String:JSON", "value": { "name": name } }]
    })
}

/// The headset's virtual input and a laptop's microphone; capture's own two streams, the remote
/// on a speaker, the local on nothing yet.
fn sources() -> Vec<Value> {
    vec![
        node(
            134,
            "Audio/Source",
            json!({ "node.name": AIRPODS, "node.description": "AirPods", "bluez5.loopback": "true" }),
        ),
        node(
            50,
            "Audio/Source",
            json!({ "node.name": "alsa_input.laptop", "node.description": "Digital Microphone" }),
        ),
        node(
            49,
            "Audio/Sink",
            json!({ "node.name": "alsa_speaker", "node.description": "Speaker" }),
        ),
        node(
            500,
            "Stream/Input/Audio",
            json!({ "application.process.id": OURS, "stream.capture.sink": "true" }),
        ),
        node(501, "Stream/Input/Audio", json!({ "application.process.id": OURS })),
        link(2, 49, 500),
    ]
}

fn stream(id: u64, process: u64) -> Value {
    node(
        id,
        "Stream/Input/Audio",
        json!({ "application.process.id": process, "application.name": "Firefox" }),
    )
}

fn me(graph: Vec<Value>) -> Me {
    call::me(&Value::Array(graph).to_string(), OURS).unwrap()
}

fn held(name: &str, description: &str, loopback: bool) -> Me {
    Me::Hold(Microphone {
        name: name.into(),
        description: description.into(),
        loopback,
    })
}

#[test]
fn firefox_on_the_headset_s_virtual_input_holds_it_by_its_description_and_says_it_is_a_loopback() {
    let mut graph = sources();
    graph.extend([stream(131, 143), link(3, 134, 131), default_source("alsa_input.laptop")]);
    assert_eq!(me(graph), held(AIRPODS, "AirPods", true));
}

#[test]
fn a_call_on_a_microphone_other_than_the_default_holds_that_one() {
    let mut graph = sources();
    graph.extend([stream(131, 143), link(3, 50, 131), default_source(AIRPODS)]);
    assert_eq!(me(graph), held("alsa_input.laptop", "Digital Microphone", false));
}

#[test]
fn a_call_on_the_default_input_is_the_default_when_it_is_no_loopback_and_passive_when_it_is() {
    let mut graph = sources();
    graph.extend([stream(131, 143), link(3, 50, 131), default_source("alsa_input.laptop")]);
    assert_eq!(me(graph), Me::Default, "today's stream, which the system moves");
    let mut graph = sources();
    graph.extend([stream(131, 143), link(3, 134, 131), default_source(AIRPODS)]);
    assert_eq!(me(graph), held(AIRPODS, "AirPods", true));
}

#[test]
fn no_call_records_the_default_unless_it_is_a_loopback_then_it_waits() {
    let mut graph = sources();
    graph.push(default_source("alsa_input.laptop"));
    assert_eq!(me(graph), Me::Default);
    let mut graph = sources();
    graph.push(default_source(AIRPODS));
    assert_eq!(me(graph), Me::Wait);
    assert_eq!(me(sources()), Me::Default, "no default named: nothing says Bluetooth");
    let mut graph = sources();
    graph.push(default_source("a_source_gone"));
    assert_eq!(me(graph), Me::Default);
}

#[test]
fn several_calls_the_one_running_wins_then_the_oldest_stream_whatever_the_order_read() {
    let running = |mut stream: Value| {
        stream["info"]["state"] = "running".into();
        stream
    };
    let mut graph = sources();
    graph.extend([stream(131, 143), stream(140, 144), link(3, 134, 131), link(4, 50, 140)]);
    graph.push(default_source(AIRPODS));
    assert_eq!(me(graph.clone()), held(AIRPODS, "AirPods", true), "none running: the lowest id");
    graph.reverse();
    assert_eq!(me(graph.clone()), held(AIRPODS, "AirPods", true), "the order read changes nothing");
    let mut graph = sources();
    graph.extend([stream(131, 143), running(stream(140, 144)), link(3, 134, 131), link(4, 50, 140)]);
    assert_eq!(me(graph), held("alsa_input.laptop", "Digital Microphone", false));
}

#[test]
fn what_is_no_call_holds_no_microphone() {
    let not_a_call = |props: Value| {
        let mut graph = sources();
        let mut props = props;
        props["application.process.id"] = 143.into();
        graph.extend([node(131, "Stream/Input/Audio", props), link(3, 134, 131)]);
        graph.push(default_source("alsa_input.laptop"));
        me(graph)
    };
    assert_eq!(not_a_call(json!({ "stream.monitor": "true" })), Me::Default, "a level meter");
    assert_eq!(
        not_a_call(json!({ "stream.capture.sink": "true" })),
        Me::Default,
        "a recorder of what plays"
    );
    assert_eq!(
        not_a_call(json!({ "node.link-group": "filter" })),
        Me::Default,
        "a filter's own input"
    );
    assert_eq!(
        not_a_call(json!({ "bluez5.loopback": "true" })),
        Me::Default,
        "the loopback's own input"
    );
    assert_eq!(
        not_a_call(json!({ "application.process.binary": "wholeheard" })),
        Me::Default,
        "Wholeheard's own"
    );
    // This process's own: capture linked to the headset is not a call taking it.
    let mut graph = sources();
    graph.extend([link(3, 134, 501), default_source("alsa_input.laptop")]);
    assert_eq!(me(graph), Me::Default);
    // Wholeheard's own named on its client alone, as a native client does.
    let mut graph = sources();
    let mut own = node(131, "Stream/Input/Audio", json!({ "client.id": 70 }));
    own["info"]["props"]["application.name"] = "wholeheard".into();
    graph.extend([
        own,
        link(3, 134, 131),
        default_source("alsa_input.laptop"),
        json!({ "id": 70, "type": "PipeWire:Interface:Client", "info": { "props": {
            "application.process.id": 77, "application.process.binary": "wholeheard-capture" } } }),
    ]);
    assert_eq!(me(graph), Me::Default);
    // What reads an output's monitor takes no microphone.
    let mut graph = sources();
    graph.extend([stream(131, 143), link(3, 49, 131), default_source("alsa_input.laptop")]);
    assert_eq!(me(graph), Me::Default);
}

#[test]
fn through_a_filter_chain_the_call_holds_the_filter_s_source_and_its_loopback_is_what_lies_behind() {
    let chain = |root: u64| {
        let mut graph = sources();
        graph.extend([
            node(
                60,
                "Audio/Source",
                json!({ "node.name": "effect_output", "node.description": "Noise cancelling", "node.link-group": "filter" }),
            ),
            node(
                61,
                "Stream/Input/Audio",
                json!({ "application.process.id": 300, "node.name": "effect_input", "node.link-group": "filter" }),
            ),
            link(5, root, 61),
            stream(131, 143),
            link(3, 60, 131),
            default_source("alsa_input.laptop"),
        ]);
        me(graph)
    };
    assert_eq!(chain(134), held("effect_output", "Noise cancelling", true));
    assert_eq!(chain(50), held("effect_output", "Noise cancelling", false));
}

#[test]
fn through_a_filter_s_playing_stream_the_call_holds_the_source_behind_it() {
    let mut graph = sources();
    graph.extend([
        node(
            60,
            "Stream/Output/Audio",
            json!({ "application.process.id": 300, "node.name": "effect_output", "node.link-group": "filter" }),
        ),
        node(
            61,
            "Stream/Input/Audio",
            json!({ "application.process.id": 300, "node.name": "effect_input", "node.link-group": "filter" }),
        ),
        link(5, 134, 61),
        stream(131, 143),
        link(3, 60, 131),
        default_source("alsa_input.laptop"),
    ]);
    assert_eq!(me(graph), held(AIRPODS, "AirPods", true));
}

#[test]
fn filters_linked_in_a_circle_end() {
    let mut graph = sources();
    graph.extend([
        node(
            60,
            "Stream/Output/Audio",
            json!({ "application.process.id": 300, "node.link-group": "a" }),
        ),
        node(
            61,
            "Stream/Input/Audio",
            json!({ "application.process.id": 300, "node.link-group": "a" }),
        ),
        link(5, 60, 61),
        stream(131, 143),
        link(3, 60, 131),
        default_source("alsa_input.laptop"),
    ]);
    assert_eq!(me(graph), Me::Default);
}

#[test]
fn a_graph_that_does_not_read_is_said_so_and_the_default_s_name_alone_decides() {
    assert!(call::me("not json", OURS).is_err());
    assert_eq!(call::me_unread(Some(AIRPODS)), Me::Wait, "never an active open on Bluetooth");
    assert_eq!(call::me_unread(Some("alsa_input.laptop")), Me::Default, "today's behaviour");
    assert_eq!(call::me_unread(None), Me::Default, "today's behaviour");
}

#[test]
fn whether_the_default_input_is_a_loopback_for_the_first_form() {
    let mut graph = sources();
    graph.push(default_source(AIRPODS));
    assert!(call::default_is_loopback(&Value::Array(graph).to_string()));
    let mut graph = sources();
    graph.push(default_source("alsa_input.laptop"));
    assert!(!call::default_is_loopback(&Value::Array(graph).to_string()));
    assert!(!call::default_is_loopback("not json"));
}

#[test]
fn what_a_change_of_the_graph_calls_for() {
    let airpods = held(AIRPODS, "AirPods", true);
    let laptop = held("alsa_input.laptop", "Digital Microphone", false);
    // Nothing changed: nothing moves.
    assert_eq!(call::step(&airpods, true, &airpods), Step::Keep);
    assert_eq!(call::step(&Me::Default, true, &Me::Default), Step::Keep);
    assert_eq!(call::step(&Me::Wait, false, &Me::Wait), Step::Keep);
    // The call lets go: capture lets go, it never holds the source alone.
    assert_eq!(call::step(&airpods, true, &Me::Wait), Step::Let);
    assert_eq!(
        call::step(&Me::Default, true, &Me::Wait),
        Step::Let,
        "the default became a loopback"
    );
    // The call takes one, or changes it, or the stream failed: another stream.
    assert_eq!(call::step(&Me::Wait, false, &airpods), Step::Take);
    assert_eq!(call::step(&airpods, true, &laptop), Step::Take);
    assert_eq!(call::step(&airpods, false, &airpods), Step::Take, "a transport failure");
    assert_eq!(call::step(&airpods, true, &Me::Default), Step::Take);
}

#[test]
fn a_bluetooth_address_never_passes() {
    assert_eq!(call::without_address("AirPods de F"), "AirPods de F");
    for address in ["00:00:5E:00:53:01", "00-00-5e-00-53-01", "00_00_5E_00_53_01"] {
        assert_eq!(call::without_address(address), "unknown", "{address}");
        let said = call::without_address(&format!("bluez_input.{address}.1 failed"));
        assert!(!said.contains("5E") && !said.contains("5e"), "{said}");
    }
    // Hexadecimal that is no address stays.
    assert_eq!(call::without_address("12:30 ab-cd"), "12:30 ab-cd");
    // A headset the system could only name by its address, in the graph.
    let mut graph = sources();
    graph[0]["info"]["props"]["node.description"] = "00-00-5E-00-53-01".into();
    graph.extend([stream(131, 143), link(3, 134, 131), default_source("alsa_input.laptop")]);
    assert_eq!(me(graph), held(AIRPODS, "unknown", true));
}

#[test]
fn only_whole_frames_of_what_the_passive_stream_delivered_are_handed_on() {
    let bytes: Vec<u8> = [0.5f32, -0.5, 0.25].iter().flat_map(|sample| sample.to_le_bytes()).collect();
    assert_eq!(call::whole_frames(&bytes, 2), [0.5, -0.5]);
    assert_eq!(call::whole_frames(&bytes, 1), [0.5, -0.5, 0.25]);
    assert_eq!(call::whole_frames(&bytes[..3], 1), [] as [f32; 0]);
    assert_eq!(call::whole_frames(&bytes, 0), [] as [f32; 0]);
}

#[test]
fn the_passive_stream_says_it_follows_and_is_never_moved_elsewhere() {
    let says = |key: &str| call::PASSIVE_PROPS.iter().find(|(name, _)| *name == key).map(|(_, value)| *value);
    assert_eq!(says("node.passive"), Some("in-follow"));
    assert_eq!(
        says("node.dont-fallback"),
        Some("true"),
        "a source gone is never the default instead"
    );
    assert_eq!(says("node.dont-reconnect"), Some("true"));
    assert_eq!(says("stream.monitor"), None, "never lying to WirePlumber's script (ADR-0015)");
    assert_eq!(says("node.group"), None);
}
