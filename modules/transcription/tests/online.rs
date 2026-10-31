//! A transcription service the user names (D43, ADR-0013), tried against a server played here on
//! this machine: what is sent — the span, its model, the times asked, the key —, and what is made of
//! what it answers. No meeting, no service of anyone's.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

use wholeheard_transcription::{
    online::Service,
    step::{Engine, RATE},
};

/// Answers each request with `status` and `body`; gives its address, and what was asked.
fn served(status: u16, answered: &'static str) -> (String, mpsc::Receiver<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tell, asked) = mpsc::channel();
    // A session sends a span per track.
    thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                head.push_str(&line);
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let _ = tell.send((head, body));
            let mut stream = stream;
            let answer = format!(
                "HTTP/1.1 {status} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answered}",
                answered.len()
            );
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    (address, asked)
}

const SEGMENTS: &str = r#"{"text":"Bonjour à tous. On commence ?","segments":[{"id":0,"start":0.5,"end":1.75,"text":" Bonjour à tous."},{"id":1,"start":2.0,"end":3.5,"text":" On commence ?"}]}"#;

#[test]
fn a_span_is_sent_as_the_audio_route_asks_and_its_segments_are_the_span_s() {
    let (address, asked) = served(200, SEGMENTS);
    let mut service = Service::new(&address, "whisper-large-v3", Some("a-key-of-the-test")).unwrap();
    assert!(
        service.name().0.starts_with("127.0.0.1:") && service.name().1 == "whisper-large-v3",
        "{:?}",
        service.name()
    );
    let heard = service
        .hear(&vec![0.25f32; RATE * 4], Some("fr"), Some("Ordre du jour : le budget."))
        .unwrap();
    let said: Vec<_> = heard
        .iter()
        .map(|segment| (segment.start, segment.end, segment.text.as_str()))
        .collect();
    assert_eq!(said, [(0.5, 1.75, "Bonjour à tous."), (2.0, 3.5, "On commence ?")]);
    let (head, body) = asked.recv().unwrap();
    let body = String::from_utf8_lossy(&body);
    assert!(head.starts_with("POST /v1/audio/transcriptions "), "{head}");
    assert!(
        head.to_ascii_lowercase().contains("authorization: bearer a-key-of-the-test"),
        "the key, as a bearer token"
    );
    assert!(
        head.to_ascii_lowercase().contains("content-type: multipart/form-data; boundary="),
        "{head}"
    );
    for (field, value) in [
        ("model", "whisper-large-v3"),
        ("response_format", "verbose_json"),
        ("language", "fr"),
        ("prompt", "Ordre du jour : le budget."),
    ] {
        assert!(body.contains(&format!("name=\"{field}\"\r\n\r\n{value}\r\n")), "{field}");
    }
    assert!(
        body.contains("name=\"file\"; filename=\"span.wav\"") && body.contains("RIFF"),
        "the span, as a file"
    );
}

