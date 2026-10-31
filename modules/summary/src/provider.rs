//! The one connection `summary` opens: to the address the user configured, speaking the
//! OpenAI protocol — a model on the machine or an online one, the module does not care.
//!
//! The host named is the host contacted: the address is read by the HTTP client's own
//! reader, a redirection is refused, and no proxy the environment names is used
//! (rules/security.md, 1).

use std::{net::IpAddr, time::Duration};

use ureq::http::Uri;

use crate::Refused;

/// How long reaching the provider may take, before any answer is awaited.
const CONNECT: Duration = Duration::from_secs(15);

/// Ollama's window grows by this, and an answer may take this much of it, its thinking counted:
/// a model that runs away is cut, and said to be, rather than waited for.
const WINDOW: usize = 4096;

/// How much of a window a request needs: Ollama's OpenAI route keeps 4096 tokens unless told at
/// its server, and drops the rest in silence, the rules first (the request is cut from its head).
/// Two bytes to a token at the very least; French measured 2.7 characters.
pub fn window(request: &str) -> usize {
    (request.len() / 2 + WINDOW).div_ceil(WINDOW) * WINDOW
}

/// The patience Ollama is given: what any provider gets, and a second for each 20 bytes it has to
/// read, a laptop's processor taking some 14 tokens a second (measured on `qwen3:8b`, 2026-10-31).
pub fn patience(given: Duration, request: &str) -> Duration {
    given + Duration::from_secs((request.len() / 20) as u64)
}

fn refused<T>(code: &'static str, why: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(why.into(), Some(code)))
}

/// Where the minutes are asked for.
pub struct Provider {
    uri: Uri,
    model: String,
    key: Option<String>,
    patience: Duration,
}

/// Never the key: a value of this type is printed by whoever debugs.
impl std::fmt::Debug for Provider {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            out,
            "Provider({}, model {:?}, key {})",
            self.uri,
            self.model,
            if self.key.is_some() { "given" } else { "none" }
        )
    }
}

/// The address of this machine a name stands for, or the reason the host is refused: the
/// minutes name a host, and it must be the one the request goes to.
fn checked(host: &str) -> Result<Option<IpAddr>, Refused> {
    // Brackets hold an IPv6 address and nothing else: around anything else, the system reads a name.
    if let Some(inside) = host.strip_prefix('[') {
        return match inside.strip_suffix(']').map(str::parse::<std::net::Ipv6Addr>) {
            Some(Ok(address)) => Ok(Some(IpAddr::V6(address))),
            _ => refused("settings.invalid", "the provider's host is in brackets and is not an IPv6 address"),
        };
    }
    if let Ok(address) = host.parse::<std::net::Ipv4Addr>() {
        return Ok(Some(IpAddr::V4(address)));
    }
    let name = host.to_ascii_lowercase();
    let label = |label: &str| !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    // A name, not something a system may read as a number in another base, or resolve its own way.
    if !name.split('.').all(label)
        || name
            .rsplit('.')
            .next()
            .is_some_and(|last| last.starts_with(|c: char| c.is_ascii_digit()))
    {
        return refused(
            "settings.invalid",
            "the provider's host is neither a name nor an address written in full",
        );
    }
    if name == "local" {
        return refused(
            "settings.invalid",
            "a host named local cannot be told from this machine in the minutes: give its full name or its address",
        );
    }
    Ok(None)
}

