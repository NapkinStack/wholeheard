//! Who can write the minutes: the models Ollama holds on this computer, and the online services
//! offered with their address. Each is an address `summit-summary` speaks to — OpenAI's way of
//! asking, which Ollama speaks too.

use std::time::Duration;

/// Ollama on this computer, where it listens unless told otherwise.
pub const OLLAMA: &str = "http://localhost:11434";

/// An online service offered with its address filled in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Online {
    pub name: &'static str,
    pub address: &'static str,
}

/// The online services offered; any other is the user's own address.
pub const ONLINE: [Online; 3] = [
    Online {
        name: "OpenRouter",
        address: "https://openrouter.ai/api/v1",
    },
    Online {
        name: "OpenAI",
        address: "https://api.openai.com/v1",
    },
    Online {
        name: "Mistral",
        address: "https://api.mistral.ai/v1",
    },
];

/// The address `summit-summary` is given for a model of Ollama at `base`.
pub fn ollama_address(base: &str) -> String {
    format!("{}/v1", base.trim_end_matches('/'))
}

/// The models Ollama at `base` holds, by name — none when it does not answer quickly: Ollama not
/// running is no fault, only no model on this computer.
pub fn ollama_models(base: &str) -> Vec<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .into();
    let answer = agent.get(format!("{}/api/tags", base.trim_end_matches('/'))).call();
    let Ok(mut answer) = answer else { return Vec::new() };
    let Ok(text) = answer.body_mut().read_to_string() else {
        return Vec::new();
    };
    let Ok(tags) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let mut names: Vec<String> = tags
        .get("models")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| model.get("name").and_then(serde_json::Value::as_str).map(str::to_string))
        // A name `summit-summary` would refuse is not offered.
        .filter(|name| !name.trim().is_empty() && !name.chars().any(char::is_control))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The host a provider's address names: what its key is filed under — the same for the same
/// server, its scheme in any case, its own port written or not. None for an address that is not one.
pub fn host(address: &str) -> Option<String> {
    if address.chars().any(char::is_whitespace) {
        return None;
    }
    let (scheme, rest) = address.split_once("://")?;
    let own_port = match scheme.to_ascii_lowercase().as_str() {
        "https" => ":443",
        "http" => ":80",
        _ => return None,
    };
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?.to_ascii_lowercase();
    let host = host.strip_suffix(own_port).map_or(host.clone(), str::to_string);
    (!host.is_empty()).then_some(host)
}