#[test]
fn a_service_that_gives_no_times_is_refused_for_the_span() {
    let (address, _asked) = served(200, r#"{"text":"Bonjour à tous."}"#);
    let mut service = Service::new(&address, "gpt-4o-transcribe", None).unwrap();
    let error = service.hear(&vec![0.25f32; RATE], Some("fr"), None).unwrap_err();
    assert!(error.to_string().contains("gives no times"), "{error}");
}

#[test]
fn what_the_service_refuses_is_said_in_one_sentence() {
    for (status, said) in [
        (401, "refused the key"),
        (403, "refused the key"),
        (404, "does not know this model"),
        (400, "refused the request"),
        (500, "failed"),
    ] {
        let (address, _asked) = served(status, r#"{"error":"no"}"#);
        let mut service = Service::new(&address, "a-model", None).unwrap();
        let error = service.hear(&vec![0.25f32; RATE], None, None).unwrap_err().to_string();
        assert!(
            error.contains(said) && error.contains(&status.to_string()) && !error.contains('\n'),
            "{status}: {error}"
        );
    }
}

#[test]
fn an_address_that_is_not_one_is_refused_before_anything_is_sent() {
    for address in [
        "ftp://example.com/v1",
        "https://example.com/v1?key=x",
        "https://user@example.com/v1",
        "example.com/v1",
        "https:// spaced.example/v1",
    ] {
        assert!(Service::new(address, "a-model", None).is_err(), "{address}");
    }
}

#[test]
fn a_service_that_does_not_answer_is_said_so() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    drop(listener);
    let mut service = Service::new(&address, "a-model", None).unwrap();
    let error = service.hear(&vec![0.25f32; RATE], None, None).unwrap_err().to_string();
    assert!(error.contains("could not be reached") && !error.contains('\n'), "{error}");
}

#[test]
fn a_service_that_sends_the_span_elsewhere_is_not_followed() {
    for status in [301, 302, 307, 308] {
        let (address, _asked) = served(status, r#"{"error":"moved"}"#);
        let mut service = Service::new(&address, "a-model", None).unwrap();
        assert!(
            service
                .hear(&vec![0.25f32; RATE], None, None)
                .unwrap_err()
                .to_string()
                .contains(&status.to_string()),
            "{status}"
        );
    }
}

#[test]
fn a_segment_of_no_words_is_left_out_and_one_ended_before_it_began_refused() {
    let (address, _asked) = served(
        200,
        r#"{"segments":[{"start":0.0,"end":0.5,"text":" "},{"start":0.5,"end":1.0,"text":"Oui."}]}"#,
    );
    let heard = Service::new(&address, "a-model", None)
        .unwrap()
        .hear(&vec![0.25f32; RATE], None, None)
        .unwrap();
    assert_eq!(heard.iter().map(|segment| segment.text.as_str()).collect::<Vec<_>>(), ["Oui."]);
    let (address, _asked) = served(200, r#"{"segments":[{"start":2.0,"end":1.0,"text":"Non."}]}"#);
    assert!(
        Service::new(&address, "a-model", None)
            .unwrap()
            .hear(&vec![0.25f32; RATE], None, None)
            .is_err()
    );
}

#[test]
fn the_test_sends_a_sound_of_its_own_and_reads_the_answer() {
    let (address, asked) = served(200, r#"{"text":"","segments":[]}"#);
    let service = Service::new(&address, "a-model", None).unwrap();
    assert_eq!(service.test(), Ok(()));
    let (_, body) = asked.recv().unwrap();
    assert!(String::from_utf8_lossy(&body).contains("RIFF"), "a sound was sent");
}

#[test]
fn the_program_writes_the_transcript_from_the_service_its_host_in_the_header() {
    let (address, _asked) = served(200, SEGMENTS);
    let folder = tempfile::tempdir().unwrap();
    std::fs::write(
        folder.path().join("session.json"),
        r#"{"contract":"session-audio/v1","started":"2026-10-26T10:00:00+01:00","stopped":"2026-10-26T10:00:05+01:00"}"#,
    )
    .unwrap();
    let mut audio = b"RIFF\xff\xff\xff\xffWAVEfmt \x10\0\0\0\x01\0\x01\0\x80\x3e\0\0\0\x7d\0\0\x02\0\x10\0data\xff\xff\xff\xff".to_vec();
    for n in 0..RATE * 5 {
        audio.extend_from_slice(&(if (RATE..RATE * 3).contains(&n) { 8000i16 } else { 0 }).to_le_bytes());
    }
    std::fs::write(folder.path().join("me.wav"), &audio).unwrap();
    // Both sides sound: each is heard by the service, and each segment goes to its own track.
    std::fs::write(folder.path().join("others.wav"), &audio).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .arg(folder.path())
        .args(["--provider", &address, "--model", "whisper-large-v3", "--language", "fr"])
        .env("WHOLEHEARD_LOG_DIR", std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("logs"))
        .env_remove("WHOLEHEARD_PROVIDER_KEY")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let transcript = std::fs::read_to_string(folder.path().join("transcript.jsonl")).unwrap();
    let header: serde_json::Value = serde_json::from_str(transcript.lines().next().unwrap()).unwrap();
    // The service's host with its port, and its model, as who heard.
    assert!(
        header["engine"].as_str().is_some_and(|engine| engine.starts_with("127.0.0.1:")) && header["model"] == "whisper-large-v3",
        "{header}"
    );
    let segments: Vec<serde_json::Value> = transcript
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .filter(|line: &serde_json::Value| line["type"] == "segment")
        .collect();
    for (speaker, track) in [("me", "local"), ("others", "remote")] {
        let theirs: Vec<_> = segments.iter().filter(|segment| segment["speaker"] == speaker).collect();
        assert!(!theirs.is_empty(), "no segment of {speaker}: {transcript}");
        assert!(
            theirs
                .iter()
                .all(|segment| segment["track"] == track && segment["start"].as_f64() < segment["end"].as_f64()),
            "{transcript}"
        );
    }
    assert!(transcript.lines().last().unwrap().contains("\"complete\""), "{transcript}");
}

#[test]
fn a_refusal_the_service_explains_is_said_in_its_words_on_one_line() {
    let (address, _asked) = served(
        400,
        r#"{"error":{"message":"The prompt is too long:\n at most 224 tokens.\u0007","type":"invalid_request_error"}}"#,
    );
    let error = Service::new(&address, "whisper-large-v3", None)
        .unwrap()
        .hear(&vec![0.25f32; RATE], Some("fr"), Some("a long context"))
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("HTTP 400")
            && error.contains("The prompt is too long: at most 224 tokens.")
            && !error.contains('\n')
            && !error.contains('\u{7}'),
        "{error}"
    );
    assert!(!error.contains("choose a model"), "{error}");
    // A sentence as long as a service may send: cut, still one line.
    let long: &'static str = Box::leak(format!(r#"{{"error":{{"message":"{}"}}}}"#, "x".repeat(5000)).into_boxed_str());
    let (address, _asked) = served(400, long);
    let error = Service::new(&address, "m", None)
        .unwrap()
        .hear(&vec![0.25f32; RATE], None, None)
        .unwrap_err()
        .to_string();
    assert!(error.chars().count() < 300 && error.ends_with('…'), "{}", error.len());
    // A sentence that reorders or hides what is read: its marks taken out.
    let (address, _asked) = served(400, r#"{"error":{"message":"gpj.\u202eexe\u200b"}}"#);
    let error = Service::new(&address, "m", None)
        .unwrap()
        .hear(&vec![0.25f32; RATE], None, None)
        .unwrap_err()
        .to_string();
    assert!(error.ends_with("(HTTP 400): gpj.exe"), "{error:?}");
    // `error` a sentence itself, as some services give it.
    let (address, _asked) = served(400, r#"{"error":"model not found"}"#);
    let error = Service::new(&address, "m", None)
        .unwrap()
        .hear(&vec![0.25f32; RATE], None, None)
        .unwrap_err()
        .to_string();
    assert!(error.ends_with("(HTTP 400): model not found"), "{error}");
    // Nothing readable: the likeliest cause, a model without times.
    let (address, _asked) = served(400, "not json");
    let error = Service::new(&address, "m", None)
        .unwrap()
        .hear(&vec![0.25f32; RATE], None, None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("choose a model that gives each segment's times"), "{error}");
}

fn tested(address: &str) -> (bool, String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
        .args(["--test", "--provider", address, "--model", "a-model"])
        .env("WHOLEHEARD_LOG_DIR", std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("logs"))
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_test_that_fails_says_its_cause_on_the_output_s_last_line() {
    for (status, answered, code) in [
        (401, "{}", "provider.refused_key"),
        (403, "{}", "provider.refused_key"),
        (402, "{}", "provider.no_credit"),
        (404, "{}", "provider.unknown_model"),
        (429, "{}", "provider.too_many"),
        (500, "{}", "provider.down"),
        (503, "{}", "provider.down"),
        (418, "{}", "provider.failed"),
        (301, "{}", "provider.redirected"),
        (308, "{}", "provider.redirected"),
        (200, r#"{"segments":[{"start":0.0,"end":1.0}]}"#, "provider.failed"),
        (200, r#"{"segments":[{"text":"sans temps"}]}"#, "provider.no_times"),
        (400, "{}", "provider.no_times"),
        (200, r#"{"text":"no times"}"#, "provider.no_times"),
        (200, "not json", "provider.failed"),
    ] {
        let (address, _asked) = served(status, answered);
        let (passed, out, err) = tested(&address);
        assert!(!passed, "{status}");
        let last: serde_json::Value = serde_json::from_str(out.lines().last().unwrap_or("{}")).unwrap();
        assert_eq!(
            (last["event"].as_str(), last["code"].as_str()),
            (Some("failed"), Some(code)),
            "{status} {answered}"
        );
        assert!(err.starts_with("wholeheard-transcription: "), "{err}");
    }
}

#[test]
fn a_test_that_passes_says_nothing_and_one_in_the_service_s_own_words_names_no_cause() {
    let (address, _asked) = served(200, SEGMENTS);
    let (passed, out, _) = tested(&address);
    assert!(passed && out.is_empty(), "{out}");
    let (address, _asked) = served(400, r#"{"error":{"message":"model not found"}}"#);
    let (passed, out, err) = tested(&address);
    assert!(!passed && out.is_empty(), "{out}");
    assert!(err.contains("model not found"), "{err}");
}

#[test]
fn a_key_a_request_cannot_carry_is_said_before_anything_leaves() {
    for key in ["clé", "a b", "a\nb"] {
        let (address, asked) = served(200, SEGMENTS);
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_wholeheard-transcription"))
            .args(["--test", "--provider", &address, "--model", "a-model"])
            .env("WHOLEHEARD_PROVIDER_KEY", key)
            .env("WHOLEHEARD_LOG_DIR", std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("logs"))
            .output()
            .unwrap();
        let out = String::from_utf8_lossy(&output.stdout);
        assert!(out.contains("settings.invalid"), "{key:?} {out}");
        assert!(asked.try_recv().is_err(), "{key:?}: something left");
    }
}

#[test]
fn a_test_of_no_address_or_nobody_there_says_its_cause() {
    let (_, out, _) = tested("not an address");
    assert!(out.contains("settings.invalid"), "{out}");
    // A port nobody listens on, on this machine.
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
    let (_, out, _) = tested(&format!("http://{free}/v1"));
    assert!(out.contains("provider.unreachable"), "{out}");
}
