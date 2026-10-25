//! Who can write the minutes: the models Ollama holds on this computer, and the online services
//! offered with their address. Each is an address `wholeheard-summary` speaks to — OpenAI's way of
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

/// The local model recommended for the minutes: tried on real meetings, it holds `minutes/v1`'s
/// form, where Mistral 7B and models for code do not (#261). Offered first when Ollama holds it.
pub const RECOMMENDED: &str = "qwen3:8b";

/// The address `wholeheard-summary` is given for a model of Ollama at `base`.
pub fn ollama_address(base: &str) -> String {
    format!("{}/v1", base.trim_end_matches('/'))
}

/// The models Ollama at `base` holds, by name — none when it does not answer quickly: Ollama not
/// running is no fault, only no model on this computer.
pub fn ollama_models(base: &str) -> Vec<String> {
    ollama_listed(base).unwrap_or_default()
}

/// The same, or none when Ollama does not answer: Ollama there with no model is not Ollama
/// missing (D37).
pub fn ollama_listed(base: &str) -> Option<Vec<String>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .build()
        .into();
    let answer = agent.get(format!("{}/api/tags", base.trim_end_matches('/'))).call();
    let mut answer = answer.ok()?;
    let text = answer.body_mut().read_to_string().ok()?;
    let Ok(tags) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Some(Vec::new());
    };
    let mut names: Vec<String> = tags
        .get("models")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| model.get("name").and_then(serde_json::Value::as_str).map(str::to_string))
        // A name `wholeheard-summary` would refuse is not offered.
        .filter(|name| !name.trim().is_empty() && !name.chars().any(char::is_control))
        .collect();
    // The recommended first, the others by name.
    names.sort_by(|a, b| (a != RECOMMENDED).cmp(&(b != RECOMMENDED)).then(a.cmp(b)));
    names.dedup();
    Some(names)
}

/// Asks Ollama at `base` to fetch `model` — its own pull, which it takes up where a cut left it —
/// and tells `progress` the bytes it holds of the layer it fetches, of how many. Whole only when
/// Ollama says `success`; a cut, an error it names, or Ollama stopping is said (D37).
pub fn ollama_pull(base: &str, model: &str, progress: &mut dyn FnMut(u64, u64)) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(5)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .build()
        .into();
    let asked = serde_json::json!({ "model": model, "stream": true }).to_string();
    let mut answer = agent
        .post(format!("{}/api/pull", base.trim_end_matches('/')))
        .header("content-type", "application/json")
        .send(asked.as_str())
        .map_err(|error| format!("Ollama could not be asked ({error})"))?;
    if answer.status() != 200 {
        return Err(format!("Ollama refused the pull (HTTP {})", answer.status().as_u16()));
    }
    let reader = std::io::BufReader::new(answer.body_mut().as_reader());
    for line in std::io::BufRead::lines(reader) {
        let line = line.map_err(|error| format!("the pull was cut ({error})"))?;
        let Ok(said) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(error) = said.get("error").and_then(serde_json::Value::as_str) {
            return Err(format!("Ollama said: {error}"));
        }
        if said.get("status").and_then(serde_json::Value::as_str) == Some("success") {
            return Ok(());
        }
        let count = |key: &str| said.get(key).and_then(serde_json::Value::as_u64);
        if let (Some(total), Some(completed)) = (count("total"), count("completed")) {
            progress(completed, total);
        }
    }
    Err("the pull ended before Ollama said it was whole".into())
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

/// Whether a provider's address is this machine — what is sent there never leaves it. Read as
/// `wholeheard-summary` reads it (its provider's reading of an address):
/// the same parser, the same rules, so that an address it refuses, or sends elsewhere, is never
/// judged local here — the agreement is never skipped on a reading of the shell's own.
pub fn local(address: &str) -> bool {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    if address.contains(['?', '#', '@', '\\']) || address.chars().any(char::is_whitespace) {
        return false;
    }
    let joined = format!("{}/chat/completions", address.trim_end_matches('/'));
    let Ok(typed) = joined.parse::<http::Uri>() else { return false };
    let (Some("http" | "https"), Some(authority), Some(host)) = (typed.scheme_str(), typed.authority(), typed.host()) else {
        return false;
    };
    // What follows the host is a port, or nothing.
    let port = authority.as_str().strip_prefix(host).unwrap_or_default();
    if !port.is_empty()
        && !port
            .strip_prefix(':')
            .is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_digit()) && digits.parse::<u16>().is_ok())
    {
        return false;
    }
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let here = |address: IpAddr| address.to_canonical().is_loopback() || address.to_canonical().is_unspecified();
    // Brackets hold an IPv6 address and nothing else; four numbers in full, an IPv4 one; a name
    // otherwise — and a name, `localhost` apart, is never this machine.
    if let Some(inside) = host.strip_prefix('[') {
        return inside
            .strip_suffix(']')
            .and_then(|inside| inside.parse::<Ipv6Addr>().ok())
            .is_some_and(|ip| here(IpAddr::V6(ip)));
    }
    host.parse::<Ipv4Addr>().is_ok_and(|ip| here(IpAddr::V4(ip)))
}
