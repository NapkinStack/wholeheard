//! What the user is told: the sentences, read here as a user reads them, in both languages.

use std::path::PathBuf;

use wholeheard_shell::{
    recording::{Said, Source},
    words::{self, Failure, Language, Notice, Refusal},
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
    let started = Said::Started(PathBuf::from("/s"), None, None);
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
fn the_recording_names_the_microphone_it_records() {
    let started = Said::Started(PathBuf::from("/s"), Some("Headset Mic".into()), None);
    assert_eq!(
        told(&started, true, Language::English).1,
        "Microphone recorded: Headset Mic. Tell the participants that this meeting is being recorded."
    );
    assert_eq!(
        told(&started, true, Language::French).1,
        "Micro enregistré\u{a0}: Headset Mic. Prévenez les participants que cette réunion est enregistrée."
    );
    for language in BOTH {
        assert_eq!(words::of(&started, false, language), None);
    }
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
    // What the program said of a refusal is the log's, never the user's (D29).
    let refused = Said::Unavailable(Source::Me, "ALSA: the system names no default device".into());
    assert_eq!(
        told(&refused, false, Language::English),
        (
            "Your microphone is not recorded for now".into(),
            "The meeting’s sound is still recorded.".into()
        )
    );
    assert_eq!(
        told(&refused, false, Language::French).1,
        "Le son de la réunion est toujours enregistré."
    );
}

#[test]
fn a_microphone_that_sends_nothing_is_told_from_one_that_hears_no_sound_and_named() {
    let mut sides = words::Sides::default();
    sides.told(&Said::Started("/s".into(), Some("Earbuds".into()), None), false, Language::French);
    let nothing = sides.told(&Said::Nothing, false, Language::French).unwrap();
    assert_eq!(nothing.title, "Votre micro ne transmet rien");
    assert!(
        nothing.body.contains("du micro Earbuds")
            && nothing.body.contains("pièce\u{a0}: vérifiez")
            && !nothing.body.contains('\'')
            && nothing.body.ends_with("L’enregistrement continue."),
        "{}",
        nothing.body
    );
    assert_ne!(nothing.title, told(&Said::Silent(Source::Me), false, Language::French).0);
    // Moved since: the device recorded now is the one named.
    sides.told(&Said::Moved(Source::Me, Some("Built-in Mic".into())), false, Language::English);
    let nothing = sides.told(&Said::Nothing, false, Language::English).unwrap();
    assert!(
        nothing.title == "Your microphone sends nothing" && nothing.body.contains("Built-in Mic"),
        "{} — {}",
        nothing.title,
        nothing.body
    );
    // No microphone left, said so: the zeros kept in its place are no news.
    sides.told(&Said::Lost(Source::Me), false, Language::French);
    assert_eq!(sides.told(&Said::Nothing, false, Language::French), None);
    // Named by nothing: the microphone, in so many words.
    let (title, body) = told(&Said::Nothing, false, Language::English);
    assert!(
        title == "Your microphone sends nothing" && body.contains("your microphone"),
        "{body}"
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
        "32 min enregistrées."
    );
    // No straight apostrophe in a sentence of ours, in either language.
    let all = [
        Said::Started("/s".into(), None, None),
        Said::Started("/s".into(), Some("Mic".into()), None),
        Said::Unavailable(Source::Me, String::new()),
        Said::Unavailable(Source::Others, String::new()),
        Said::Silent(Source::Me),
        Said::Silent(Source::Others),
        Said::Nothing,
        Said::Stopped("/s".into(), 60.0),
        Said::Moved(Source::Me, Some("Mic".into())),
        Said::Moved(Source::Me, None),
        Said::Moved(Source::Others, Some("Speaker".into())),
        Said::Moved(Source::Others, None),
        Said::Lost(Source::Me),
        Said::Lost(Source::Others),
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
fn a_failure_never_says_what_the_program_said_of_it() {
    let said = |sentence: &str, language| told(&Said::Failed(sentence.into()), false, language).1;
    // The sentence is the user's; what the program said is the log's, and the developer mode's (D29).
    for sentence in [
        "wholeheard-capture: no source could be opened: nothing to record",
        "Timeout: no sound for 30 s",
        "(os error 28)",
        &"é".repeat(5000),
    ] {
        for language in BOTH {
            let body = said(sentence, language);
            assert!(
                !body.contains('(') && (body.ends_with("session’s folder.") || body.ends_with("dossier de la réunion.")),
                "{sentence:?}: {body}"
            );
        }
    }
    // A program that said nothing — or its name and nothing — is said to have said nothing.
    for nothing in ["", "   ", "wholeheard-capture:", " wholeheard-capture: \t", "()"] {
        for language in BOTH {
            let body = said(nothing, language);
            assert!(
                !body.contains('(') && (body.contains("without saying why") || body.contains("sans dire pourquoi")),
                "{nothing:?}: {body}"
            );
        }
    }
}

#[test]
fn no_notice_of_a_failure_carries_what_a_program_or_the_system_said() {
    use wholeheard_shell::{
        transcription::Made,
        words::{Minutes, Trouble},
    };
    const SAID: &str = "wholeheard-summary: CANARY (os error 2)";
    for language in BOTH {
        let mut notices = vec![
            words::trouble(&Trouble::NoRecorder(SAID.into()), language),
            words::trouble(&Trouble::Shortcut(SAID.into()), language),
            words::trouble(&Trouble::NoTranscriber(SAID.into()), language),
            words::made(&Made::Failed(SAID.into()), language),
            words::minutes(&Minutes::Failed(None), language),
            words::minutes(&Minutes::RedoFailed(None), language),
            words::fetched(&Err(SAID.into()), true, language),
        ];
        for said in [
            Said::Failed(SAID.into()),
            Said::Unavailable(Source::Me, SAID.into()),
            Said::Unavailable(Source::Others, SAID.into()),
        ] {
            notices.extend(words::of(&said, false, language));
        }
        for Notice { title, body } in notices {
            assert!(
                !title.contains("CANARY") && !body.contains("CANARY") && !body.contains('('),
                "{title} — {body}"
            );
        }
    }
}

#[test]
fn the_menu_and_the_shells_own_troubles_are_said_in_both_languages() {
    use words::Trouble;
    let (english, french) = (words::menu(Language::English), words::menu(Language::French));
    assert_eq!(
        (english.start, english.stop, english.quit),
        ("Start recording", "Stop recording", "Quit Wholeheard")
    );
    assert_eq!(
        (french.start, french.stop),
        ("Démarrer un enregistrement", "Arrêter l’enregistrement")
    );
    // No entry is the same in the two languages, and none is empty.
    for (one, other) in [
        (english.start, french.start),
        (english.stop, french.stop),
        (english.stopping, french.stopping),
        (english.quit, french.quit),
        (english.recording, french.recording),
        (english.transcribe, french.transcribe),
        (english.transcribing, french.transcribing),
    ] {
        assert!(
            !one.is_empty() && !other.is_empty() && one != other && !one.contains('\'') && !other.contains('\''),
            "{one} / {other}"
        );
    }
    // A trouble says what it is and what still works. Where the user has something to put
    // right — the settings file's line, the place the model goes —, it says where (D29).
    for language in BOTH {
        for trouble in [
            Trouble::NoRecorder(String::new()),
            Trouble::Settings("settings.toml, line 3: invalid boolean".into()),
            Trouble::Shortcut(String::new()),
            Trouble::NoModel("<data>/wholeheard/models/ggml.bin".into()),
            Trouble::NoTranscriber(String::new()),
        ] {
            let Notice { title, body } = words::trouble(&trouble, language);
            assert!(
                !title.is_empty() && !title.contains('\'') && !body.contains('('),
                "{title} — {body}"
            );
        }
        let settings = words::trouble(&Trouble::Settings("settings.toml, line 3: invalid boolean".into()), language).body;
        assert!(settings.contains("settings.toml, line 3: invalid boolean"), "{settings}");
        let model = words::trouble(&Trouble::NoModel("<data>/wholeheard/models/ggml.bin".into()), language).body;
        assert!(model.contains("<data>/wholeheard/models/ggml.bin"), "{model}");
    }
    // Each says what still works, or what to do — not the trouble a second time.
    let body = |trouble, language| words::trouble(&trouble, language).body;
    assert!(body(Trouble::NoRecorder(String::new()), Language::English).ends_with("Installing Wholeheard again puts it back."));
    assert!(body(Trouble::NoRecorder(String::new()), Language::French).ends_with("Réinstaller Wholeheard le remet en place."));
    assert!(body(Trouble::Settings(String::new()), Language::English).contains("runs with its defaults"));
    assert!(body(Trouble::Shortcut(String::new()), Language::French).contains("permet toujours de démarrer"));
    // What the settings file holds wrong comes out on one line.
    assert!(body(Trouble::Settings("line one\n\tline two".into()), Language::English).contains("line one line two"));
}

#[test]
fn a_transcript_is_said_ready_or_not_with_how_to_have_it_again() {
    use wholeheard_shell::{
        transcription::Made,
        words::{Minutes, Trouble},
    };
    let ready = |language| words::made(&Made::Written("/s/2026-10-16_09-30".into()), language);
    assert_eq!(ready(Language::English).title, "Transcript ready");
    // D44: on Linux a click opens the meeting in Wholeheard, and says so; elsewhere no click is
    // promised — the notification is said, and the window holds the meeting.
    if cfg!(target_os = "linux") {
        assert_eq!(ready(Language::French).body, "Cliquez pour l’ouvrir.");
        assert_eq!(ready(Language::English).body, "Click to open it.");
        assert_eq!(words::minutes(&Minutes::Ready, Language::French).body, "Cliquez pour l’ouvrir.");
    } else {
        assert!(!ready(Language::French).body.contains("Cliquez") && !ready(Language::English).body.contains("Click"));
        assert!(!words::minutes(&Minutes::Ready, Language::English).body.contains("Click"));
    }
    assert!(!ready(Language::French).body.contains("dossier"));
    // Failed: the recording untouched, the menu's entry — never what the program said.
    let failed = words::made(
        &Made::Failed("wholeheard-transcription: the model cannot be loaded".into()),
        Language::English,
    );
    assert!(
        failed.body.contains("untouched") && failed.body.contains("Transcribe the last recording"),
        "{}",
        failed.body
    );
    assert!(!failed.body.contains("cannot be loaded"), "{}", failed.body);
    let french = words::made(&Made::Failed(String::new()), Language::French);
    assert!(french.body.contains("Transcrire le dernier enregistrement") && !french.body.contains('('));
    // The entry named in the sentences is the menu's own.
    for language in BOTH {
        let entry = words::menu(language).transcribe;
        assert!(words::made(&Made::Failed(String::new()), language).body.contains(entry), "{entry}");
        assert!(
            words::trouble(&Trouble::NoModel(String::new()), language).body.contains(entry),
            "{entry}"
        );
        let none = words::trouble(&Trouble::NoSession, language);
        assert!(!none.title.is_empty() && !none.body.contains('('), "{}", none.body);
    }
    assert_ne!(words::follows(Language::English), words::follows(Language::French));
}

#[test]
fn the_model_is_offered_with_its_size_and_host_and_its_fetch_is_said() {
    for language in BOTH {
        let entry = words::download(language, "874 MB", "huggingface.co");
        assert!(entry.contains("874 MB") && entry.contains("huggingface.co"), "{entry}");
        // The notice names the very entry to choose, and keeps the recording.
        let needed = words::model_needed(language, "874 MB", "huggingface.co");
        assert!(needed.body.contains(&entry), "{}", needed.body);
        let kept = if language == Language::French {
            "L’enregistrement est conservé"
        } else {
            "The recording is kept"
        };
        assert!(needed.body.contains(kept), "{}", needed.body);
        assert!(words::downloading(language, 37).contains("37"));
        // While it comes, its share; every byte there, the check — never "100 %" for a while.
        assert_eq!(words::fetching(language, 0, 874), words::downloading(language, 0));
        assert_eq!(words::fetching(language, 437, 874), words::downloading(language, 50));
        assert_eq!(words::fetching(language, 873, 874), words::downloading(language, 99));
        let checking = words::fetching(language, 874, 874);
        assert!(
            !checking.contains('%') && checking != words::downloading(language, 100),
            "{checking}"
        );
        let refused = words::fetched(&Err("what came is not the published model".into()), true, language);
        assert!(!refused.body.contains("published model"), "{}", refused.body);
        assert_ne!(words::fetched(&Ok(()), true, language).title, refused.title);
        // Another than the one chosen: no transcript said begun, the choice left to the user (D37).
        assert_ne!(
            words::fetched(&Ok(()), false, language).body,
            words::fetched(&Ok(()), true, language).body
        );
    }
    assert_eq!(words::fetched(&Ok(()), true, Language::French).title, "Modèle prêt");
}

#[test]
fn audio_sent_to_the_trash_is_said_with_its_counts_and_a_refusal_never_claims_too_much() {
    let said = |language, outcome: Result<(usize, u32), Refusal>| {
        let Notice { title, body } = words::audio_trashed(language, &outcome);
        format!("{title} | {body}")
    };
    assert_eq!(
        said(Language::English, Ok((1, 1))),
        "Audio moved to the trash | The audio of 1 meeting kept more than 1 day is in the trash; \
         its transcript and minutes stay."
    );
    assert_eq!(
        said(Language::English, Ok((3, 30))),
        "Audio moved to the trash | The audio of 3 meetings kept more than 30 days is in the trash; \
         their transcripts and minutes stay."
    );
    assert_eq!(
        said(Language::French, Ok((1, 1))),
        "Audio mis à la corbeille | L’audio d’une réunion gardé plus d’un jour est dans la corbeille\u{202f}; \
         sa transcription et son compte rendu restent."
    );
    assert_eq!(
        said(Language::French, Ok((3, 30))),
        "Audio mis à la corbeille | L’audio de 3 réunions gardé plus de 30 jours est dans la \
         corbeille\u{202f}; leurs transcriptions et leurs comptes rendus restent."
    );
    // The trash may have taken some before it refused: what is said holds either way, in the
    // user's language, without the system's own wording.
    assert_eq!(
        said(Language::English, Err(Refusal::NotAllowed)),
        "The audio was not all moved to the trash | The trash refused access. What was not moved \
         stays where it was; nothing was deleted."
    );
    assert_eq!(
        said(Language::French, Err(Refusal::NoSpace)),
        "L’audio n’a pas été entièrement mis à la corbeille | La corbeille n’a plus de place. Ce \
         qui n’a pas été déplacé reste à sa place\u{202f}; rien n’a été supprimé."
    );
    assert_eq!(
        said(Language::French, Err(Refusal::Other)),
        "L’audio n’a pas été entièrement mis à la corbeille | La corbeille du système ne l’a pas \
         pris. Ce qui n’a pas été déplacé reste à sa place\u{202f}; rien n’a été supprimé."
    );
    assert!(said(Language::English, Err(Refusal::NoSpace)).contains("The trash has no room left."));
    assert!(said(Language::English, Err(Refusal::Other)).contains("The system's trash did not take it."));
    assert!(said(Language::French, Err(Refusal::NotAllowed)).contains("La corbeille a refusé l’accès."));
}

#[test]
fn the_minutes_are_said_ready_waiting_or_not_made_with_what_to_do() {
    use wholeheard_shell::words::Minutes;
    for language in BOTH {
        let entry = words::menu(language).minutes;
        let ready = words::minutes(&Minutes::Ready, language);
        assert!(!ready.title.is_empty());
        // Failed: the transcript untouched, the entry that makes them again — never the program's sentence.
        let failed = words::minutes(&Minutes::Failed(None), language);
        assert!(
            failed.body.contains(entry) && !failed.body.contains("refused the key"),
            "{}",
            failed.body
        );
        // Waiting: nothing sent to that host before the user agrees, in the window.
        let waiting = words::minutes(&Minutes::Waiting("openrouter.ai".into()), language);
        assert!(waiting.body.contains("openrouter.ai"), "{}", waiting.body);
        for other in [Minutes::Unchosen, Minutes::NoTranscript] {
            let said = words::minutes(&other, language);
            assert!(
                !said.title.is_empty() && !said.body.is_empty() && !said.body.contains('('),
                "{}",
                said.body
            );
        }
        assert_ne!(words::menu(language).minuting, entry);
    }
    assert_eq!(words::minutes(&Minutes::Ready, Language::French).title, "Compte rendu prêt");
    // The minutes replaced, kept beside when the trash refused them: said, with where.
    for language in BOTH {
        assert!(
            words::minutes(&Minutes::ReadyKeptBeside("minutes-before-1760751234.md".into()), language)
                .body
                .contains("minutes-before-1760751234.md")
        );
        assert!(
            words::transcript_kept_beside(language, "transcript-before-1760751234.jsonl", false)
                .body
                .contains("transcript-before-1760751234.jsonl")
        );
        // A transcript made again that failed, and the one there not put back: said as such,
        // never as a trash that refused.
        let back = words::transcript_kept_beside(language, "transcript-before-1760751234.jsonl", true);
        let trashed = words::transcript_kept_beside(language, "transcript-before-1760751234.jsonl", false);
        assert!(
            back.body.contains("transcript-before-1760751234.jsonl") && back.title != trashed.title && back.body != trashed.body,
            "{}",
            back.body
        );
        // Minutes made again that failed: the ones there stayed, and the menu's entry is not offered.
        let redo = words::minutes(&Minutes::failed(None, true), language);
        assert_eq!(
            words::minutes(&Minutes::failed(None, false), language).title,
            words::minutes(&Minutes::Failed(None), language).title
        );
        let failed = words::minutes(&Minutes::Failed(None), language);
        assert!(
            redo.title != failed.title && !redo.body.contains(words::menu(language).minutes) && !redo.body.contains("not minutes"),
            "{}",
            redo.body
        );
    }
    assert_eq!(
        words::minutes(&Minutes::RedoFailed(None), Language::French).title,
        "Le compte rendu n’a pas été refait"
    );
    assert_eq!(
        words::minutes(&Minutes::Waiting("openrouter.ai".into()), Language::French).body,
        "Rien n’est envoyé à openrouter.ai avant votre accord, dans la fenêtre de Wholeheard."
    );
}

#[test]
fn a_failure_the_user_can_put_right_says_where_and_the_gesture() {
    use wholeheard_shell::words::{Minutes, Trouble};
    for language in BOTH {
        // The meetings' folder cannot be made: the settings choose another, reinstalling does not.
        let folder = words::trouble(&Trouble::NoFolder("/media/gone/Wholeheard".into()), language).body;
        assert!(
            folder.contains("/media/gone/Wholeheard") && !folder.contains("nstall") && !folder.contains("éinstall"),
            "{folder}"
        );
        // Who writes the minutes is no provider's address: the settings, not a try again.
        let provider = words::minutes(&Minutes::NoProvider("ollama:11434".into()), language).body;
        assert!(
            provider.contains("ollama:11434") && !provider.contains(words::menu(language).minutes),
            "{provider}"
        );
    }
}

#[test]
fn the_page_has_what_a_program_said_only_in_developer_mode() {
    assert_eq!(words::shown_detail("wholeheard-summary: the key was refused", false), "");
    assert_eq!(
        words::shown_detail("wholeheard-summary: the key was refused", true),
        "wholeheard-summary: the key was refused"
    );
}

#[test]
fn minutes_not_made_say_their_cause_and_its_gesture_when_the_program_named_one() {
    use wholeheard_shell::{summarising::Cause, words::Minutes};
    const ALL: [Cause; 16] = [
        Cause::TooMany,
        Cause::Down,
        Cause::NoTimes,
        Cause::Unreachable,
        Cause::TooSlow,
        Cause::RefusedKey,
        Cause::NoCredit,
        Cause::UnknownModel,
        Cause::Redirected,
        Cause::ProviderFailed,
        Cause::CutShort,
        Cause::NotMinutes,
        Cause::TranscriptUnusable,
        Cause::NoSpeech,
        Cause::Settings,
        Cause::Unwritable,
    ];
    // Where the gesture is a setting, the sentence says the settings, not the menu's entry.
    const SETTINGS: [Cause; 8] = [
        Cause::RefusedKey,
        Cause::UnknownModel,
        Cause::Redirected,
        Cause::Settings,
        Cause::CutShort,
        Cause::TooSlow,
        Cause::NotMinutes,
        Cause::NoTimes,
    ];
    // What each sentence is about, in each language: a sentence said for another cause fails.
    let about = |cause: Cause| match cause {
        Cause::Unreachable => ("could not be reached", "pas pu être joint"),
        Cause::TooSlow => ("in time", "à temps"),
        Cause::RefusedKey => ("refused the key", "refusé la clé"),
        Cause::NoCredit => ("credit", "crédit"),
        Cause::UnknownModel => ("does not know", "ne connaît pas"),
        Cause::Redirected => ("another address", "autre adresse"),
        Cause::ProviderFailed => ("failed to answer", "pas su répondre"),
        Cause::CutShort => ("before the end", "avant la fin"),
        Cause::NotMinutes => ("did not write minutes", "pas écrit de compte rendu"),
        Cause::TranscriptUnusable => ("cannot be read", "ne peut pas être lu"),
        Cause::NoSpeech => ("no speech", "aucune parole"),
        Cause::Settings => ("cannot be used as given", "tels quels"),
        Cause::Unwritable => ("could not be written", "pas pu être écrit"),
        Cause::TooMany => ("too many requests", "trop de demandes"),
        Cause::Down => ("it is failing", "en panne"),
        Cause::NoTimes => ("no times", "ne donne pas les temps"),
    };
    for language in BOTH {
        let mut said = std::collections::HashSet::new();
        for cause in ALL {
            let sentence = words::cause(cause, language);
            assert!(
                !sentence.is_empty() && !sentence.contains('\'') && said.insert(sentence),
                "{cause:?}: {sentence}"
            );
            let (english, french) = about(cause);
            let topic = if language == Language::French { french } else { english };
            assert!(sentence.contains(topic), "{cause:?}: {sentence}");
            if language == Language::French {
                // Every colon after a space that does not break, as French is printed.
                assert!(
                    sentence.match_indices(':').all(|(at, _)| sentence[..at].ends_with('\u{a0}')),
                    "{sentence}"
                );
            }
            for failed in [Minutes::failed(Some(cause), false), Minutes::failed(Some(cause), true)] {
                let body = words::minutes(&failed, language).body;
                assert!(body.contains(sentence), "{cause:?}: {body}");
                if SETTINGS.contains(&cause) {
                    let settings = if language == Language::French { "réglages" } else { "settings" };
                    assert!(
                        body.contains(settings) && !body.contains(words::menu(language).minutes),
                        "{cause:?}: {body}"
                    );
                }
            }
        }
        // No cause named, or one this version does not know: the step's sentence, as before.
        assert!(
            words::minutes(&Minutes::Failed(None), language)
                .body
                .contains(words::menu(language).minutes)
        );
    }
}

#[test]
fn the_minutes_sections_are_named_as_the_contract_and_d11_name_them() {
    // minutes/v1's own names in English; D11's second board's in French.
    assert_eq!(
        words::window(Language::English).minutes_sections,
        ["Highlights", "Actions", "Key points"]
    );
    assert_eq!(
        words::window(Language::French).minutes_sections,
        ["Points saillants", "Actions", "Points clés"]
    );
}

#[test]
fn a_device_changed_says_what_is_recorded_now_one_gone_says_that_side_and_one_back_says_so() {
    // D31, each sentence as the user reads it.
    use Language::{English, French};
    let mic = Some("Headset Mic".to_string());
    for (said, language, title, body) in [
        (
            Said::Moved(Source::Me, mic.clone()),
            English,
            "Microphone changed",
            "Now recorded: Headset Mic.",
        ),
        (
            Said::Moved(Source::Me, mic.clone()),
            French,
            "Micro changé",
            "Enregistré maintenant\u{a0}: Headset Mic.",
        ),
        (
            Said::Moved(Source::Others, Some("Speaker".into())),
            English,
            "Sound output changed",
            "The meeting’s sound is now recorded from Speaker.",
        ),
        (
            Said::Moved(Source::Others, Some("Speaker".into())),
            French,
            "Sortie audio changée",
            "Le son de la réunion est maintenant enregistré depuis Speaker.",
        ),
        (
            Said::Moved(Source::Me, None),
            English,
            "Microphone changed",
            "Your microphone is still recorded, from the one your computer uses now.",
        ),
        (
            Said::Moved(Source::Me, None),
            French,
            "Micro changé",
            "Votre micro est toujours enregistré, depuis celui qu’utilise maintenant votre ordinateur.",
        ),
        (
            Said::Moved(Source::Others, None),
            English,
            "Sound output changed",
            "The meeting’s sound is still recorded, from the output your computer uses now.",
        ),
        (
            Said::Moved(Source::Others, None),
            French,
            "Sortie audio changée",
            "Le son de la réunion est toujours enregistré, depuis la sortie qu’utilise maintenant votre ordinateur.",
        ),
        // None left: that side is not recorded for now — nothing said of the other, which may
        // have none either. The microphone: never why, a call may simply have let go (D66).
        (
            Said::Lost(Source::Me),
            English,
            "Your microphone is not recorded for now",
            "The recording goes on.",
        ),
        (
            Said::Lost(Source::Me),
            French,
            "Votre micro n’est pas enregistré pour l’instant",
            "L’enregistrement continue.",
        ),
        (
            Said::Lost(Source::Others),
            English,
            "The meeting’s sound is not recorded for now",
            "Your computer has no sound output left. An output plugged in is recorded at once; the recording goes on.",
        ),
        (
            Said::Lost(Source::Others),
            French,
            "Le son de la réunion n’est plus enregistré pour l’instant",
            "Votre ordinateur n’a plus de sortie audio. Une sortie branchée est enregistrée aussitôt\u{202f}; l’enregistrement continue.",
        ),
    ] {
        assert_eq!(
            told(&said, false, language),
            (title.to_string(), body.to_string()),
            "{said:?} {language:?}"
        );
    }
    // The meeting's sound back after none was left: recorded again, and said so.
    for (name, language, title, body) in [
        (Some("Speaker"), English, "The meeting’s sound is recorded again", "From Speaker."),
        (
            Some("Speaker"),
            French,
            "Le son de la réunion est de nouveau enregistré",
            "Depuis Speaker.",
        ),
        (
            None,
            French,
            "Le son de la réunion est de nouveau enregistré",
            "Depuis la sortie qu’utilise maintenant votre ordinateur.",
        ),
    ] {
        let Notice { title: got, body: said } = words::back(name, language);
        assert_eq!((got.as_str(), said.as_str()), (title, body), "{language:?}");
    }
}

#[test]
fn what_the_devices_are_decides_what_is_said_of_one_gone_and_one_back() {
    use Language::{English, French};
    use words::Sides;
    let (lost, back) = (Said::Lost(Source::Others), Said::Moved(Source::Others, Some("Casque".into())));
    let body = |sides: &mut Sides, said: &Said, language| sides.told(said, false, language).map(|notice| notice.body);

    // The microphone still recorded: the output gone says so — the criterion's "the recording
    // goes on with the other".
    let mut sides = Sides::default();
    assert_eq!(
        body(&mut sides, &lost, English).as_deref(),
        Some("Your computer has no sound output left. Your microphone is still recorded, and an output plugged in is recorded at once.")
    );
    // Back: recorded again; then a move like any other.
    assert_eq!(
        sides.told(&back, false, English).unwrap().title,
        "The meeting’s sound is recorded again"
    );
    assert_eq!(sides.told(&back, false, English).unwrap().title, "Sound output changed");

    // Both gone: neither is said still recorded.
    let mut sides = Sides::default();
    sides.told(&Said::Lost(Source::Me), false, French);
    assert_eq!(
        body(&mut sides, &lost, French).as_deref(),
        Some("Votre ordinateur n’a plus de sortie audio. Une sortie branchée est enregistrée aussitôt\u{202f}; l’enregistrement continue.")
    );
    // The microphone not opened at the start: the same.
    let mut sides = Sides::default();
    sides.told(&Said::Unavailable(Source::Me, String::new()), false, English);
    assert_eq!(
        body(&mut sides, &lost, English).as_deref(),
        Some("Your computer has no sound output left. An output plugged in is recorded at once; the recording goes on.")
    );
    // And the other way round, in French, the meeting's sound still recorded.
    let mut sides = Sides::default();
    assert_eq!(
        body(&mut sides, &Said::Lost(Source::Me), French).as_deref(),
        Some("Le son de la réunion est toujours enregistré.")
    );
}

#[test]
fn a_microphone_not_recorded_says_nothing_of_why_and_no_gesture() {
    use Language::{English, French};
    use words::Sides;
    for (said, language, title, body) in [
        (
            Said::Unavailable(Source::Me, "the call holds it".into()),
            English,
            "Your microphone is not recorded for now",
            "The meeting’s sound is still recorded.",
        ),
        (
            Said::Unavailable(Source::Me, "the call holds it".into()),
            French,
            "Votre micro n’est pas enregistré pour l’instant",
            "Le son de la réunion est toujours enregistré.",
        ),
        (
            Said::Lost(Source::Me),
            English,
            "Your microphone is not recorded for now",
            "The meeting’s sound is still recorded.",
        ),
        (
            Said::Lost(Source::Me),
            French,
            "Votre micro n’est pas enregistré pour l’instant",
            "Le son de la réunion est toujours enregistré.",
        ),
    ] {
        let notice = Sides::default().told(&said, false, language).unwrap();
        assert_eq!((notice.title.as_str(), notice.body.as_str()), (title, body), "{said:?}");
    }
}

#[test]
fn a_microphone_going_is_said_once_a_recording_and_its_return_never() {
    use Language::{English, French};
    use words::Sides;
    let (lost, taken) = (Said::Lost(Source::Me), Said::Moved(Source::Me, Some("AirPods".into())));
    let waiting = Said::Unavailable(Source::Me, String::new());
    let title = |sides: &mut Sides, said: &Said| sides.told(said, false, French).map(|notice| notice.title);
    let not_recorded = Some("Votre micro n’est pas enregistré pour l’instant");

    // It waited from the start: said once, however often a call comes.
    let mut sides = Sides::default();
    assert_eq!(title(&mut sides, &waiting).as_deref(), not_recorded);
    assert_eq!(title(&mut sides, &taken), None);
    assert_eq!(title(&mut sides, &lost), None);
    assert_eq!(title(&mut sides, &taken), None);
    assert_eq!(title(&mut sides, &lost), None);

    // In the contract's order, `started` after what waits: still said once.
    let mut sides = Sides::default();
    assert_eq!(title(&mut sides, &waiting).as_deref(), not_recorded);
    sides.told(&Said::Started("/s".into(), None, None), false, French);
    assert_eq!(title(&mut sides, &taken), None);
    assert_eq!(title(&mut sides, &lost), None);

    // It was recorded: the same sentence, in both languages, once.
    let mut sides = Sides::default();
    sides.told(&Said::Started("/s".into(), Some("AirPods".into()), None), false, French);
    assert_eq!(title(&mut sides, &lost).as_deref(), not_recorded);
    assert_eq!(title(&mut sides, &taken), None);
    assert_eq!(title(&mut sides, &lost), None);
    let notice = Sides::default().told(&lost, false, English).unwrap();
    assert_eq!(notice.title, "Your microphone is not recorded for now");
    assert_eq!(notice.body, "The meeting’s sound is still recorded.");

    // Back, it is recorded: a microphone that sends nothing is told again.
    assert_eq!(title(&mut sides, &taken), None);
    assert_eq!(title(&mut sides, &Said::Nothing).as_deref(), Some("Votre micro ne transmet rien"));

    // Neither side left: the meeting's sound is not said recorded, however the two went.
    for (others, me) in [
        (Said::Lost(Source::Others), Said::Lost(Source::Me)),
        (Said::Unavailable(Source::Others, String::new()), waiting.clone()),
    ] {
        let mut sides = Sides::default();
        sides.told(&others, false, French);
        let notice = sides.told(&me, false, French).unwrap();
        assert_eq!(notice.title, "Votre micro n’est pas enregistré pour l’instant");
        assert_eq!(notice.body, "L’enregistrement continue.");
    }

    // The meeting's side is not a call's to take: each said, as before (D31).
    let (gone, back) = (Said::Lost(Source::Others), Said::Moved(Source::Others, Some("Casque".into())));
    let mut sides = Sides::default();
    for _ in 0..2 {
        assert!(title(&mut sides, &gone).is_some());
        assert!(title(&mut sides, &back).is_some());
    }
}

#[test]
fn a_microphone_is_said_changed_only_from_the_last_name_said() {
    use Language::French;
    use words::Sides;
    let (a, b) = (
        Said::Moved(Source::Me, Some("Micro du portable".into())),
        Said::Moved(Source::Me, Some("AirPods".into())),
    );
    let (lost, waiting) = (Said::Lost(Source::Me), Said::Unavailable(Source::Me, String::new()));
    let said = |sides: &mut Sides, said: &Said| sides.told(said, false, French).map(|notice| notice.body);
    let named = |name: &str| Some(format!("Enregistré maintenant\u{a0}: {name}."));
    let from_a = || {
        let mut sides = Sides::default();
        sides.told(&Said::Started("/s".into(), Some("Micro du portable".into()), None), false, French);
        sides
    };

    // A, B, A: each said, so the last word stays the true one.
    let mut sides = from_a();
    assert_eq!(said(&mut sides, &a), None);
    assert_eq!(said(&mut sides, &b), named("AirPods"));
    assert_eq!(said(&mut sides, &b), None);
    assert_eq!(said(&mut sides, &a), named("Micro du portable"));

    // Back after a gap with no other between: silent. Another: said, and it is the last now.
    let mut sides = from_a();
    sides.told(&lost, false, French);
    assert_eq!(said(&mut sides, &a), None);
    sides.told(&lost, false, French);
    assert_eq!(said(&mut sides, &b), named("AirPods"));
    sides.told(&lost, false, French);
    assert_eq!(said(&mut sides, &b), None);
    assert_eq!(said(&mut sides, &a), named("Micro du portable"));

    // A call's first take is silent, and is the last name; only then does a change count.
    let mut sides = Sides::default();
    sides.told(&waiting, false, French);
    sides.told(&Said::Started("/s".into(), None, None), false, French);
    assert_eq!(said(&mut sides, &b), None);
    assert_eq!(said(&mut sides, &b), None);
    assert_eq!(said(&mut sides, &a), named("Micro du portable"));
    sides.told(&lost, false, French);
    assert_eq!(said(&mut sides, &a), None);

    // Gone before the first take: still the first take.
    let mut sides = Sides::default();
    sides.told(&waiting, false, French);
    sides.told(&lost, false, French);
    assert_eq!(said(&mut sides, &b), None);
    sides.told(&lost, false, French);
    assert_eq!(said(&mut sides, &b), None);
    assert_eq!(said(&mut sides, &a), named("Micro du portable"));
}

#[test]
fn the_first_run_s_french_is_typeset_as_french_and_both_languages_say_it() {
    let french = words::window(Language::French);
    let english = words::window(Language::English);
    let said = [
        french.welcome_question,
        french.welcome_writer,
        french.welcome_here[1],
        french.welcome_connected[1],
        french.welcome_ollama_silent,
        french.welcome_ollama_lacks,
        french.welcome_again[1],
    ];
    for sentence in said {
        // A space before ? ! ; is a narrow one, before : a no-break one — never a plain space.
        for mark in ['?', '!', ';', ':'] {
            assert!(!sentence.contains(&format!(" {mark}")), "{sentence}");
        }
        assert!(!sentence.contains('\''), "{sentence}");
    }
    assert!(french.welcome_question.ends_with("confidentiels\u{202f}?"));
    assert!(french.welcome_ollama_silent.contains("ordinateur\u{a0}:") && french.welcome_ollama_silent.contains("ollama.com"));
    assert_eq!(french.welcome_private[0], "Mode privé");
    assert_eq!(english.welcome_private[0], "Private mode");
    assert_eq!(english.welcome_connected[0], "Connected mode");
    assert!(english.welcome_ollama_silent.contains("ollama.com"));
}

#[test]
fn the_context_s_words_are_said_in_both_languages_french_typeset_as_french() {
    let french = words::window(Language::French);
    let english = words::window(Language::English);
    // The field's name, as D59's criterion says it.
    assert_eq!(french.context_label, "De quoi parle la réunion\u{202f}? Qui y participe\u{202f}?");
    assert_eq!(english.context_label, "What is the meeting about? Who takes part?");
    assert_eq!(french.context_redo, "Refaire avec ce contexte");
    for sentence in [
        french.context_label,
        french.context_example,
        french.context_why,
        french.context_script,
        french.context_making,
        french.context_unfinished,
        french.context_changed[0],
        french.context_changed[1],
    ] {
        for mark in ['?', '!', ';', ':'] {
            assert!(!sentence.contains(&format!(" {mark}")), "{sentence}");
        }
        assert!(!sentence.contains('\''), "{sentence}");
    }
    // The count's two halves, each with its number.
    for (most, left) in [
        (french.context_most, french.context_left),
        (english.context_most, english.context_left),
    ] {
        assert_eq!(most.matches("{}").count(), 1);
        assert!(left.iter().all(|left| left.matches("{}").count() == 1));
    }
    assert!(french.context_script.contains("Whisper") && english.context_script.contains("Whisper"));
}

#[test]
fn a_confidential_meeting_s_words_are_said_in_both_languages_french_typeset_as_french() {
    let french = words::window(Language::French);
    let english = words::window(Language::English);
    // The switch's name and words, as D60's criterion says them.
    assert_eq!(french.confidential_name, "Réunion confidentielle");
    assert_eq!(french.confidential_says, "Rien ne quitte cet ordinateur");
    assert_eq!(english.confidential_name, "Confidential meeting");
    for sentence in [
        french.confidential_left,
        french.confidential_not_saved,
        french.confidential_ollama_empty,
    ] {
        for mark in ['?', '!', ';', ':'] {
            assert!(!sentence.contains(&format!(" {mark}")), "{sentence}");
        }
        assert!(!sentence.contains('\''), "{sentence}");
    }
    for (said, holes) in [
        (french.confidential_left, 1),
        (english.confidential_left, 1),
        (french.confidential_left_sound, 1),
        (english.confidential_left_sound, 1),
        (french.confidential_left_transcript, 1),
        (english.confidential_left_transcript, 1),
        (french.confidential_fetch, 1),
        (english.confidential_fetch, 1),
    ] {
        assert_eq!(said.matches("{}").count(), holes, "{said}");
    }
    // Waiting for Ollama, said in a notification too, the model it would fetch named.
    for language in [Language::French, Language::English] {
        for silent in [true, false] {
            let said = words::minutes(&words::Minutes::OllamaWaits { silent }, language);
            assert!(said.body.contains("Ollama"), "{}", said.body);
            assert_eq!(
                said.body.contains(wholeheard_shell::providers::RECOMMENDED),
                !silent,
                "{}",
                said.body
            );
        }
    }
    let said = words::minutes(&words::Minutes::OllamaWaits { silent: false }, Language::French);
    assert!(!said.body.contains(" :"), "{}", said.body);
}

#[test]
fn every_french_word_of_the_window_is_typeset_as_french() {
    fn strings(value: &serde_json::Value, found: &mut Vec<String>) {
        match value {
            serde_json::Value::String(text) => found.push(text.clone()),
            serde_json::Value::Array(values) => values.iter().for_each(|value| strings(value, found)),
            serde_json::Value::Object(fields) => fields.values().for_each(|value| strings(value, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    strings(&serde_json::to_value(words::window(Language::French)).unwrap(), &mut found);
    assert!(found.len() > 100, "{}", found.len());
    // Before : and », after «, a space that does not break; before ? ! ;, a narrow one — never a
    // plain space, which a narrow window would break the line on.
    for sentence in &found {
        for mark in ['?', '!', ';', ':', '»'] {
            assert!(!sentence.contains(&format!(" {mark}")), "{sentence}");
        }
        assert!(!sentence.contains("« "), "{sentence}");
    }
}

#[test]
fn the_announcement_is_in_the_meetings_language_when_it_says_french_or_english_else_the_windows() {
    use wholeheard_shell::words::{Language, announcement};
    let french = "J’enregistre cette réunion pour en faire le compte rendu. Dites-le-moi si vous préférez que je ne le fasse pas.";
    let english = "I’m recording this meeting to write up its minutes. Tell me if you’d rather I didn’t.";
    assert_eq!(announcement(Some("fr"), Language::English), french);
    assert_eq!(announcement(Some("en"), Language::French), english);
    assert_eq!(announcement(Some("de"), Language::French), french);
    assert_eq!(announcement(None, Language::English), english);
}

/// The French sentences of `src/words.rs`, read from its structure: each string literal after the
/// word `French` and before the next `English`, as every arm of the file is written. Its escapes
/// (`\u{a0}`, `\u{202f}`, `\"`) are read as the compiler reads them. Also the literals after
/// `English`, for the test to hold that the structure was read right.
fn literals_by_language() -> (Vec<String>, Vec<String>) {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/words.rs")).unwrap();
    let (mut french, mut english) = (Vec::new(), Vec::new());
    let mut in_french = false;
    let mut word = String::new();
    let mut chars = source.chars().peekable();
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            chars.by_ref().find(|&d| d == '\n');
            previous = '\n';
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            previous = c;
            continue;
        }
        match word.as_str() {
            "French" => in_french = true,
            "English" => in_french = false,
            _ => {}
        }
        word.clear();
        if c == '"' && previous != '\'' {
            let mut text = String::new();
            while let Some(d) = chars.next() {
                match d {
                    '\\' => match chars.next() {
                        Some('u') => {
                            let code: String = chars.by_ref().skip(1).take_while(|&e| e != '}').collect();
                            text.push(char::from_u32(u32::from_str_radix(&code, 16).unwrap()).unwrap());
                        }
                        Some('n') => text.push('\n'),
                        Some(other) => text.push(other),
                        None => break,
                    },
                    '"' => break,
                    other => text.push(other),
                }
            }
            if in_french { &mut french } else { &mut english }.push(text);
            previous = '"';
            continue;
        }
        previous = c;
    }
    (french, english)
}

#[test]
fn the_literals_of_each_language_are_read_as_the_file_arranges_them() {
    let (french, english) = literals_by_language();
    assert!(french.len() > 300 && english.len() > 300, "{} {}", french.len(), english.len());
    let accented: Vec<_> = english
        .iter()
        .filter(|text| text.contains(|c: char| "éèêàâçùûîôœÉÈÀÇ".contains(c)))
        .collect();
    // Inès: a name in an English example.
    assert_eq!(
        accented,
        ["E.g. the Q4 budget with Inès Durand and the purchasing team"],
        "French read as English"
    );
    // An `expect`'s words, for the developer: no sentence the user reads.
    let plain: Vec<_> = french.iter().filter(|text| format!(" {text} ").contains(" the ")).collect();
    assert_eq!(plain, ["the minutes' end is always said"], "English read as French");
}

#[test]
fn every_french_sentence_has_its_spaces_before_the_tall_signs() {
    // D47: U+202F before « ? ! ; », U+00A0 before « : » — times (12:21) and addresses (https:)
    // aside — in every French sentence the window and the notifications say.
    let mut wrong = Vec::new();
    for text in literals_by_language().0 {
        // A program's name, as its own messages open: no sentence. What is filled in neither.
        if matches!(text.as_str(), "wholeheard-capture:" | "wholeheard-transcription:") {
            continue;
        }
        let chars: Vec<char> = text.chars().collect();
        let mut filled = false;
        for (at, &c) in chars.iter().enumerate() {
            filled = (filled || c == '{') && c != '}';
            if filled {
                continue;
            }
            let before = if at == 0 { None } else { Some(chars[at - 1]) };
            let bad = match c {
                '?' | '!' | ';' => before != Some('\u{202f}'),
                ':' => {
                    // 12:21: two figures after; https:// : its own word before, two slashes after.
                    let time = before.is_some_and(|b| b.is_ascii_digit())
                        && chars[at + 1..].iter().take_while(|d| d.is_ascii_digit()).count() == 2;
                    let word: String = chars[..at].iter().rev().take_while(|d| d.is_alphabetic()).collect();
                    let address = matches!(word.as_str(), "sptth" | "ptth") && chars[at + 1..].starts_with(&['/', '/']);
                    !time && !address && before != Some('\u{a0}')
                }
                _ => false,
            };
            if bad {
                wrong.push(text.clone());
                break;
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} French sentences without their space:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn a_list_waiting_on_its_key_says_so_and_what_stays_offered() {
    let (english, french) = (words::window(Language::English), words::window(Language::French));
    assert!(english.offered_refresh_waits.contains("waits on the key") && english.offered_refresh_waits.contains("still offered"));
    assert!(french.offered_refresh_waits.contains("attend la clé") && french.offered_refresh_waits.contains("restent proposés"));
}

#[test]
fn the_small_window_says_a_silence_as_the_notifications_do() {
    // D49: the same gesture as the notification's, in each language; the side's own.
    let cases = [
        (
            Language::French,
            "Votre micro",
            "Le son de la réunion",
            "pas coupé sur l’appareil lui-même, et qu’il est prêt",
        ),
        (
            Language::English,
            "Your microphone",
            "The meeting’s sound",
            "not muted on the device itself, and that it is ready",
        ),
    ];
    for (language, me, others, gesture) in cases {
        let page = words::window(language);
        assert_eq!(page.small_mic, me);
        assert_eq!(page.small_meeting, others);
        let told = words::of(&Said::Nothing, true, language).unwrap();
        assert!(told.body.contains(gesture), "{}", told.body);
        assert!(page.small_nothing.contains(gesture), "{}", page.small_nothing);
        for said in [
            page.small_silent_me,
            page.small_silent_others,
            page.small_nothing,
            page.small_waiting_me,
            page.small_waiting_others,
        ] {
            assert!(
                !said.is_empty() && said.chars().count() <= 110,
                "too long for two lines of the small window: {said}"
            );
        }
    }
    assert!(words::window(Language::French).small_silent_me.contains("parlez"));
    assert!(
        words::window(Language::French)
            .small_silent_others
            .contains("jouer un son dans l’appel")
    );
    assert!(words::window(Language::English).small_silent_me.contains("say something"));
    assert!(
        words::window(Language::English)
            .small_silent_others
            .contains("play a sound in the call")
    );
}
/// The wording table of the UX review, §10 (D47): its sentences, as they are read. A row left out
/// here is one the table's sentence would make false: said in the pull request.
#[test]
fn the_review_s_wording_is_what_the_window_says_in_french() {
    let french = words::window(Language::French);
    for (said, wanted) in [
        (french.transcript, "Transcription"),
        (french.transcript_title, "Transcription"),
        (french.chip_transcribing, "Transcription…"),
        (french.ran_here, "sur cet ordinateur"),
        (french.heard_by, "Transcrit par {}, sur cet ordinateur"),
        (french.remote_described, "À distance — le son de l’ordinateur"),
        (french.no_match, "Aucun passage ne correspond."),
        (french.search, "Rechercher dans la transcription"),
        (french.interrupted, "Interrompu"),
        (french.undated, "Date inconnue"),
        (french.chip_awaiting, "Accord requis"),
        (french.minutes_waiting, "En file d’attente · {} avant"),
        (french.minutes_unsaved, "Modification non enregistrée\u{a0}: {} reste utilisé."),
        (french.test_sends, "Le test envoie une courte phrase à {} — rien de vos réunions."),
        (french.test_answered, "Ça marche\u{a0}: le modèle répond."),
        (
            french.developer[1],
            "Affiche le détail technique sous chaque erreur, pour aider au diagnostic.",
        ),
        (french.log_level[1], "S’applique au prochain lancement de Wholeheard."),
        (french.playback, "Position de lecture"),
        (french.not_playable, "Cette version de Wholeheard ne peut pas lire cet audio."),
        (
            french.not_played,
            "Lecture impossible ici. Ouvrez le dossier pour l’écouter avec un autre lecteur.",
        ),
        (
            french.no_player,
            "L’audio a été supprimé pour faire de la place. La transcription et le compte rendu restent.",
        ),
        (french.time_listen, "Écouter depuis {}"),
        (french.time_passage, "Voir le passage de {}"),
        (french.small_no_keys, "Pas de raccourci clavier — en définir un"),
        (french.shortcut_none, "Aucun raccourci défini"),
        (french.shortcut_where, "Il se modifie dans les réglages clavier du système."),
        (
            french.ollama_silent,
            "Ollama ne répond pas ou n’a aucun modèle\u{a0}: lancez Ollama, puis téléchargez un modèle.",
        ),
        (french.key, "Clé d’API"),
        (french.agree_local_write, "Rédiger sur cet ordinateur"),
    ] {
        assert_eq!(said, wanted);
    }
    assert!(
        french.ollama_lacks.ends_with("ou téléchargez-le avec Ollama."),
        "{}",
        french.ollama_lacks
    );
    let menu = words::menu(Language::French);
    assert_eq!(menu.start, french.record_start);
    assert_eq!(menu.minutes, "Rédiger le compte rendu de la dernière réunion");
    assert_eq!(
        words::minutes(&words::Minutes::Unchosen, Language::French).title,
        "Aucun modèle choisi pour les comptes rendus"
    );
}

#[test]
fn the_review_s_english_sentences_are_rewritten() {
    let english = words::window(Language::English);
    assert_eq!(english.interrupted, "Interrupted");
    assert_eq!(english.keep_not_days, "{} is not a number of days: from 1 to 36,500");
}

/// *Transcript* is the English word: French says *Transcription*, feminine, everywhere (D47).
#[test]
fn french_never_says_transcript() {
    use words::Minutes;
    let window = serde_json::to_value(words::window(Language::French)).unwrap();
    let mut strings = vec![window];
    while let Some(value) = strings.pop() {
        match value {
            serde_json::Value::Array(items) => strings.extend(items),
            serde_json::Value::Object(fields) => strings.extend(fields.into_iter().map(|(_, value)| value)),
            serde_json::Value::String(said) => {
                assert!(
                    !said.to_lowercase().contains("transcript") || said.to_lowercase().contains("transcription"),
                    "{said}"
                );
                assert!(!said.contains("Le transcription") && !said.contains("le transcription"), "{said}");
            }
            _ => {}
        }
    }
    for notice in [
        words::minutes(&Minutes::Ready, Language::French),
        words::minutes(&Minutes::NoTranscript, Language::French),
        words::minutes(&Minutes::Failed(None), Language::French),
        words::transcript_kept_beside(Language::French, "x", false),
        words::transcript_kept_beside(Language::French, "x", true),
        words::fetched(&Ok(()), false, Language::French),
        words::audio_trashed(Language::French, &Ok((2, 30))),
    ] {
        let said = format!("{} {}", notice.title, notice.body);
        assert!(
            !said.contains("transcript ") && !said.contains("transcripts") && !said.contains("transcript."),
            "{said}"
        );
    }
}

#[test]
fn a_stop_says_how_long_was_recorded_agreed_in_french_and_no_session_in_a_failure() {
    let stopped = |seconds| told(&Said::Stopped("/s".into(), seconds), false, Language::French).1;
    assert_eq!(stopped(1.0), "1 s enregistrée.");
    assert_eq!(stopped(60.0), "1 min enregistrée.");
    assert_eq!(stopped(960.0), "16 min enregistrées.");
    assert_eq!(stopped(5400.0), "1 h 30 enregistrées.");
    for why in ["", "gone"] {
        let failed = told(&Said::Failed(why.into()), false, Language::French).1;
        assert!(failed.ends_with("est gardé dans le dossier de la réunion."), "{failed}");
    }
}

fn main_rs() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
        .unwrap()
        // A checkout on Windows ends its lines otherwise.
        .replace("\r\n", "\n")
}

/// Each `#[tauri::command…]` of the shell's `main.rs`, whatever its arguments: its signature, then
/// its body.
fn commands() -> Vec<(String, String)> {
    main_rs()
        .split("#[tauri::command")
        .skip(1)
        .map(|command| {
            let command = &command[command.find("]\n").expect("an attribute ends") + 2..];
            let command = &command[..command.find("\n}\n").expect("a command ends")];
            let (signature, body) = command.split_once(" {\n").expect("a command has a body");
            (signature.to_string(), body.to_string())
        })
        .collect()
}

/// The structs of `main.rs` the page is handed: each with its fields, name and type.
fn payloads() -> Vec<(String, Vec<(String, String)>)> {
    let source = main_rs();
    let mut found = Vec::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if !(line.starts_with("#[derive(") && line.contains("Serialize")) {
            continue;
        }
        while lines.peek().is_some_and(|next| next.starts_with("#[")) {
            lines.next();
        }
        let Some(name) = lines.peek().and_then(|next| next.strip_prefix("struct ")) else {
            continue;
        };
        let name = name.split([' ', '<']).next().unwrap().to_string();
        let mut fields = Vec::new();
        for field in lines.by_ref().skip(1).take_while(|field| *field != "}") {
            if let Some((field, kind)) = field.trim().split_once(": ")
                && !field.starts_with("//")
            {
                fields.push((field.to_string(), kind.trim_end_matches(',').to_string()));
            }
        }
        found.push((name, fields));
    }
    found
}

#[test]
fn a_command_hands_the_page_no_text_of_its_own_only_a_sentence_of_these_words() {
    let commands = commands();
    assert!(commands.len() > 30, "{} commands read", commands.len());
    let structs: Vec<String> = payloads().into_iter().map(|(name, _)| name).collect();
    assert!(structs.len() > 10, "{structs:?}");
    for (signature, body) in &commands {
        if let Some((_, answer)) = signature.rsplit_once("-> ") {
            // Every name of its answer is known here: an alias could hide a text.
            for name in answer
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|name| !name.is_empty())
            {
                assert!(
                    ["Result", "Option", "Vec", "Told", "PathBuf", "String", "speech", "Status"].contains(&name)
                        || structs.iter().any(|known| known == name),
                    "{signature}: {name} is no type this test knows"
                );
            }
            // A text alone is none of the page's answers, a list of names and the logs asked for aside.
            assert!(
                !answer.replace("Vec<String>", "").contains("String") || signature.starts_with("async fn diagnostic("),
                "a command answers with a text: {signature}"
            );
            assert!(!answer.contains("Result") || answer.starts_with("Result<"), "{signature}");
            if let Some(answer) = answer.strip_prefix("Result<") {
                let error = answer.rsplit(", ").next().unwrap().trim();
                assert!(
                    error == "Told>" || error == "()>",
                    "a command fails with a sentence of words.rs (Told), or says nothing: {signature}"
                );
            }
        }
        for text in ["Err(\"", "Err(format!", ".into())", "ok_or(\"", ".ok_or_else(|| format!"] {
            assert!(!body.contains(text), "{signature} hands the page a text: {text}");
        }
        // An error kept as an answer's field: read by what `Ok` is made of, as far as a name tells.
        for (at, _) in body.match_indices(".err()") {
            assert!(
                body[at + ".err()".len()..].trim_start().starts_with(".map(|why| shell.told("),
                "{signature} keeps an error for its answer"
            );
        }
    }
}

#[test]
fn an_answer_holds_a_failure_as_a_sentence_or_for_developer_mode_never_as_raw_text() {
    // By the field's name: a raw error under another name, or within another type, is not seen.
    const NAMES: [&str; 8] = ["unreadable", "error", "why", "failed", "refused", "detail", "reason", "problem"];
    // Those of `makings`, the page's reading of the makings: what a program said, through
    // `shown_detail`, and its cause's sentence.
    const THROUGH_THE_WORDS: [&str; 3] = ["failed", "transcripts_failed", "failed_said"];
    let payloads = payloads();
    let mut read = 0;
    for (name, fields) in &payloads {
        for (field, kind) in fields {
            if !NAMES.iter().any(|said| field.contains(said)) {
                continue;
            }
            read += 1;
            assert!(
                !kind.contains("String") || (name == "Makings" && THROUGH_THE_WORDS.contains(&field.as_str())),
                "{name}.{field}: {kind} hands the page raw text"
            );
        }
    }
    assert!(read >= 4, "{read} fields read: the structs are no longer read as written");
    let source = main_rs();
    assert!(source.contains("let said = |why: &String| words::shown_detail(why, developer);"));
    assert_eq!(
        source.matches("(named(folder), said(why))").count(),
        2,
        "failed, transcripts_failed"
    );
}

#[test]
fn the_settings_files_path_stays_out_of_what_a_command_keeps_of_a_failure() {
    let source = main_rs();
    let reread = &source[source.find("fn reread(&self)").unwrap()..];
    let reread = &reread[..reread.find("\n    }\n").unwrap()];
    // Every route starts here, the transcript's and the minutes' that keep it for developer mode.
    assert!(reread.contains("settings::unnamed("), "{reread}");
    // One place logs a command's failure: `log::command_failed`, which `tests/log.rs` reads.
    assert!(!source.contains("\"shell.command_failed\""));
    assert_eq!(source.matches("log::command_failed(").count(), 1);
}

#[test]
fn a_settings_file_that_cannot_be_read_is_said_whole_its_raw_text_under_it_only_in_developer_mode() {
    let raw = "settings.toml, line 1: keep_audio is \"always\" or a number of days";
    let page = |language, developer| serde_json::to_value(words::told(Failure::SettingsLocked, raw, language, developer)).unwrap();
    assert_eq!(
        page(Language::French, false),
        "Le fichier des réglages ne peut pas être lu\u{a0}: Wholeheard ne l’écrit pas tant qu’il n’est pas corrigé."
    );
    assert_eq!(
        page(Language::English, false),
        "The settings file cannot be read: Wholeheard does not write over it until it is corrected."
    );
    assert_eq!(
        page(Language::English, true),
        format!("The settings file cannot be read: Wholeheard does not write over it until it is corrected.\n{raw}").as_str()
    );
}

#[test]
fn a_command_that_failed_says_its_sentence_and_the_raw_text_only_in_developer_mode() {
    let raw = "os error 28";
    let page = |language, developer| serde_json::to_string(&words::told(Failure::MeetingFile, raw, language, developer)).unwrap();
    assert_eq!(
        page(Language::French, false),
        "\"son dossier n’a pas pu être écrit\u{a0}: vérifiez qu’il reste de la place et que vous pouvez y écrire, puis réessayez.\""
    );
    assert!(page(Language::English, false).starts_with("\"its folder could not be written to"));
    assert!(!page(Language::English, false).contains(raw));
    assert!(
        page(Language::French, true).ends_with("réessayez.\\nos error 28\""),
        "{}",
        page(Language::French, true)
    );
    // A trash's refusal is the sentence its expiry says.
    assert_eq!(
        words::failure(Failure::Trash(Refusal::NoSpace), Language::French),
        words::audio_trashed(Language::French, &Err(Refusal::NoSpace)).body
    );
}

#[test]
fn a_test_with_a_cause_says_it_alone_the_raw_text_only_in_developer_mode() {
    use wholeheard_shell::{summarising::Cause, words};
    for (cause, advice) in [
        (Cause::TooMany, "wait, or check your plan with it"),
        (
            Cause::Down,
            "try again later. On this computer, this model may be too large for the memory: choose a smaller one",
        ),
        (
            Cause::NoTimes,
            "choose one that gives each segment’s times in Wholeheard’s settings",
        ),
    ] {
        assert!(words::cause(cause, Language::English).contains(advice), "{cause:?}");
    }
    let plain = serde_json::to_string(&words::test_caused(Cause::Down, "raw", Language::French, false)).unwrap();
    assert!(
        plain.contains("en panne") && !plain.contains("raw") && !plain.contains("ne répond pas"),
        "{plain}"
    );
    let developer = serde_json::to_string(&words::test_caused(Cause::Down, "raw", Language::French, true)).unwrap();
    assert!(developer.contains("raw"), "{developer}");
}
