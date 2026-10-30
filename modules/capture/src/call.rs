//! Where a call plays (D41, ADR-0011), read from PipeWire's graph as `pw-dump` prints it: a call is
//! an application, capture excepted, with a stream recording a microphone; where it plays is the
//! outputs its playing streams are linked to. What it holds is the microphone the local side
//! follows (D66, ADR-0015). No device is opened here.

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;

/// An output, as PipeWire names it and as a person reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub name: String,
    pub description: String,
}

/// The output the remote side follows: the one a call plays on, when it is not the default.
/// `None` — no call, a call on the default, on several outputs, or on none yet — leaves the
/// default, as D31. A graph that does not read is said so.
pub fn output(dump: &str, ours: u64) -> Result<Option<Output>, String> {
    let graph = Graph::read(dump)?;
    let calls: BTreeSet<u64> = graph
        .nodes
        .values()
        .filter(|node| node.class == "Stream/Input/Audio" && !node.records_an_output && node.process != Some(ours))
        .filter_map(|node| node.process)
        .collect();
    let playing: BTreeSet<u64> = graph
        .nodes
        .iter()
        .filter(|(_, node)| node.class == "Stream/Output/Audio" && node.process.is_some_and(|process| calls.contains(&process)))
        .map(|(id, _)| *id)
        .collect();
    let outputs: BTreeSet<u64> = graph
        .links
        .iter()
        .filter(|(from, to)| playing.contains(from) && graph.nodes.get(to).is_some_and(|node| node.class == "Audio/Sink"))
        .map(|(_, to)| *to)
        .collect();
    let [only] = outputs.into_iter().collect::<Vec<_>>()[..] else {
        return Ok(None);
    };
    let node = &graph.nodes[&only];
    if graph.default_sink.as_deref() == Some(node.name.as_str()) {
        return Ok(None);
    }
    Ok(Some(Output {
        name: node.name.clone(),
        // Named as cpal and `linked` name it: never two names for one device (#458), never an address.
        description: node.named(),
    }))
}

/// capture's own stream recording an output — the remote side — by its node's id.
pub fn remote_stream(dump: &str, ours: u64) -> Option<u64> {
    let graph = Graph::read(dump).ok()?;
    graph
        .nodes
        .iter()
        .find(|(_, node)| node.class == "Stream/Input/Audio" && node.records_an_output && node.process == Some(ours))
        .map(|(id, _)| *id)
}

/// What feeds each of capture's own streams, as the graph links it — the microphone's, then the
/// remote's (an output's monitor) —, by the name a person reads (#352): the device a track is
/// recorded from, whatever the default says meanwhile.
pub fn linked(dump: &str, ours: u64) -> [Option<String>; 2] {
    let Ok(graph) = Graph::read(dump) else {
        return [None, None];
    };
    let fed = |remote: bool| {
        let (stream, _) = graph
            .nodes
            .iter()
            .find(|(_, node)| node.class == "Stream/Input/Audio" && node.process == Some(ours) && node.records_an_output == remote)?;
        let (from, _) = graph.links.iter().find(|(_, to)| to == stream)?;
        let node = graph.nodes.get(from)?;
        // Named as cpal names a device (cpal-0.18.2, host/pipewire/device.rs): never two names for one.
        Some(node.named())
    };
    [fed(false), fed(true)]
}

/// A microphone, as PipeWire names it and as a person reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Microphone {
    pub name: String,
    pub description: String,
    /// It is a headset's virtual input, or a filter fed by one (ADR-0015): whoever makes it run
    /// asks for the headset's profile, so it is only ever listened to passively, saying this
    /// `node.passive`. None: any other source.
    pub passive: Option<&'static str>,
}

/// What the local side records (D66, ADR-0015).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Me {
    /// No call on a microphone of its own: the default input, in the stream the system moves.
    Default,
    /// The microphone a call holds, when it is not simply the default.
    Hold(Microphone),
    /// No call, and the default input is a headset's virtual input: nothing is opened.
    Wait,
}

/// What a change of `Me` calls for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Keep,
    /// Close the stream, and wait.
    Let,
    /// Close the stream, if any, and open what is wanted.
    Take,
}

