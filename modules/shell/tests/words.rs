//! What the user is told: the sentences, read here as a user reads them, in both languages.

use std::path::PathBuf;

use summit_shell::{
    recording::{Said, Source},
    words::{self, Language, Notice},
};

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
    for language in [Language::English, Language::French] {
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
    let (title, body) = told(&Said::Silent(Source::Me), false, Language::English);
    assert!(
        title.contains("your microphone") && body.ends_with("The recording goes on."),
        "{title} — {body}"
    );
    let (title, body) = told(&Said::Silent(Source::Others), false, Language::French);
    assert!(
        title.contains("la réunion") && body.ends_with("L'enregistrement continue."),
        "{title} — {body}"
    );

    // What the program said of a refusal comes last, as it said it.
    let refused = Said::Unavailable(Source::Me, "the system names no default device".into());
    assert_eq!(
        told(&refused, false, Language::English),
        (
            "Your microphone is not recorded".into(),
            "The meeting's sound is still recorded. (the system names no default device)".into()
        )
    );
    assert_eq!(
        told(&refused, false, Language::French).1,
        "Le son de la réunion reste enregistré. (the system names no default device)"
    );
    // A refusal with no reason given says none.
    assert_eq!(
        told(&Said::Unavailable(Source::Others, String::new()), false, Language::French),
        (
            "Le son de la réunion n'est pas enregistré".into(),
            "Votre micro reste enregistré.".into()
        )
    );
}

#[test]
fn a_stop_says_how_long_and_a_failure_what_the_program_said_of_it() {
    let folder = PathBuf::from("/s");
    for (seconds, said) in [
        (0.2, "0 s"),
        (42.4, "42 s"),
        (59.6, "1 min"),
        (1912.4, "31 min"),
        (3600.0, "1 h 00"),
        (5400.0, "1 h 30"),
    ] {
        assert_eq!(
            told(&Said::Stopped(folder.clone(), seconds), false, Language::English).1,
            format!("{said} recorded.")
        );
        assert_eq!(
            told(&Said::Stopped(folder.clone(), seconds), false, Language::French).1,
            format!("Durée : {said}.")
        );
    }
    // A program names itself before its sentence: the name is not the user's to read.
    let failed = Said::Failed("summit-capture: no source could be opened: nothing to record".into());
    assert_eq!(
        told(&failed, false, Language::English).1,
        "What it had recorded is in the session's folder. (no source could be opened: nothing to record)"
    );
    assert!(
        told(&failed, false, Language::French)
            .1
            .ends_with("(no source could be opened: nothing to record)")
    );
    // One that gave no sentence is said to have given none.
    assert!(
        told(&Said::Failed(String::new()), false, Language::English)
            .1
            .contains("without saying why")
    );
    assert!(
        told(&Said::Failed(String::new()), false, Language::French)
            .1
            .contains("sans dire pourquoi")
    );
    // A sentence with a colon of its own, and no program's name, is kept whole.
    let own = Said::Failed("the disk is full: nothing more is written".into());
    assert!(
        told(&own, false, Language::English)
            .1
            .ends_with("(the disk is full: nothing more is written)")
    );
}
