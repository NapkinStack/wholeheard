//! Which applications take a microphone (D30), read from what PipeWire says of its graph — the
//! batches `pw-dump --monitor` prints: the whole graph first, then each change, an object gone
//! being its id with no info. The batches here are made for the tests, in that form.

use serde_json::{Value, json};
use wholeheard_shell::microphone::{Change, Takers};

fn client(id: u64, name: &str, pid: u64, binary: &str) -> Value {
    json!({ "id": id, "type": "PipeWire:Interface:Client", "info": { "props": {
        "application.name": name, "application.process.id": pid, "application.process.binary": binary } } })
}

fn device(id: u64, class: &str) -> Value {
    json!({ "id": id, "type": "PipeWire:Interface:Node", "info": { "props": { "media.class": class, "node.name": format!("device-{id}") } } })
}

fn stream(id: u64, client: u64, name: &str, more: Value) -> Value {
    let mut props = json!({ "media.class": "Stream/Input/Audio", "client.id": client, "application.name": name });
    if let Value::Object(more) = more {
        props.as_object_mut().unwrap().extend(more);
    }
    json!({ "id": id, "type": "PipeWire:Interface:Node", "info": { "props": props } })
}

fn link(id: u64, from: u64, to: u64) -> Value {
    json!({ "id": id, "type": "PipeWire:Interface:Link", "info": { "output-node-id": from, "input-node-id": to } })
}

fn gone(id: u64) -> Value {
    json!({ "id": id, "info": null })
}

const MIC: u64 = 10;
const SPEAKER: u64 = 11;

fn graph() -> Takers {
    let mut takers = Takers::default();
    let first = takers.read(&json!([device(MIC, "Audio/Source"), device(SPEAKER, "Audio/Sink")]));
    assert!(first.is_empty(), "{first:?}");
    takers
}

#[test]
fn an_application_that_takes_a_microphone_is_said_once_and_its_letting_go_too() {
    let mut takers = graph();
    // Firefox opens a stream: said only once it reads a microphone, not before it is linked.
    assert!(
        takers
            .read(&json!([
                client(20, "Firefox", 4242, "firefox"),
                stream(30, 20, "Firefox", json!({}))
            ]))
            .is_empty()
    );
    assert_eq!(
        takers.read(&json!([link(40, MIC, 30)])),
        [Change::Took {
            key: 4242,
            name: "Firefox".into()
        }]
    );
    // A change of its stream that carries only its state: still held.
    assert!(takers.read(&json!([{ "id": 30, "type": "PipeWire:Interface:Node", "info": { "state": "running" } }])).is_empty());
    // A second stream of the same application: nothing more.
    assert!(
        takers
            .read(&json!([stream(31, 20, "Firefox", json!({})), link(41, MIC, 31)]))
            .is_empty()
    );
    // One of them gone: it still holds one.
    assert!(takers.read(&json!([gone(41), gone(31)])).is_empty());
    // The last gone: it let go.
    assert_eq!(takers.read(&json!([gone(40), gone(30)])), [Change::Let { key: 4242 }]);
    // Taken again: said again.
    assert_eq!(
        takers.read(&json!([stream(32, 20, "Firefox", json!({})), link(42, MIC, 32)])),
        [Change::Took {
            key: 4242,
            name: "Firefox".into()
        }]
    );
}

#[test]
fn what_reads_the_computer_s_sound_a_level_meter_and_wholeheard_itself_take_no_microphone() {
    let mut takers = graph();
    let batch = json!([
        // Reading what the computer plays: a sink's monitor, or said so by the stream.
        client(21, "OBS", 100, "obs"),
        stream(33, 21, "OBS", json!({})),
        link(43, SPEAKER, 33),
        stream(34, 21, "OBS", json!({ "stream.capture.sink": "true" })),
        link(44, MIC, 34),
        // A level meter: Settings, pavucontrol.
        client(22, "pavucontrol", 200, "pavucontrol"),
        stream(35, 22, "Peak detect", json!({ "stream.monitor": "true" })),
        link(45, MIC, 35),
        // Wholeheard's own capture.
        client(23, "wholeheard-capture", 300, "wholeheard-capture"),
        stream(36, 23, "wholeheard-capture", json!({})),
        link(46, MIC, 36),
    ]);
    assert!(takers.read(&batch).is_empty(), "{:?}", takers.read(&json!([])));
}

#[test]
fn an_application_is_known_by_its_process_and_named_by_its_name() {
    let mut takers = graph();
    // Two clients of one process — a browser's tabs: one application.
    let batch = json!([
        client(24, "Chromium", 555, "chromium"),
        client(25, "Chromium input", 555, "chromium"),
        stream(37, 24, "WebRTC VoiceEngine", json!({})),
        link(47, MIC, 37),
        stream(38, 25, "Chromium", json!({})),
        link(48, MIC, 38),
    ]);
    assert_eq!(
        takers.read(&batch),
        [Change::Took {
            key: 555,
            name: "Chromium".into()
        }]
    );
    // A client gone with its streams: let go.
    assert_eq!(
        takers.read(&json!([gone(47), gone(37), gone(48), gone(38), gone(24), gone(25)])),
        [Change::Let { key: 555 }]
    );
    // A client with no process said: known by its client, named by its stream.
    assert_eq!(
        takers.read(&json!([{ "id": 26, "type": "PipeWire:Interface:Client", "info": { "props": {} } }, stream(39, 26, "Discord", json!({})), link(49, MIC, 39)])),
        [Change::Took { key: 1 << 40 | 26, name: "Discord".into() }]
    );
}

