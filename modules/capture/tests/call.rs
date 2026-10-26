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
