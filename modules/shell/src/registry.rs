//! The models Settings offer (D40, ADR-0010): one list, `registry/models.json`, built in as the
//! list shipped. A list that does not read as one is refused whole: in a pull request by the
//! shell's tests, and in Wholeheard by the same reading. Read again, it is taken only signed by
//! the decider's key (D56): the signature is minisign's, its private key outside any repository.

use std::{collections::HashSet, io::Read, time::Duration};

use crate::model::{self, Published};

/// The list in the repository, as this build was made with it.
pub const SHIPPED: &str = include_str!("../registry/models.json");

/// Where the list is read again from, on the user's gesture: the file on `main`.
pub const ADDRESS: &str = "https://raw.githubusercontent.com/NapkinStack/wholeheard/main/modules/shell/registry/models.json";

/// The decider's public key, as `minisign -G` writes it on the second line of its `.pub` file.
/// None until they give it (cycle 05, *Dependency with a date*): no list is read again, Settings
/// say so, and so does the check of the list in the repository.
pub const PUBLIC_KEY: Option<&str> = None;

/// What no list is read again without, said to the logs and to whoever reads the check.
const WAITS: &str = "the list is not read again: it waits on the decider's key";

/// The hosts a model of the list may be fetched from.
pub const HOSTS: [&str; 1] = ["huggingface.co"];

/// The most a list may weigh, read: a few models take a few hundred bytes each.
pub const LIMIT: usize = 64 * 1024;

/// The most a signature may weigh, read: minisign's four lines.
const SIGNATURE_LIMIT: usize = 4 * 1024;

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

/// The host an address names, as the user is shown it.
pub fn host(address: &str) -> &str {
    address.split('/').nth(2).unwrap_or(address)
}

/// Reads the list again from `address` — `ADDRESS`, or a test's — and takes it only signed by
/// `key`, and as a list this build would ship: one that reads, its first model this build's
/// default (ADR-0003), the model a transcript waits for and the tray fetches. With no key, nothing
/// is asked of anyone.
pub fn fetch(address: &str, key: Option<&str>) -> Result<Vec<Listed>, String> {
    let key = key.ok_or(WAITS)?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_response(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        // The host the button names is the one read: a redirect elsewhere is refused.
        .max_redirects(0)
        .build()
        .into();
    let text = answered(&agent, address, LIMIT).map_err(|why| format!("the list {why}"))?;
    let signature =
        answered(&agent, &format!("{address}.minisig"), SIGNATURE_LIMIT).map_err(|why| format!("the list's signature {why}"))?;
    // Nothing of the list is read before it is known to be the decider's.
    verify(&text, &signature, key)?;
    let listed = read(&text)?;
    if listed[0].published != model::DEFAULT {
        return Err("the list's first model is not this build's default".into());
    }
    Ok(listed)
}

/// The text `address` answers, read up to one byte over `limit`.
fn answered(agent: &ureq::Agent, address: &str, limit: usize) -> Result<String, String> {
    let mut answer = agent.get(address).call().map_err(|error| format!("could not be read ({error})"))?;
    let status = answer.status().as_u16();
    if status != 200 {
        return Err(format!("could not be read (HTTP {status})"));
    }
    let mut text = String::new();
    answer
        .body_mut()
        .as_reader()
        .take(limit as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|error| format!("could not be read ({error})"))?;
    Ok(text)
}

/// Whether `signature` is `key`'s over the bytes of `list`. minisign's pre-hashed signature only,
/// what `minisign -S` writes: the legacy form is refused.
pub fn verify(list: &str, signature: &str, key: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::from_base64(key).map_err(|error| format!("the key built in does not read ({error})"))?;
    let signature =
        minisign_verify::Signature::decode(signature).map_err(|error| format!("the list's signature does not read ({error})"))?;
    key.verify(list.as_bytes(), &signature, false)
        .map_err(|error| format!("the list's signature does not verify ({error})"))
}

/// The check of the list in the repository, in every pull request: what it can say of `list`,
/// or why it is refused. With no key built in, nothing can be verified, and it says that.
pub fn in_the_repository(list: &str, signature: Option<&str>, key: Option<&str>) -> Result<String, String> {
    let Some(key) = key else {
        return Ok(format!("signature not verified, {WAITS} (D56)"));
    };
    verify(list, signature.ok_or("the list has no signature")?, key)?;
    Ok("its signature verifies against the key built in".into())
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
