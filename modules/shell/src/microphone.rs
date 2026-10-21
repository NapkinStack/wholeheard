//! Which applications take a microphone (D30), read from PipeWire's graph as `pw-dump --monitor`
//! prints it: the whole graph first, then each change, an object gone being its id with no info.
//!
//! An application takes a microphone when one of its streams that records is linked to a device
//! that is no output: a stream reading what the computer plays — a sink's monitor —, a level
//! meter, and Wholeheard's own capture take none. An application is known by its process, so that
//! a browser's tabs and its many streams are one; it is named as it names itself. Nothing of this
//! is kept beyond what the graph holds now.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::BufReader,
    process::{Command, Stdio},
    time::Duration,
};

use serde_json::Value;

/// Watches PipeWire's graph for as long as Wholeheard runs, and tells each change, on a thread of
/// its own. `pw-dump` missing, or ended, is started again a minute later; what it said before
/// stands, and an application still holding a microphone is not said again.
pub fn watch(tell: impl Fn(Change) + Send + 'static) {
    std::thread::spawn(move || {
        let mut takers = Takers::default();
        loop {
            let started = Command::new("pw-dump")
                .args(["--monitor", "--no-colors"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            match started {
                Ok(mut dump) => {
                    takers.anew();
                    if let Some(out) = dump.stdout.take() {
                        let batches = serde_json::Deserializer::from_reader(BufReader::new(out)).into_iter::<Value>();
                        for batch in batches.map_while(Result::ok) {
                            takers.read(&batch).into_iter().for_each(&tell);
                        }
                    }
                    let _ = dump.kill();
                    let _ = dump.wait();
                    tracing::warn!("shell.microphones_unwatched");
                }
                Err(error) => tracing::warn!(detail = %error, "shell.microphones_unwatched"),
            }
            std::thread::sleep(Duration::from_secs(60));
        }
    });
}

/// An application starting or ceasing to take a microphone, known by `key`: its process, or its
/// client when it says no process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Took { key: u64, name: String },
    Let { key: u64 },
}

/// Which application is offered to be recorded: once while it holds a microphone, never while
/// Wholeheard records, never with offers off. "Not this time" lasts until it lets go.
#[derive(Default)]
pub struct Offers {
    offered: HashSet<u64>,
}

impl Offers {
    /// What a change calls for: the name of an application to offer, or nothing.
    pub fn on(&mut self, change: &Change, recording: bool, offering: bool) -> Option<String> {
        match change {
            // Seen while it could not be offered, it is not offered for this hold.
            Change::Took { key, name } => (self.offered.insert(*key) && offering && !recording).then(|| name.clone()),
            Change::Let { key } => {
                self.offered.remove(key);
                None
            }
        }
    }
}

/// The graph as far as these questions need it.
#[derive(Default)]
pub struct Takers {
    /// Each node's props, by id — in their order, so that the oldest stream of an application
    /// names it, whatever the order the graph is read in.
    nodes: BTreeMap<u64, Value>,
    /// Each client's props, by id.
    clients: HashMap<u64, Value>,
    /// Each link: the node it comes from, the node it goes to.
    links: HashMap<u64, (u64, u64)>,
    /// Each recording stream's application, as its client said it while it was there: a client
    /// read gone before its streams leaves them the application they were.
    owners: HashMap<u64, Owner>,
    /// The applications taking a microphone, as last said.
    taking: BTreeMap<u64, String>,
}

/// The application a stream belongs to, and whether it is Wholeheard's own.
struct Owner {
    key: u64,
    name: String,
    own: bool,
}

/// Programs whose streams are Wholeheard's own.
const OWN: [&str; 2] = ["wholeheard-capture", "wholeheard"];

impl Takers {
    /// The graph forgotten, to be read whole again; who takes a microphone is kept, so that the
    /// first batch says only what changed meanwhile.
    pub fn anew(&mut self) {
        self.nodes.clear();
        self.clients.clear();
        self.links.clear();
    }

    /// Reads one batch of the graph, and says who started or ceased to take a microphone.
    pub fn read(&mut self, batch: &Value) -> Vec<Change> {
        for object in batch.as_array().into_iter().flatten() {
            let Some(id) = object["id"].as_u64() else { continue };
            let info = &object["info"];
            if info.is_null() {
                self.nodes.remove(&id);
                self.owners.remove(&id);
                self.clients.remove(&id);
                self.links.remove(&id);
                continue;
            }
            match object["type"].as_str().unwrap_or_default().rsplit(':').next() {
                Some("Node") => {
                    // A change may carry the props again, or only a state: the props are kept.
                    if info["props"].is_object() {
                        self.nodes.insert(id, info["props"].clone());
                    }
                }
                Some("Client") if info["props"].is_object() => {
                    self.clients.insert(id, info["props"].clone());
                }
                Some("Link") => {
                    if let (Some(from), Some(to)) = (info["output-node-id"].as_u64(), info["input-node-id"].as_u64()) {
                        self.links.insert(id, (from, to));
                    }
                }
                _ => {}
            }
        }
        // Each recording stream's application, while its client is there to say it.
        for (id, props) in &self.nodes {
            if props["media.class"].as_str() != Some("Stream/Input/Audio") {
                continue;
            }
            let client = props["client.id"].as_u64();
            if client.is_none_or(|client| self.clients.contains_key(&client)) || !self.owners.contains_key(id) {
                self.owners.insert(*id, self.owner(*id, props));
            }
        }
        let now = self.now();
        let mut changes: Vec<Change> = self
            .taking
            .keys()
            .filter(|key| !now.contains_key(key))
            .map(|&key| Change::Let { key })
            .collect();
        changes.extend(
            now.iter()
                .filter(|(key, _)| !self.taking.contains_key(key))
                .map(|(&key, name)| Change::Took { key, name: name.clone() }),
        );
        self.taking = now;
        changes
    }

    /// A stream's application, as its client says it: by its process, named as it names itself.
    fn owner(&self, id: u64, props: &Value) -> Owner {
        let text = |props: &Value, key: &str| props[key].as_str().filter(|text| !text.is_empty()).map(str::to_string);
        let client_id = props["client.id"].as_u64();
        let client = client_id.and_then(|id| self.clients.get(&id)).cloned().unwrap_or(Value::Null);
        let binary = text(&client, "application.process.binary");
        let own = binary.as_deref().is_some_and(|binary| OWN.contains(&binary));
        let key = match (client["application.process.id"].as_u64(), client_id) {
            (Some(process), _) => process,
            (None, Some(client)) => 1 << 40 | client,
            (None, None) => 1 << 41 | id,
        };
        let name = text(&client, "application.name")
            .or_else(|| text(props, "application.name"))
            .or(binary)
            .unwrap_or_else(|| "?".into());
        Owner { key, name, own }
    }

    /// The applications taking a microphone in the graph as it is.
    fn now(&self) -> BTreeMap<u64, String> {
        let flag = |props: &Value, key: &str| matches!(&props[key], Value::Bool(true)) || props[key].as_str() == Some("true");
        // The streams a device that is no output feeds.
        let fed: HashSet<u64> = self
            .links
            .values()
            .filter(|(from, _)| {
                self.nodes
                    .get(from)
                    .and_then(|props| props["media.class"].as_str())
                    .is_some_and(|class| class.starts_with("Audio/Source") || class == "Audio/Duplex")
            })
            .map(|&(_, to)| to)
            .collect();
        let mut taking = BTreeMap::new();
        for (id, props) in &self.nodes {
            if props["media.class"].as_str() != Some("Stream/Input/Audio")
                || !fed.contains(id)
                || flag(props, "stream.capture.sink")
                || flag(props, "stream.monitor")
            {
                continue;
            }
            let Some(owner) = self.owners.get(id) else { continue };
            if owner.own {
                continue;
            }
            let (key, name) = (owner.key, owner.name.clone());
            taking.entry(key).or_insert(name);
        }
        taking
    }
}
