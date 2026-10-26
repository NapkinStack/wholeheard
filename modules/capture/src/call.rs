//! Where a call plays (D41, ADR-0011), read from PipeWire's graph as `pw-dump` prints it: a call is
//! an application, capture excepted, with a stream recording a microphone; where it plays is the
//! outputs its playing streams are linked to. No device is opened here.

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
        description: node.description.clone().unwrap_or_else(|| node.name.clone()),
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
        Some(node.description.clone().unwrap_or_else(|| node.name.clone()))
    };
    [fed(false), fed(true)]
}

struct Node {
    class: String,
    name: String,
    description: Option<String>,
    process: Option<u64>,
    records_an_output: bool,
}

struct Graph {
    nodes: HashMap<u64, Node>,
    links: Vec<(u64, u64)>,
    default_sink: Option<String>,
}

impl Graph {
    fn read(dump: &str) -> Result<Graph, String> {
        let objects: Vec<Value> = serde_json::from_str(dump).map_err(|error| format!("the graph does not read ({error})"))?;
        let mut graph = Graph {
            nodes: HashMap::new(),
            links: Vec::new(),
            default_sink: None,
        };
        // A native client names its process on the client alone (pw-loopback, cpal's streams).
        let number = |value: &Value| value.as_u64().or_else(|| value.as_str()?.parse().ok());
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
                            records_an_output: matches!(&props["stream.capture.sink"], Value::Bool(true))
                                || props["stream.capture.sink"].as_str() == Some("true"),
                        },
                    );
                }
                (Some("PipeWire:Interface:Link"), _) => {
                    let ends = (object["info"]["output-node-id"].as_u64(), object["info"]["input-node-id"].as_u64());
                    if let (Some(from), Some(to)) = ends {
                        graph.links.push((from, to));
                    }
                }
                (Some("PipeWire:Interface:Metadata"), _) if object["props"]["metadata.name"] == "default" => {
                    let entries = object["metadata"].as_array().into_iter().flatten();
                    let sink = entries
                        .into_iter()
                        .find(|entry| entry["subject"] == 0 && entry["key"] == "default.audio.sink");
                    graph.default_sink = sink.and_then(|entry| entry["value"]["name"].as_str().map(str::to_string));
                }
                _ => {}
            }
        }
        Ok(graph)
    }
}