/// The `node.passive` that never makes a source run and runs when something else does (ADR-0015),
/// for the PipeWire that reads it: `pw-dump`'s `Core` version, the server's, whose parser decides.
/// To 1.6: `in`; a passive link makes nothing runnable, and a running driver carries a leaf node;
/// `in-follow` is not known there, and reads as an active stream. From 1.7: `in-follow`; the leaf
/// rule is gone, and `in` would hear nothing. A version not read: `in`, the wrong guess costing a
/// silent track, never a call.
pub fn passive_value(version: Option<&str>) -> &'static str {
    let number = |part: Option<&str>| part.and_then(|part| part.parse::<u32>().ok());
    let mut parts = version.unwrap_or_default().split('.');
    match (number(parts.next()), number(parts.next())) {
        (Some(major), Some(minor)) if (major, minor) >= (1, 7) => "in-follow",
        _ => "in",
    }
}

/// The local stream's own, on a source that must not be made to run. Nothing moves it elsewhere
/// when the source goes: it fails, and the graph says what comes next.
pub fn passive_props(value: &'static str) -> [(&'static str, &'static str); 3] {
    [
        ("node.passive", value),
        ("node.dont-fallback", "true"),
        ("node.dont-reconnect", "true"),
    ]
}

/// How long after this many failures the next try comes: a reading comes each second, and what fails
/// is not asked each time; never so long that a call's words are lost for it.
pub fn retry_after(tries: u32) -> std::time::Duration {
    std::time::Duration::from_secs(2u64.saturating_pow(tries).min(8))
}

/// Programs whose streams are Wholeheard's own — as the shell's offer reads them.
const OWN: [&str; 2] = ["wholeheard-capture", "wholeheard"];

/// What the local side should record, the graph being what it is. Of several calls, the one whose
/// stream is `running` wins, then the lowest node id — the oldest: a stream that comes later
/// takes nothing from one already recorded.
pub fn me(dump: &str, ours: u64) -> Result<Me, String> {
    let graph = Graph::read(dump)?;
    let mut calls: Vec<(&u64, &Node)> = graph
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.class == "Stream/Input/Audio"
                && !node.records_an_output
                && !node.monitor
                && !node.loopback
                && node.link_group.is_none()
                && !node.own
                && node.process != Some(ours)
        })
        .collect();
    calls.sort_by_key(|(id, node)| (!node.running, **id));
    let held = calls.iter().find_map(|(id, _)| {
        let mut froms: Vec<u64> = graph.links.iter().filter(|(_, to)| to == *id).map(|(from, _)| *from).collect();
        froms.sort_unstable();
        froms.into_iter().find_map(|from| graph.source_behind(from, &mut BTreeSet::new()))
    });
    let Some(source) = held else {
        return Ok(if graph.default_waits() { Me::Wait } else { Me::Default });
    };
    let node = &graph.nodes[&source];
    let loopback = graph.loopback_behind(source, &mut BTreeSet::new());
    if !loopback && Some(&node.name) == graph.default_source.as_ref() {
        return Ok(Me::Default);
    }
    Ok(Me::Hold(Microphone {
        name: node.name.clone(),
        description: node.named(),
        passive: loopback.then(|| passive_value(graph.version.as_deref())),
    }))
}

/// The same, when the graph does not read: only the default input's name is left, if that. A
/// headset's is never opened actively, and a default not named may be one.
pub fn me_unread(default: Option<&str>) -> Me {
    match default {
        Some(name) if !name.starts_with("bluez_") => Me::Default,
        _ => Me::Wait,
    }
}

/// Whether the default input must not be opened, for the first form, which follows nothing. A
/// graph that does not read is said so.
pub fn default_waits(dump: &str) -> Result<bool, String> {
    Ok(Graph::read(dump)?.default_waits())
}

/// What to do of the stream that is `on`, `alive` or not, when the graph wants `want`.
pub fn step(on: &Me, alive: bool, want: &Me) -> Step {
    match want {
        Me::Wait if *on == Me::Wait => Step::Keep,
        Me::Wait => Step::Let,
        _ if on == want && alive => Step::Keep,
        _ => Step::Take,
    }
}