#[test]
fn what_is_not_a_batch_of_the_graph_changes_nothing() {
    let mut takers = graph();
    for odd in [json!(null), json!(7), json!({}), json!([1, "x", { "id": "no" }, { "info": {} }])] {
        assert!(takers.read(&odd).is_empty(), "{odd}");
    }
}

#[test]
fn an_application_is_offered_once_while_it_holds_a_microphone_never_while_recording_nor_when_offers_are_off() {
    use wholeheard_shell::microphone::Offers;
    let took = |key: u64| Change::Took {
        key,
        name: format!("app {key}"),
    };
    let mut offers = Offers::default();
    // Offered once.
    assert_eq!(offers.on(&took(1), false, true), Some("app 1".to_string()));
    assert_eq!(offers.on(&took(1), false, true), None);
    // Let go, taken again: offered again — "Not this time" lasts until then.
    assert_eq!(offers.on(&Change::Let { key: 1 }, false, true), None);
    assert_eq!(offers.on(&took(1), false, true), Some("app 1".to_string()));
    // Never while recording, and not after it for the same hold.
    assert_eq!(offers.on(&took(2), true, true), None);
    assert_eq!(offers.on(&took(2), false, true), None);
    // Offers off: none, until they are on again and the application takes one again.
    assert_eq!(offers.on(&took(3), false, false), None);
    assert_eq!(offers.on(&Change::Let { key: 3 }, false, true), None);
    assert_eq!(offers.on(&took(3), false, true), Some("app 3".to_string()));
}

#[test]
fn the_graph_as_pw_dump_writes_it_booleans_virtual_and_duplex_devices_links_and_clients_gone() {
    // pw-dump writes a flag as a JSON boolean.
    let mut takers = graph();
    let batch = json!([
        device(12, "Audio/Source/Virtual"),
        device(13, "Audio/Duplex"),
        client(27, "OBS", 700, "obs"),
        stream(50, 27, "OBS", json!({ "stream.capture.sink": true })),
        link(60, MIC, 50),
        client(28, "Meter", 701, "meter"),
        stream(51, 28, "Meter", json!({ "stream.monitor": true })),
        link(61, MIC, 51),
    ]);
    assert!(takers.read(&batch).is_empty());
    // A virtual microphone, a duplex device: microphones.
    assert_eq!(
        takers.read(&json!([
            client(29, "Zoom", 702, "zoom"),
            stream(52, 29, "Zoom", json!({})),
            link(62, 12, 52)
        ])),
        [Change::Took {
            key: 702,
            name: "Zoom".into()
        }]
    );
    assert_eq!(
        takers.read(&json!([
            client(30, "Teams", 703, "teams"),
            stream(53, 30, "Teams", json!({})),
            link(63, 13, 53)
        ])),
        [Change::Took {
            key: 703,
            name: "Teams".into()
        }]
    );
    // Its link gone, its stream still there: it lets go.
    assert_eq!(takers.read(&json!([gone(62)])), [Change::Let { key: 702 }]);
    // Its client gone before its streams, as a program ending may be read: the same application,
    // known and named as it was — never a new one.
    assert!(takers.read(&json!([gone(30)])).is_empty());
    assert_eq!(takers.read(&json!([gone(63), gone(53)])), [Change::Let { key: 703 }]);
    // Wholeheard's own capture, its client gone first: still its own.
    takers.read(&json!([
        client(31, "wholeheard-capture", 704, "wholeheard-capture"),
        stream(54, 31, "wholeheard-capture", json!({})),
        link(64, MIC, 54)
    ]));
    assert!(takers.read(&json!([gone(31)])).is_empty());
}

#[test]
fn the_graph_read_anew_after_pw_dump_started_again_says_only_what_changed() {
    let mut takers = graph();
    let firefox = json!([
        client(20, "Firefox", 4242, "firefox"),
        stream(30, 20, "Firefox", json!({})),
        link(40, MIC, 30)
    ]);
    assert_eq!(takers.read(&firefox).len(), 1);
    // pw-dump started again: the whole graph again, Firefox still holding — nothing to say.
    takers.anew();
    let mut whole = vec![device(MIC, "Audio/Source"), device(SPEAKER, "Audio/Sink")];
    whole.extend(firefox.as_array().unwrap().iter().cloned());
    assert!(takers.read(&Value::Array(whole)).is_empty());
    // Started again without it: it let go meanwhile.
    takers.anew();
    assert_eq!(takers.read(&json!([device(MIC, "Audio/Source")])), [Change::Let { key: 4242 }]);
}
