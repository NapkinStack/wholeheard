//! The one connection `summary` opens: to the address the user configured, speaking the
//! OpenAI protocol — a model on the machine or an online one, the program does not care.
//!
//! Nothing here follows a redirection: the host that answers is the host that was configured
//! (rules/security.md, 1).

use std::time::Duration;

use crate::Refused;

/// Where the minutes are asked for.
pub struct Provider<'a> {
    /// The address up to and without `/chat/completions`, e.g. `http://localhost:11434/v1`.
    pub address: &'a str,
    pub model: &'a str,
    /// Sent as a bearer token when there is one; never written anywhere.
    pub key: Option<&'a str>,
    pub patience: Duration,
}

impl Provider<'_> {
    /// The host as the minutes name it: `local` for this machine, the host's name otherwise.
    pub fn host(&self) -> String {
        let after = self.address.split_once("://").map_or(self.address, |(_, rest)| rest);
        let host = after.split(['/', '?']).next().unwrap_or("").rsplit('@').next().unwrap_or("");
        let name = if host.starts_with('[') {
            host.split(']').next().unwrap_or("").trim_start_matches('[')
        } else {
            host.split(':').next().unwrap_or("")
        };
        if matches!(name, "localhost" | "127.0.0.1" | "::1") {
            "local".into()
        } else {
            name.to_string()
        }
    }

    /// Asks once, and gives the answer's text — or what failed, in one sentence.
    pub fn ask(&self, request: &str) -> Result<String, Refused> {
        let refused = |why: String| Err(Refused(why));
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(self.patience))
            .build()
            .into();
        let mut call = agent.post(format!("{}/chat/completions", self.address.trim_end_matches('/')));
        if let Some(key) = self.key.filter(|key| !key.is_empty()) {
            call = call.header("Authorization", format!("Bearer {key}"));
        }
        let body = serde_json::json!({ "model": self.model, "stream": false, "messages": [{ "role": "user", "content": request }] });
        let mut response = match call.send_json(&body) {
            Ok(response) => response,
            Err(error) => return refused(format!("the provider at {} could not be reached: {error}", self.host())),
        };
        let status = response.status().as_u16();
        let answer: serde_json::Value = response.body_mut().read_json().unwrap_or_default();
        match status {
            200 => {}
            401 | 403 => return refused(format!("the provider at {} refused the key (HTTP {status})", self.host())),
            404 => {
                return refused(format!(
                    "the provider at {} does not know the model {} or this address (HTTP 404)",
                    self.host(),
                    self.model
                ));
            }
            300..=399 => {
                return refused(format!(
                    "the provider at {} sent the request elsewhere, and it was not followed (HTTP {status})",
                    self.host()
                ));
            }
            _ => return refused(format!("the provider at {} failed (HTTP {status})", self.host())),
        }
        let choice = &answer["choices"][0];
        let Some(text) = choice["message"]["content"].as_str().filter(|text| !text.trim().is_empty()) else {
            return refused(format!("the provider at {} answered without any text", self.host()));
        };
        if choice["finish_reason"] == "length" {
            return refused(format!(
                "the provider at {} cut its answer short: the minutes would be incomplete",
                self.host()
            ));
        }
        Ok(text.to_string())
    }
}