impl Provider {
    /// `address` goes up to and without `/chat/completions`, e.g. `http://localhost:11434/v1`.
    /// A key, when there is one, is sent as a bearer token and written nowhere.
    pub fn new(address: &str, model: &str, key: Option<&str>, patience: Duration) -> Result<Self, Refused> {
        if address.contains(['?', '#', '@']) || address.chars().any(char::is_whitespace) {
            return refused(
                "settings.invalid",
                "the provider's address holds more than an address: no query, no fragment, no user or key in it",
            );
        }
        let joined = format!("{}/chat/completions", address.trim_end_matches('/'));
        let Ok(typed) = joined.parse::<Uri>() else {
            return refused("settings.invalid", "the provider's address is not an address");
        };
        let (Some(scheme @ ("http" | "https")), Some(authority), Some(host)) = (typed.scheme_str(), typed.authority(), typed.host()) else {
            return refused(
                "settings.invalid",
                "the provider's address must start with http:// or https:// and name a host",
            );
        };
        // What follows the host is a port, or nothing: the client's reader lets a wrong one pass as none.
        let port = match authority.as_str().strip_prefix(host).unwrap_or_default() {
            "" => String::new(),
            port if port
                .strip_prefix(':')
                .is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_digit()) && digits.parse::<u16>().is_ok()) =>
            {
                port.into()
            }
            _ => return refused("settings.invalid", "the provider's address holds a port that is not one"),
        };
        // "localhost" is this machine by definition: the request goes to its address, so that
        // no resolver decides otherwise after the minutes have said "local".
        let host = if host.eq_ignore_ascii_case("localhost") {
            "127.0.0.1"
        } else {
            host
        };
        let Ok(uri) = format!("{scheme}://{host}{port}{}", typed.path()).parse::<Uri>() else {
            return refused("settings.invalid", "the provider's address is not an address");
        };
        checked(uri.host().unwrap_or_default())?;
        if model.trim().is_empty() || model.chars().any(char::is_control) {
            return refused(
                "settings.invalid",
                "the model's name is empty or holds a character that is not text",
            );
        }
        let key = key.filter(|key| !key.is_empty());
        // No space either: a server drops the ones around a key, and what it then sends back
        // is no longer the text this program would recognise.
        if key.is_some_and(|key| !key.chars().all(|c| c.is_ascii_graphic())) {
            return refused("settings.invalid", "the key holds a space or a character a request cannot carry");
        }
        Ok(Self {
            uri,
            model: model.into(),
            key: key.map(Into::into),
            patience,
        })
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// The host as the minutes name it: `local` for this machine, the host's name otherwise —
    /// read from the same address the request is sent to.
    pub fn host(&self) -> String {
        let host = self.uri.host().unwrap_or_default();
        match checked(host) {
            // An address of this machine: loopback, the one that means "any", or either written inside an IPv6 one.
            Ok(Some(address)) if address.to_canonical().is_loopback() || address.to_canonical().is_unspecified() => "local".into(),
            Ok(Some(address)) => address.to_string(),
            _ => host.to_ascii_lowercase(),
        }
    }

    /// Who answers, as a sentence names it: "the provider on this computer", or at its host.
    pub fn named(&self) -> String {
        match self.host().as_str() {
            "local" => "the provider on this computer".into(),
            host => format!("the provider at {host}"),
        }
    }

    /// Where Ollama takes a request with its window, when it is Ollama that listens at this
    /// address: an address of OpenAI's form, no key, and its own answer to its own question.
    /// Whatever else answers, the address is asked as it was given.
    fn ollama(&self, agent: &ureq::Agent) -> Option<Uri> {
        let root = self.uri.to_string();
        let root = root.strip_suffix("/v1/chat/completions").filter(|_| self.key.is_none())?;
        let mut response = agent
            .get(format!("{root}/api/version"))
            .config()
            .timeout_global(Some(CONNECT.min(self.patience)))
            .build()
            .call()
            .ok()?;
        let said = response.body_mut().read_json::<serde_json::Value>().ok()?;
        (response.status() == 200 && said["version"].is_string())
            .then(|| format!("{root}/api/chat").parse().ok())
            .flatten()
    }

    /// Asks once, and gives the answer's text — or what failed, in one sentence.
    pub fn ask(&self, request: &str) -> Result<String, Refused> {
        let host = self.named();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .proxy(None)
            .http_status_as_error(false)
            // A host that answers nothing at all is not reached, rather than slow (summarising/v1).
            .timeout_connect(Some(CONNECT.min(self.patience)))
            .build()
            .into();
        let ollama = self.ollama(&agent);
        let waited = if ollama.is_some() {
            patience(self.patience, request)
        } else {
            self.patience
        };
        let mut call = agent
            .post(ollama.as_ref().unwrap_or(&self.uri))
            .config()
            .timeout_global(Some(waited))
            .build();
        if let Some(key) = &self.key {
            call = call.header("Authorization", format!("Bearer {key}"));
        }
        let mut body = serde_json::json!({ "model": self.model, "stream": false, "messages": [{ "role": "user", "content": request }] });
        if ollama.is_some() {
            body["options"] = serde_json::json!({ "num_ctx": window(request), "num_predict": WINDOW });
        }
        let mut response = match call.send_json(&body) {
            Ok(response) => response,
            Err(ureq::Error::Timeout(ureq::Timeout::Resolve | ureq::Timeout::Connect)) => {
                return refused(
                    "provider.unreachable",
                    format!("{host} could not be reached within {} s", CONNECT.as_secs()),
                );
            }
            Err(ureq::Error::Timeout(_)) => {
                return refused("provider.too_slow", format!("{host} did not answer within {} s", waited.as_secs()));
            }
            Err(error) => return refused("provider.unreachable", format!("{host} could not be reached: {error}")),
        };
        match response.status().as_u16() {
            200 => {}
            status @ (401 | 403) => return refused("provider.refused_key", format!("{host} refused the key (HTTP {status})")),
            402 => {
                return refused(
                    "provider.no_credit",
                    format!("{host} refused for lack of credit (HTTP 402): add credit, or raise the key's limit, with this provider"),
                );
            }
            404 => {
                return refused(
                    "provider.unknown_model",
                    format!("{host} does not know this model or this address (HTTP 404)"),
                );
            }
            status @ (300 | 301 | 302 | 303 | 307 | 308) => {
                return refused(
                    "provider.redirected",
                    format!("{host} sent the request elsewhere, and it was not followed (HTTP {status})"),
                );
            }
            status => return refused("provider.failed", format!("{host} failed (HTTP {status})")),
        }
        let Ok(answer) = response.body_mut().read_json::<serde_json::Value>() else {
            return refused(
                "provider.failed",
                format!("{host} sent an answer that could not be read to its end"),
            );
        };
        let (reason, said) = if ollama.is_some() {
            (&answer["done_reason"], &answer["message"]["content"])
        } else {
            let choice = &answer["choices"][0];
            (&choice["finish_reason"], &choice["message"]["content"])
        };
        // An answer is whole when the provider says it stopped by itself, or says nothing.
        match reason {
            serde_json::Value::Null => {}
            reason if reason == "stop" => {}
            reason if reason == "length" => {
                return refused(
                    "answer.cut_short",
                    format!("{host} cut its answer short: the minutes would be incomplete"),
                );
            }
            _ => return refused("provider.failed", format!("{host} stopped before the end of its answer")),
        }
        match said.as_str() {
            Some(text) if !text.trim().is_empty() => Ok(text.to_string()),
            _ => refused("provider.failed", format!("{host} answered without any text")),
        }
    }
}