/// The text with each Bluetooth address it holds replaced: a source's name holds one, so may a
/// description the system made from it, and neither a log nor a session ever does.
pub fn without_address(text: &str) -> String {
    let bytes = text.as_bytes();
    let pair = |at: usize| bytes.len() >= at + 2 && bytes[at].is_ascii_hexdigit() && bytes[at + 1].is_ascii_hexdigit();
    let address = |at: usize| {
        (0..6).all(|n| pair(at + 3 * n))
            // A space: as WirePlumber's `create-loopback-node.lua` makes a description of one.
            && (0..5).all(|n| matches!(bytes[at + 3 * n + 2], b':' | b'-' | b'_' | b' '))
            && !bytes.get(at + 17).is_some_and(u8::is_ascii_hexdigit)
    };
    let (mut kept, mut from, mut at) = (String::new(), 0, 0);
    while at < bytes.len() {
        if text.is_char_boundary(at) && address(at) {
            kept.push_str(&text[from..at]);
            kept.push_str("unknown");
            at += 17;
            from = at;
        } else {
            at += 1;
        }
    }
    kept.push_str(&text[from..]);
    kept
}

/// The samples of what a stream delivered as 32-bit floats, whole frames only: a track refuses
/// anything else, and which channel a sample belongs to is never guessed.
pub fn whole_frames(bytes: &[u8], channels: u16) -> Vec<f32> {
    let frame = usize::from(channels) * 4;
    if frame == 0 {
        return Vec::new();
    }
    let (samples, _) = bytes[..bytes.len() - bytes.len() % frame].as_chunks::<4>();
    samples.iter().map(|sample| f32::from_le_bytes(*sample)).collect()
}

struct Node {
    class: String,
    name: String,
    description: Option<String>,
    process: Option<u64>,
    records_an_output: bool,
    monitor: bool,
    loopback: bool,
    link_group: Option<String>,
    own: bool,
    running: bool,
}

impl Node {
    /// Named as cpal names a device; never with a Bluetooth address.
    fn named(&self) -> String {
        without_address(self.description.as_deref().unwrap_or("unknown"))
    }
}

struct Graph {
    nodes: HashMap<u64, Node>,
    links: Vec<(u64, u64)>,
    default_sink: Option<String>,
    default_source: Option<String>,
    /// PipeWire's own, as its `Core` says.
    version: Option<String>,
}

impl Graph {
    /// Whether opening the default input could wake a headset's, read as WirePlumber's script reads
    /// it, through the filters. A name with no node is judged by the name; no name at all, by
    /// whether there is a headset's input for the system to choose.
    fn default_waits(&self) -> bool {
        let Some(name) = &self.default_source else {
            return self.nodes.values().any(|node| node.loopback);
        };
        match self.nodes.iter().find(|(_, node)| node.name == *name) {
            Some((id, _)) => self.loopback_behind(*id, &mut BTreeSet::new()),
            None => name.starts_with("bluez_"),
        }
    }

    /// The other nodes of a filter's group, each with what feeds it.
    fn behind(&self, id: u64, group: &str) -> Vec<u64> {
        let mut froms: Vec<u64> = self
            .nodes
            .iter()
            .filter(|(other, node)| **other != id && node.link_group.as_deref() == Some(group) && !node.monitor)
            .flat_map(|(other, _)| self.links.iter().filter(move |(_, to)| to == other).map(|(from, _)| *from))
            .collect();
        froms.sort_unstable();
        froms
    }

    /// The source a call linked to `id` listens to: `id` itself, or, through a filter that offers
    /// no source of its own, the one behind it. As WirePlumber's script walks it, each group once.
    fn source_behind(&self, id: u64, seen: &mut BTreeSet<String>) -> Option<u64> {
        let node = self.nodes.get(&id)?;
        if node.class.starts_with("Audio/Source") || node.class == "Audio/Duplex" {
            return Some(id);
        }
        let group = node.link_group.as_ref()?;
        if !seen.insert(group.clone()) {
            return None;
        }
        self.behind(id, group).into_iter().find_map(|from| self.source_behind(from, seen))
    }

