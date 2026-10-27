//! A meeting said confidential (D60): `confidential` of `contracts/session-audio/v2`, written and
//! removed whole on the user's word; who hears and who writes it, on this computer alone, whatever
//! the settings name; what had already left before it was said.

use std::{fs, io::ErrorKind, path::Path};

use wholeheard_shell::{
    confidential::{self, Left, Wait, What},
    meeting, minutes, providers, sessions,
    settings::{Engine, Settings},
    transcription::Hearer,
};

const SECTIONS: [&str; 3] = ["Points saillants", "Actions", "Points clés"];
const OLLAMA: &str = "http://localhost:11434";

fn meeting_folder() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::write(
        folder.path().join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-27T14:05:00+01:00","stopped":"2026-10-27T14:35:00+01:00","tracks":{}}"#,
    )
    .unwrap();
    folder
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_mark_is_an_empty_file_written_whole_and_removed_whole() {
    let folder = meeting_folder();
    assert!(!confidential::marked(folder.path()));
    confidential::mark(folder.path(), true).unwrap();
    assert!(confidential::marked(folder.path()));
    assert_eq!(fs::read(folder.path().join("confidential")).unwrap(), b"");
    // Nothing left beside it.
    assert_eq!(names(folder.path()), ["confidential", "session.json"]);
    // Said twice: the same one file.
    confidential::mark(folder.path(), true).unwrap();
    assert_eq!(names(folder.path()), ["confidential", "session.json"]);
    // Turned off: gone, and nothing else is.
    confidential::mark(folder.path(), false).unwrap();
    assert!(!confidential::marked(folder.path()));
    assert_eq!(names(folder.path()), ["session.json"]);
    // Off when it is off already: no failure.
    confidential::mark(folder.path(), false).unwrap();
    assert_eq!(names(folder.path()), ["session.json"]);
}

#[test]
fn a_mark_with_something_in_it_is_still_the_mark() {
    // The contract says empty; one written by hand with a word in it is the user's word all the same.
    let folder = meeting_folder();
    fs::write(folder.path().join("confidential"), "oui\n").unwrap();
    assert!(confidential::marked(folder.path()));
    // A folder of that name is no mark.
    let other = meeting_folder();
    fs::create_dir(other.path().join("confidential")).unwrap();
    assert!(!confidential::marked(other.path()));
}

#[test]
fn no_mark_is_written_before_the_meeting_s_folder_holds_its_session() {
    let folder = tempfile::tempdir().unwrap();
    let refused = confidential::mark(folder.path(), true).unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::NotFound);
    assert!(names(folder.path()).is_empty(), "{:?}", names(folder.path()));
    assert!(confidential::mark(&folder.path().join("absent"), true).is_err());
    assert!(!folder.path().join("absent").exists());
}

#[test]
fn the_list_says_which_meetings_are_confidential() {
    let root = tempfile::tempdir().unwrap();
    for name in ["kept", "said"] {
        let folder = root.path().join(name);
        fs::create_dir(&folder).unwrap();
        fs::write(
            folder.join("session.json"),
            r#"{"contract":"session-audio/v2","started":"2026-10-27T14:05:00+01:00","tracks":{}}"#,
        )
        .unwrap();
    }
    confidential::mark(&root.path().join("said"), true).unwrap();
    let listed: Vec<(String, bool)> = sessions::list(root.path(), None)
        .into_iter()
        .map(|listed| {
            (
                listed.folder.file_name().unwrap().to_string_lossy().into_owned(),
                listed.confidential,
            )
        })
        .collect();
    assert_eq!(listed, [("said".to_string(), true), ("kept".to_string(), false)]);
}

fn settings(engine: Engine) -> Settings {
    Settings {
        engine,
        model: "/models/ggml-large-v3-turbo.bin".into(),
        transcription_provider: Some("https://api.groq.com/openai/v1".into()),
        transcription_model: Some("whisper-large-v3".into()),
        transcription_agreed: Some("api.groq.com".into()),
        ..Default::default()
    }
}

