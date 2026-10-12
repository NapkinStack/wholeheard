//! The one connection `summary` opens: to the address the user configured, speaking the
//! OpenAI protocol — a model on the machine or an online one, the module does not care.
//!
//! The host named is the host contacted: the address is read by the HTTP client's own
//! reader, a redirection is refused, and no proxy the environment names is used
//! (rules/security.md, 1).

use std::{net::IpAddr, time::Duration};

use ureq::http::Uri;

use crate::Refused;

fn refused<T>(why: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(why.into()))
}

/// Where the minutes are asked for.
#[derive(Debug)]
pub struct Provider {
    uri: Uri,
    model: String,
    key: Option<String>,
    patience: Duration,
}

impl Provider {
    /// `address` goes up to and without `/chat/completions`, e.g. `http://localhost:11434/v1`.
    /// A key, when there is one, is sent as a bearer token and written nowhere.
    pub fn new(address: &str, model: &str, key: Option<&str>, patience: Duration) -> Result<Self, Refused> {
        if address.contains(['?', '#', '@']) || address.chars().any(char::is_whitespace) {
            return refused("the provider's address holds more than an address: no query, no fragment, no user or key in it");
        }
        let joined = format!("{}/chat/completions", address.trim_end_matches('/'));
        let Ok(uri) = joined.parse::<Uri>() else {
            return refused("the provider's address is not an address");
        };
        if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none_or(str::is_empty) {
            return refused("the provider's address must start with http:// or https:// and name a host");
        }
        if model.is_empty() || model.chars().any(char::is_control) {
            return refused("the model's name is empty or holds a character that is not text");
        }
        let key = key.filter(|key| !key.is_empty());
        if key.is_some_and(|key| !key.chars().all(|c| c.is_ascii_graphic() || c == ' ')) {
            return refused("the key holds a character a request cannot carry");
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
        let host = self.uri.host().unwrap_or_default().trim_start_matches('[').trim_end_matches(']');
        let name = host.trim_end_matches('.').to_ascii_lowercase();
        let here = name == "localhost" || name.parse::<IpAddr>().is_ok_and(|address| address.is_loopback());
        if here { "local".into() } else { name }
    }

    /// Asks once, and gives the answer's text — or what failed, in one sentence.
    pub fn ask(&self, request: &str) -> Result<String, Refused> {
        let host = self.host();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .proxy(None)
            .http_status_as_error(false)
            .timeout_global(Some(self.patience))
            .build()
            .into();
        let mut call = agent.post(&self.uri);
        if let Some(key) = &self.key {
            call = call.header("Authorization", format!("Bearer {key}"));
        }
        let body = serde_json::json!({ "model": self.model, "stream": false, "messages": [{ "role": "user", "content": request }] });
        let mut response = match call.send_json(&body) {
            Ok(response) => response,
            Err(error) => return refused(format!("the provider at {host} could not be reached: {error}")),
        };
        match response.status().as_u16() {
            200 => {}
            status @ (401 | 403) => return refused(format!("the provider at {host} refused the key (HTTP {status})")),
            404 => {
                return refused(format!(
                    "the provider at {host} does not know this model or this address (HTTP 404)"
                ));
            }
            status @ 300..=399 => {
                return refused(format!(
                    "the provider at {host} sent the request elsewhere, and it was not followed (HTTP {status})"
                ));
            }
            status => return refused(format!("the provider at {host} failed (HTTP {status})")),
        }
        let Ok(answer) = response.body_mut().read_json::<serde_json::Value>() else {
            return refused(format!("the provider at {host} sent an answer that could not be read to its end"));
        };
        let choice = &answer["choices"][0];
        // An answer is whole when the provider says it stopped by itself, or says nothing.
        match choice["finish_reason"].as_str() {
            None | Some("stop") => {}
            Some("length") => {
                return refused(format!(
                    "the provider at {host} cut its answer short: the minutes would be incomplete"
                ));
            }
            Some(_) => return refused(format!("the provider at {host} stopped before the end of its answer")),
        }
        match choice["message"]["content"].as_str() {
            Some(text) if !text.trim().is_empty() => Ok(text.to_string()),
            _ => refused(format!("the provider at {host} answered without any text")),
        }
    }
}