    /// Whether `id` is a headset's virtual input, or a filter fed by one.
    fn loopback_behind(&self, id: u64, seen: &mut BTreeSet<String>) -> bool {
        let Some(node) = self.nodes.get(&id) else { return false };
        if node.loopback {
            return true;
        }
        let Some(group) = &node.link_group else { return false };
        seen.insert(group.clone()) && self.behind(id, group).into_iter().any(|from| self.loopback_behind(from, seen))
    }

    fn read(dump: &str) -> Result<Graph, String> {
        let objects: Vec<Value> = serde_json::from_str(dump).map_err(|error| format!("the graph does not read ({error})"))?;
        let mut graph = Graph {
            nodes: HashMap::new(),
            links: Vec::new(),
            default_sink: None,
            default_source: None,
            version: None,
        };
        // A native client names its process on the client alone (pw-loopback, cpal's streams).
        let number = |value: &Value| value.as_u64().or_else(|| value.as_str()?.parse().ok());
        let binaries: HashMap<u64, &str> = objects
            .iter()
            .filter(|object| object["type"] == "PipeWire:Interface:Client")
            .filter_map(|object| {
                Some((
                    object["id"].as_u64()?,
                    object["info"]["props"]["application.process.binary"].as_str()?,
                ))
            })
            .collect();
        let clients: HashMap<u64, u64> = objects
            .iter()
            .filter(|object| object["type"] == "PipeWire:Interface:Client")
            .filter_map(|object| {
                let props = &object["info"]["props"];
                // Never `pipewire.sec.pid`: for a client of PulseAudio's it is `pipewire-pulse`'s, one for all.
                Some((object["id"].as_u64()?, number(&props["application.process.id"])?))
            })
            .collect();
        for object in &objects {
            let id = object["id"].as_u64();
            match (object["type"].as_str(), id) {
                (Some("PipeWire:Interface:Node"), Some(id)) => {
                    let props = &object["info"]["props"];
                    let text = |key: &str| props[key].as_str().map(str::to_string);
                    let flag = |key: &str| matches!(&props[key], Value::Bool(true)) || props[key].as_str() == Some("true");
                    let binary = props["application.process.binary"]
                        .as_str()
                        .or_else(|| binaries.get(&number(&props["client.id"])?).copied());
                    graph.nodes.insert(
                        id,
                        Node {
                            class: text("media.class").unwrap_or_default(),
                            name: text("node.name").unwrap_or_default(),
                            description: text("node.description"),
                            // A number in `pw-dump`; a text in some versions of the properties.
                            // A number in `pw-dump`, a text in some versions; else its client's.
                            process: number(&props["application.process.id"])
                                .or_else(|| clients.get(&number(&props["client.id"])?).copied()),
                            records_an_output: flag("stream.capture.sink"),
                            monitor: flag("stream.monitor"),
                            loopback: flag("bluez5.loopback"),
                            link_group: text("node.link-group"),
                            own: binary.is_some_and(|binary| OWN.contains(&binary)),
                            running: object["info"]["state"] == "running",
                        },
                    );
                }
                (Some("PipeWire:Interface:Core"), _) => {
                    graph.version = object["info"]["version"].as_str().map(str::to_string);
                }
                (Some("PipeWire:Interface:Link"), _) => {
                    let ends = (object["info"]["output-node-id"].as_u64(), object["info"]["input-node-id"].as_u64());
                    if let (Some(from), Some(to)) = ends {
                        graph.links.push((from, to));
                    }
                }
                (Some("PipeWire:Interface:Metadata"), _) if object["props"]["metadata.name"] == "default" => {
                    let entries = object["metadata"].as_array().into_iter().flatten();
                    let default = |key: &str| {
                        let entry = entries.clone().find(|entry| entry["subject"] == 0 && entry["key"] == key)?;
                        entry["value"]["name"].as_str().map(str::to_string)
                    };
                    graph.default_sink = default("default.audio.sink");
                    graph.default_source = default("default.audio.source");
                }
                _ => {}
            }
        }
        Ok(graph)
    }
}
