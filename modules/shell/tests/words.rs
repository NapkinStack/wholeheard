//! What the user is told: the sentences, read here as a user reads them, in both languages.

use std::path::PathBuf;

use wholeheard_shell::{
    recording::{Said, Source},
    words::{self, Language, Notice, Refusal},
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
    let started = Said::Started(PathBuf::from("/s"), None);
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
    let started = Said::Started(PathBuf::from("/s"), Some("Headset Mic".into()));
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
            "Your microphone is not recorded".into(),
            "The meeting’s sound is still recorded.".into()
        )
    );
    assert_eq!(
        told(&refused, false, Language::French).1,
        "Le son de la réunion est toujours enregistré."
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
        Said::Started("/s".into(), None),
        Said::Started("/s".into(), Some("Mic".into())),
        Said::Unavailable(Source::Me, String::new()),
        Said::Unavailable(Source::Others, String::new()),
        Said::Silent(Source::Me),
        Said::Silent(Source::Others),
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
                !body.contains('(') && (body.ends_with("session’s folder.") || body.ends_with("dossier de la session.")),
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
            words::fetched(&Err(SAID.into()), language),
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
        ("Démarrer l’enregistrement", "Arrêter l’enregistrement")
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
    use wholeheard_shell::{transcription::Made, words::Trouble};
    let ready = |language| words::made(&Made::Written("/s/2026-10-16_09-30".into()), language);
    assert_eq!(ready(Language::English).title, "Transcript ready");
    assert!(ready(Language::French).body.contains("ouvrir le dossier"));
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
        let refused = words::fetched(&Err("what came is not the published model".into()), language);
        assert!(!refused.body.contains("published model"), "{}", refused.body);
        assert_ne!(words::fetched(&Ok(()), language).title, refused.title);
    }
    assert_eq!(words::fetched(&Ok(()), Language::French).title, "Modèle prêt");
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
        "Audio mis à la corbeille | L’audio d’une réunion gardé plus d’un jour est dans la corbeille ; \
         son transcript et son compte rendu restent."
    );
    assert_eq!(
        said(Language::French, Ok((3, 30))),
        "Audio mis à la corbeille | L’audio de 3 réunions gardé plus de 30 jours est dans la \
         corbeille ; leurs transcripts et leurs comptes rendus restent."
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
         qui n’a pas été déplacé reste à sa place ; rien n’a été supprimé."
    );
    assert_eq!(
        said(Language::French, Err(Refusal::Other)),
        "L’audio n’a pas été entièrement mis à la corbeille | La corbeille du système ne l’a pas \
         pris. Ce qui n’a pas été déplacé reste à sa place ; rien n’a été supprimé."
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
    const ALL: [Cause; 13] = [
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
    const SETTINGS: [Cause; 7] = [
        Cause::RefusedKey,
        Cause::UnknownModel,
        Cause::Redirected,
        Cause::Settings,
        Cause::CutShort,
        Cause::TooSlow,
        Cause::NotMinutes,
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
        // have none either — and one plugged in is recorded at once.
        (
            Said::Lost(Source::Me),
            English,
            "Your microphone is not recorded for now",
            "Your computer has no microphone left. A microphone plugged in is recorded at once; the recording goes on.",
        ),
        (
            Said::Lost(Source::Me),
            French,
            "Votre micro n’est plus enregistré pour l’instant",
            "Votre ordinateur n’a plus de micro. Un micro branché est enregistré aussitôt\u{a0}; l’enregistrement continue.",
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
            "Votre ordinateur n’a plus de sortie audio. Une sortie branchée est enregistrée aussitôt\u{a0}; l’enregistrement continue.",
        ),
    ] {
        assert_eq!(
            told(&said, false, language),
            (title.to_string(), body.to_string()),
            "{said:?} {language:?}"
        );
    }
    // A device back after none was left: recorded again, and said so.
    for (source, name, language, title, body) in [
        (
            Source::Me,
            mic.as_deref(),
            English,
            "Your microphone is recorded again",
            "Now recorded: Headset Mic.",
        ),
        (
            Source::Me,
            mic.as_deref(),
            French,
            "Votre micro est de nouveau enregistré",
            "Enregistré maintenant\u{a0}: Headset Mic.",
        ),
        (
            Source::Others,
            Some("Speaker"),
            English,
            "The meeting’s sound is recorded again",
            "From Speaker.",
        ),
        (
            Source::Others,
            Some("Speaker"),
            French,
            "Le son de la réunion est de nouveau enregistré",
            "Depuis Speaker.",
        ),
        (
            Source::Me,
            None,
            English,
            "Your microphone is recorded again",
            "From the one your computer uses now.",
        ),
        (
            Source::Others,
            None,
            French,
            "Le son de la réunion est de nouveau enregistré",
            "Depuis la sortie qu’utilise maintenant votre ordinateur.",
        ),
    ] {
        let Notice { title: got, body: said } = words::back(source, name, language);
        assert_eq!((got.as_str(), said.as_str()), (title, body), "{source:?} {language:?}");
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
        Some("Votre ordinateur n’a plus de sortie audio. Une sortie branchée est enregistrée aussitôt\u{a0}; l’enregistrement continue.")
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
        Some(
            "Votre ordinateur n’a plus de micro. Le son de la réunion est toujours enregistré, et un micro branché est enregistré aussitôt."
        )
    );
}
