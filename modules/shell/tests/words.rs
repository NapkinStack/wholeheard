//! What the user is told: the sentences, read here as a user reads them, in both languages.

use std::path::PathBuf;

use summit_shell::{
    recording::{Said, Source},
    words::{self, Language, Notice},
};

const BOTH: [Language; 2] = [Language::English, Language::French];

fn told(said: &Said, reminder: bool, language: Language) -> (String, String) {
    let Notice { title, body } = words::of(said, reminder, language).expect("nothing was said");
    (title, body)
}

#[test]
fn the_interface_follows_the_systems_language_and_any_other_is_english() {
    for french in ["fr", "fr-FR", "fr_CA.UTF-8", "FR", "fr@euro"] {
        assert_eq!(Language::of(french), Language::French, "{french}");
    }
    for other in ["en-GB", "en_US.UTF-8", "de-DE", "C", "POSIX", "", "fra", "french", "frisian-NL"] {
        assert_eq!(Language::of(other), Language::English, "{other}");
    }
}

#[test]
fn the_reminder_is_a_setting_and_a_source_that_comes_back_calls_for_no_word() {
    let started = Said::Started(PathBuf::from("/s"));
    for language in BOTH {
        assert_eq!(words::of(&started, false, language), None);
        assert_eq!(words::of(&Said::Heard(Source::Me), true, language), None);
        assert_eq!(words::of(&Said::Heard(Source::Others), true, language), None);
    }
    assert_eq!(
        told(&started, true, Language::English),
        (
            "Recording".into(),
            "Tell the participants that this meeting is being recorded.".into()
        )
    );
    assert_eq!(
        told(&started, true, Language::French),
        (
            "Enregistrement en cours".into(),
            "Prévenez les participants que cette réunion est enregistrée.".into()
        )
    );
}

#[test]
fn a_source_that_is_silent_or_refused_is_named_and_the_recording_is_said_to_go_on() {
    for (source, english, french) in [(Source::Me, "microphone", "micro"), (Source::Others, "meeting", "réunion")] {
        let (title, body) = told(&Said::Silent(source), false, Language::English);
        assert!(
            title.contains(english) && body.ends_with("The recording goes on."),
            "{title} — {body}"
        );
        let (title, body) = told(&Said::Silent(source), false, Language::French);
        assert!(
            title.contains(french) && body.ends_with("L’enregistrement continue."),
            "{title} — {body}"
        );
        // A source refused is named in the title, and the body says the other is still recorded.
        for language in BOTH {
            let (title, body) = told(&Said::Unavailable(source, String::new()), false, language);
            let (named, other) = if source == Source::Me {
                (french, "réunion")
            } else {
                (french, "micro")
            };
            let named = if language == Language::English { english } else { named };
            let other = match (language, source) {
                (Language::English, Source::Me) => "meeting",
                (Language::English, Source::Others) => "microphone",
                _ => other,
            };
            assert!(
                title.contains(named) && body.contains(other) && !body.contains('('),
                "{title} — {body}"
            );
        }
    }
    // What the program said of a refusal comes last, as it said it — a colon of its own kept.
    let refused = Said::Unavailable(Source::Me, "ALSA: the system names no default device".into());
    assert_eq!(
        told(&refused, false, Language::English),
        (
            "Your microphone is not recorded".into(),
            "The meeting’s sound is still recorded. (ALSA: the system names no default device)".into()
        )
    );
    assert_eq!(
        told(&refused, false, Language::French).1,
        "Le son de la réunion est toujours enregistré. (ALSA: the system names no default device)"
    );
}

#[test]
fn french_is_written_as_french_is_printed() {
    // A space that does not break before a colon and a question mark: a notification never
    // starts a line on one.
    let silent = told(&Said::Silent(Source::Others), false, Language::French).1;
    assert!(
        silent.contains("secondes\u{a0}: l’appel") && silent.contains("appareil\u{202f}?"),
        "{silent}"
    );
    assert_eq!(
        told(&Said::Stopped("/s".into(), 1912.4), false, Language::French).1,
        "Durée\u{a0}: 32 min."
    );
    // No straight apostrophe in a sentence of ours, in either language.
    let all = [
        Said::Started("/s".into()),
        Said::Unavailable(Source::Me, String::new()),
        Said::Unavailable(Source::Others, String::new()),
        Said::Silent(Source::Me),
        Said::Silent(Source::Others),
        Said::Stopped("/s".into(), 60.0),
        Said::Failed(String::new()),
        Said::Failed("x".into()),
    ];
    for (said, language) in all.iter().flat_map(|said| BOTH.map(|language| (said, language))) {
        let (title, body) = told(said, true, language);
        assert!(!title.contains('\'') && !body.contains('\''), "{title} — {body}");
    }
}

#[test]
fn a_length_reads_as_a_person_says_it_each_to_the_nearest() {
    for (seconds, said) in [
        (0.0, "0 s"),
        (0.4, "0 s"),
        (0.5, "1 s"),
        (59.4, "59 s"),
        (59.5, "1 min"),
        (89.4, "1 min"),
        (90.0, "2 min"),
        (119.4, "2 min"),
        (1912.4, "32 min"),
        (3569.0, "59 min"),
        (3570.0, "1 h 00"),
        (5400.0, "1 h 30"),
        (86400.0, "24 h 00"),
        (-3.0, "0 s"),
        (f64::NAN, "0 s"),
        (f64::INFINITY, "0 s"),
    ] {
        assert_eq!(words::length(seconds), said, "{seconds}");
    }
    assert_eq!(
        told(&Said::Stopped("/s".into(), 42.4), false, Language::English).1,
        "42 s recorded."
    );
}

#[test]
fn a_failure_says_what_the_program_said_of_it_and_nothing_else_of_the_program() {
    let said = |sentence: &str, language| told(&Said::Failed(sentence.into()), false, language).1;
    // The program that records names itself before its sentence: the name is not the user's to read.
    assert_eq!(
        said("summit-capture: no source could be opened: nothing to record", Language::English),
        "What was recorded is in the session’s folder. (no source could be opened: nothing to record)"
    );
    assert!(said("summit-capture: the disk is full", Language::French).ends_with("dossier de la session. (the disk is full)"));
    // Any other sentence is kept whole, whatever stands before its colon.
    for whole in [
        "Timeout: no sound for 30 s",
        "Error: the disk is full",
        "Summit Capture: the disk is full",
    ] {
        assert!(said(whole, Language::English).ends_with(&format!("({whole})")), "{whole}");
    }
    // A program that said nothing — or its name and nothing — is said to have said nothing.
    for nothing in ["", "   ", "summit-capture:", " summit-capture: \t", "()"] {
        for language in BOTH {
            let body = said(nothing, language);
            assert!(
                !body.contains('(') && (body.contains("without saying why") || body.contains("sans dire pourquoi")),
                "{nothing:?}: {body}"
            );
        }
    }
    // Brackets of its own are not doubled, and a very long sentence is cut short.
    assert!(said("(os error 28)", Language::English).ends_with("folder. (os error 28)"));
    let long = said(&"é".repeat(5000), Language::English);
    assert!(
        long.chars().count() < 300 && long.ends_with("…)"),
        "{} characters",
        long.chars().count()
    );
}
