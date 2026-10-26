//! The models Settings offer (D40, ADR-0010): one list, `registry/models.json`, built in as the
//! list shipped. A list that does not read as one is refused whole: in a pull request by the
//! shell's tests, and in Wholeheard by the same reading.

use std::collections::HashSet;

use crate::model::{self, Published};

/// The list in the repository, as this build was made with it.
pub const SHIPPED: &str = include_str!("../registry/models.json");

/// The hosts a model of the list may be fetched from.
pub const HOSTS: [&str; 1] = ["huggingface.co"];

/// The most a list may weigh, read: a few models take a few hundred bytes each.
pub const LIMIT: usize = 64 * 1024;

/// What a model is good for, said by a sentence of `words.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Good {
    Recommended,
    Faster,
}

/// A model of the list: as its publisher gives it, and what it is good for — none when the list
/// names a use this build has no sentence for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub published: Published,
    pub good: Option<Good>,
}

/// The list shipped. Its tests read the same file: a build cannot ship one that does not read.
pub fn shipped() -> Vec<Listed> {
    read(SHIPPED).expect("the list shipped reads: the shell's tests read it")
}

/// Reads a list of models — version 1 — or says why it is not one. Fields it does not know are
/// read past; a model from a host not allowed, of no size, or without a SHA-256 refuses it whole.
pub fn read(text: &str) -> Result<Vec<Listed>, String> {
    if text.len() > LIMIT {
        return Err(format!("the list is larger than {} KB", LIMIT / 1024));
    }
    let list: serde_json::Value = serde_json::from_str(text).map_err(|error| format!("the list does not read ({error})"))?;
    match list.get("version").and_then(serde_json::Value::as_u64) {
        Some(1) => {}
        Some(other) => return Err(format!("the list is of version {other}, unknown to this build")),
        None => return Err("the list says no version this build reads".into()),
    }
    let models = list
        .get("whisper")
        .and_then(serde_json::Value::as_array)
        .filter(|models| !models.is_empty())
        .ok_or("the list holds no model")?;
    let mut files = HashSet::new();
    models
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let listed = listed(entry).map_err(|why| format!("model {}: {why}", index + 1))?;
            if !files.insert(model::file(&listed.published)) {
                return Err(format!("model {}: its file is another model's", index + 1));
            }
            Ok(listed)
        })
        .collect()
}

fn listed(entry: &serde_json::Value) -> Result<Listed, String> {
    let text = |name: &str| entry.get(name).and_then(serde_json::Value::as_str);
    let address = text("address").ok_or("no address")?;
    let rest = address.strip_prefix("https://").ok_or("its address is not HTTPS")?;
    let (host, path) = rest.split_once('/').ok_or("its address names no file")?;
    if !HOSTS.contains(&host) {
        return Err(format!("{host} is not a host models are fetched from"));
    }
    // The address's last part is the file it is kept as, in the models' folder: a name, never
    // a folder, a query or a fragment.
    let file = path.rsplit('/').next().unwrap_or_default();
    let named = file
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if !named || file.is_empty() || file.starts_with('.') || path.contains(['?', '#']) {
        return Err("its address names no file".into());
    }
    let bytes = entry.get("bytes").and_then(serde_json::Value::as_u64).filter(|bytes| *bytes > 0);
    let bytes = bytes.ok_or("no size")?;
    let sha256 = text("sha256").filter(|sha256| sha256.len() == 64 && sha256.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')));
    let sha256 = sha256.ok_or("no SHA-256 of 64 lower-case hexadecimal characters")?;
    let good = match text("good") {
        Some("recommended") => Some(Good::Recommended),
        Some("faster") => Some(Good::Faster),
        _ => None,
    };
    // Read once at the start, and again on the user's gesture: a few strings kept for the run.
    let kept = |text: &str| -> &'static str { Box::leak(text.to_owned().into_boxed_str()) };
    let host = HOSTS.iter().find(|allowed| **allowed == host).copied().unwrap_or_default();
    Ok(Listed {
        published: Published {
            address: kept(address),
            host,
            bytes,
            sha256: kept(sha256),
        },
        good,
    })
}