#[test]
fn a_confidential_meeting_is_heard_by_whisper_with_the_settings_model_or_by_speechanalyzer_chosen() {
    let whisper = Hearer::Model("/models/ggml-large-v3-turbo.bin".into());
    // A service named and agreed to: Whisper hears, with the settings' model — never the service.
    assert_eq!(confidential::hearer(&settings(Engine::Service), "fr", || true), whisper);
    // A service on this computer too: Whisper, as the criterion says.
    let mut here = settings(Engine::Service);
    here.transcription_provider = Some("http://localhost:8000/v1".into());
    assert_eq!(confidential::hearer(&here, "fr", || true), whisper);
    // Whisper chosen: Whisper.
    assert_eq!(confidential::hearer(&settings(Engine::Whisper), "fr", || true), whisper);
    // Apple's SpeechAnalyzer chosen already, and offered: it hears, on the Mac.
    assert_eq!(
        confidential::hearer(&settings(Engine::SpeechAnalyzer), "fr", || true),
        Hearer::SpeechAnalyzer { language: "fr".into() }
    );
    // Chosen and no longer offered: Whisper.
    assert_eq!(confidential::hearer(&settings(Engine::SpeechAnalyzer), "fr", || false), whisper);
    // The same settings, not confidential, would send to the service: the mark is what overrides.
    assert!(matches!(
        Hearer::of(&settings(Engine::Service), "fr", || true),
        Hearer::Service { .. }
    ));
}

fn named(provider: &str, model: &str) -> Option<(String, String)> {
    Some((provider.to_string(), model.to_string()))
}

fn held(models: &[&str]) -> impl FnOnce() -> Option<Vec<String>> {
    let models: Vec<String> = models.iter().map(|model| model.to_string()).collect();
    move || Some(models)
}

#[test]
fn a_confidential_meeting_s_minutes_are_written_by_ollama_s_model_the_settings_name() {
    let ollama = providers::ollama_address(OLLAMA);
    // The settings name Ollama's: that model, and Ollama is not even asked.
    let asked = std::cell::Cell::new(false);
    let chosen = confidential::writer(named(&ollama, "mistral:7b"), OLLAMA, || {
        asked.set(true);
        Some(vec![])
    });
    assert_eq!(chosen, Ok((ollama.clone(), "mistral:7b".to_string())));
    assert!(!asked.get());
    // Another server of this computer: nothing leaves it either, it is kept.
    assert_eq!(
        confidential::writer(named("http://127.0.0.1:8080/v1", "llama"), OLLAMA, held(&[])),
        Ok(("http://127.0.0.1:8080/v1".to_string(), "llama".to_string()))
    );
}

#[test]
fn a_service_named_gives_way_to_the_recommended_model_else_the_first_ollama_holds() {
    let ollama = providers::ollama_address(OLLAMA);
    let service = named("https://openrouter.ai/api/v1", "mistralai/mistral-small");
    // The recommended one held, among others: it — wherever it is in the list.
    assert_eq!(
        confidential::writer(service.clone(), OLLAMA, held(&["gemma3:4b", providers::RECOMMENDED, "mistral:7b"])),
        Ok((ollama.clone(), providers::RECOMMENDED.to_string()))
    );
    // Not held: the first Ollama holds.
    assert_eq!(
        confidential::writer(service.clone(), OLLAMA, held(&["mistral:7b", "gemma3:4b"])),
        Ok((ollama.clone(), "mistral:7b".to_string()))
    );
    // Nobody named at all: the same.
    assert_eq!(
        confidential::writer(None, OLLAMA, held(&[providers::RECOMMENDED])),
        Ok((ollama.clone(), providers::RECOMMENDED.to_string()))
    );
}

