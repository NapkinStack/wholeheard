//! A transcription service the user names (D43, ADR-0013), behind the step's interface: each span
//! is sent to OpenAI's `/audio/transcriptions` at the address the user gave, its times asked for
//! (`verbose_json`); a service that gives no times is refused, never timed by guess. The key, when
//! there is one, is sent as a bearer token and written nowhere; nothing of the meeting is logged.

use std::time::Duration;

use ureq::http::Uri;

use crate::{
    Segment,
    speech::write_span,
    step::{Engine, RATE},
};

/// How long reaching the service may take, whatever the span.
const CONNECT: Duration = Duration::from_secs(15);

pub struct Service {
    uri: String,
    host: String,
    model: String,
    key: Option<String>,
}

impl Service {
    /// `address` goes up to and without `/audio/transcriptions`, e.g. `https://api.groq.com/openai/v1`.
    pub fn new(address: &str, model: &str, key: Option<&str>) -> Result<Self, String> {
        if address.contains(['?', '#', '@']) || address.chars().any(char::is_whitespace) {
            return Err("the service's address holds more than an address: no query, no fragment, no user or key in it".into());
        }
        let uri = format!("{}/audio/transcriptions", address.trim_end_matches('/'));
        let typed: Uri = uri.parse().map_err(|_| "the service's address is not an address".to_string())?;
        let (Some("http" | "https"), Some(authority)) = (typed.scheme_str(), typed.authority()) else {
            return Err("the service's address must start with http:// or https:// and name a host".into());
        };
        if model.trim().is_empty() {
            return Err("a model is named for the service".into());
        }
        Ok(Service {
            // With its port: two services of one host are two hearers, never one transcript continued.
            host: authority.as_str().to_string(),
            uri,
            model: model.to_string(),
            key: key.filter(|key| !key.is_empty()).map(str::to_string),
        })
    }

    /// A second of a sound of the program's own, sent as a span is: the service answers, and its
    /// answer reads — nothing of a meeting.
    pub fn test(&self) -> Result<(), String> {
        let tone: Vec<f32> = (0..RATE)
            .map(|n| 0.2 * (std::f32::consts::TAU * 440.0 * n as f32 / RATE as f32).sin())
            .collect();
        self.ask(&tone, None, None).map(|_| ())
    }

    /// One span sent, its segments read — or what failed, in one sentence.
    fn ask(&self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> Result<Vec<Segment>, String> {
        let host = &self.host;
        let mut audio = Vec::new();
        write_span(&mut audio, sound).map_err(|error| format!("the span could not be made ({error})"))?;
        let boundary = format!("wholeheard-{}-{}", std::process::id(), sound.len());
        let mut body = Vec::new();
        let mut field = |name: &str, value: &str| {
            body.extend_from_slice(
                format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
            );
        };
        field("model", &self.model);
        field("response_format", "verbose_json");
        if let Some(language) = language {
            field("language", language);
        }
        if let Some(context) = context {
            field("prompt", context);
        }
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"span.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(&audio);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        // A minute beyond twice the span's length: a service hears faster than the meeting lasted.
        let patience = Duration::from_secs(60) + Duration::from_secs_f64(2.0 * sound.len() as f64 / RATE as f64);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .proxy(None)
            .http_status_as_error(false)
            .timeout_global(Some(patience))
            .timeout_connect(Some(CONNECT))
            .build()
            .into();
        let mut call = agent
            .post(&self.uri)
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"));
        if let Some(key) = &self.key {
            call = call.header("Authorization", format!("Bearer {key}"));
        }
        let mut response = call.send(&body[..]).map_err(|error| match error {
            ureq::Error::Timeout(_) => format!("{host} did not answer in time"),
            error => format!("{host} could not be reached: {error}"),
        })?;
        match response.status().as_u16() {
            200 => {}
            status @ (401 | 403) => return Err(format!("{host} refused the key (HTTP {status})")),
            404 => return Err(format!("{host} does not know this model or this address (HTTP 404)")),
            400 => {
                return Err(format!(
                    "{host} refused the request (HTTP 400): choose a model that gives each segment's times"
                ));
            }
            status => return Err(format!("{host} failed (HTTP {status})")),
        }
        let answer: serde_json::Value = response
            .body_mut()
            .read_json()
            .map_err(|_| format!("{host} sent an answer that could not be read"))?;
        let Some(segments) = answer["segments"].as_array() else {
            return Err(format!("{host} gives no times: choose another model"));
        };
        segments
            .iter()
            // A segment of no words is no segment, as Whisper's own are left out.
            .filter(|segment| segment["text"].as_str().is_none_or(|text| !text.trim().is_empty()))
            .map(
                |segment| match (segment["start"].as_f64(), segment["end"].as_f64(), segment["text"].as_str()) {
                    (Some(start), Some(end), Some(text)) if start <= end => Ok(Segment {
                        start,
                        end,
                        text: text.trim().to_string(),
                    }),
                    _ => Err(format!("{host} sent a segment without its times")),
                },
            )
            .collect()
    }
}

impl Engine for Service {
    /// The service's host and its model, as the transcript's header says who heard.
    fn name(&self) -> (String, String) {
        (self.host.clone(), self.model.clone())
    }

    fn hear(&mut self, sound: &[f32], language: Option<&str>, context: Option<&str>) -> std::io::Result<Vec<Segment>> {
        self.ask(sound, language, context).map_err(std::io::Error::other)
    }
}