#[test]
fn ollama_silent_or_empty_the_minutes_wait_and_nothing_is_sent() {
    let service = named("https://openrouter.ai/api/v1", "mistralai/mistral-small");
    assert_eq!(confidential::writer(service.clone(), OLLAMA, || None), Err(Wait::Silent));
    assert_eq!(confidential::writer(service, OLLAMA, held(&[])), Err(Wait::Empty));
    assert_eq!(confidential::writer(None, OLLAMA, || None), Err(Wait::Silent));
    // An address that is none is never taken for this computer's.
    assert_eq!(
        confidential::writer(named("localhost:11434", "qwen3:8b"), OLLAMA, || None),
        Err(Wait::Silent)
    );
    // An Ollama elsewhere is asked nothing: waited for, silent.
    let asked = std::cell::Cell::new(false);
    let elsewhere = confidential::writer(
        named("https://openrouter.ai/api/v1", "mistralai/mistral-small"),
        "http://192.0.2.7:11434",
        || {
            asked.set(true);
            Some(vec![providers::RECOMMENDED.to_string()])
        },
    );
    assert_eq!((elsewhere, asked.get()), (Err(Wait::Silent), false));
}

fn transcript_by(folder: &Path, engine: &str) {
    fs::write(
        folder.join("transcript.jsonl"),
        format!(
            "{{\"type\":\"header\",\"contract\":\"transcript/v1\",\"engine\":\"{engine}\",\"model\":\"m\",\"language\":null,\"context\":null}}\n\
             {{\"type\":\"segment\",\"speaker\":\"me\",\"track\":\"local\",\"start\":0.0,\"end\":1.0,\"text\":\"Bonjour.\"}}\n\
             {{\"type\":\"complete\",\"duration\":1.0}}\n"
        ),
    )
    .unwrap();
}

fn minutes_by(folder: &Path, provider: &str) {
    fs::write(
        folder.join("minutes.md"),
        format!(
            "---\ncontract: minutes/v1\nmade: 2026-10-27T15:00:00+01:00\nprovider: \"{provider}\"\nmodel: \"m\"\ncontext: null\n---\n\n## A\n\nx\n\n## B\n\n- y\n\n## C\n\n- z\n"
        ),
    )
    .unwrap();
}

fn left_of(folder: &Path) -> Vec<Left> {
    confidential::left(&meeting::transcript(folder), &minutes::minutes(folder, SECTIONS))
}

#[test]
fn what_had_left_is_read_from_who_heard_and_who_wrote() {
    let folder = meeting_folder();
    // Nothing made: nothing left.
    assert_eq!(left_of(folder.path()), []);
    // Made on this computer: nothing left.
    transcript_by(folder.path(), "whisper.cpp");
    minutes_by(folder.path(), "local");
    assert_eq!(left_of(folder.path()), []);
    transcript_by(folder.path(), "SpeechAnalyzer");
    assert_eq!(left_of(folder.path()), []);
    // A service on this computer: nothing left either.
    transcript_by(folder.path(), "localhost:8000");
    assert_eq!(left_of(folder.path()), []);
    // A service heard it: its sound left, to the host the header names.
    transcript_by(folder.path(), "api.groq.com");
    // A service wrote them: the transcript left, to the host the front matter names.
    minutes_by(folder.path(), "openrouter.ai");
    assert_eq!(
        left_of(folder.path()),
        [
            Left {
                what: What::Sound,
                host: "api.groq.com".into()
            },
            Left {
                what: What::Transcript,
                host: "openrouter.ai".into()
            },
        ]
    );
}

#[test]
fn what_had_left_is_said_once_each() {
    let mut left = vec![Left {
        what: What::Sound,
        host: "api.groq.com".into(),
    }];
    confidential::add(&mut left, What::Sound, "api.groq.com");
    confidential::add(&mut left, What::Transcript, "api.groq.com");
    confidential::add(&mut left, What::Transcript, "api.groq.com");
    assert_eq!(
        left,
        [
            Left {
                what: What::Sound,
                host: "api.groq.com".into()
            },
            Left {
                what: What::Transcript,
                host: "api.groq.com".into()
            },
        ]
    );
}
