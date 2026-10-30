//! The settings, read as a user wrote them — or did not.

use std::{fs, path::Path};

use wholeheard_shell::{
    settings::{After, Engine, KeepAudio, Log, Settings, Window, file, read, welcome_due, whisper_models, write},
    words::Language,
};

fn written(text: &str) -> Result<Settings, String> {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, text).unwrap();
    read(&path).map_err(|said| said.replace(&path.display().to_string(), "<file>"))
}

#[test]
fn the_reminder_is_on_unless_the_file_says_otherwise() {
    assert!(Settings::default().reminder);
    assert!(written("after = \"nothing\"\n").unwrap().reminder);
}

#[test]
fn no_file_and_an_empty_one_are_every_default() {
    let defaults = Settings::default();
    // The reminder is shown unless the user says otherwise; sessions go to a folder named Wholeheard.
    assert!(defaults.reminder && defaults.sessions.is_absolute() && defaults.sessions.ends_with("Wholeheard"));
    assert_eq!(read(Path::new("/nowhere/settings.toml")), Ok(defaults.clone()));
    for nothing in ["", "\n\n", "# only a comment\n"] {
        assert_eq!(written(nothing), Ok(defaults.clone()), "{nothing:?}");
    }
    assert!(file().is_some_and(|path| path.ends_with("wholeheard/settings.toml")));
}

#[test]
fn a_file_names_what_it_changes_and_leaves_the_rest() {
    let only = written("reminder = false\n").unwrap();
    assert!(!only.reminder && only.sessions == Settings::default().sessions);
    // A whole path, as each system writes one.
    let elsewhere = if cfg!(windows) { r"C:\srv\meetings" } else { "/srv/meetings" };
    let both = written(&format!("sessions = '{elsewhere}'\nreminder = true\n")).unwrap();
    assert!(both.reminder && both.sessions == Path::new(elsewhere));
    // A key of a later version is left alone: the file is not refused for it.
    assert_eq!(
        written("after_recording = \"minutes\"\n[provider]\nhost = \"x\"\n"),
        Ok(Settings::default())
    );
    // A folder from the user's home, as people write it.
    let home = written(&format!("sessions = \"{}/Réunions/Wholeheard\"\n", '~')).unwrap().sessions;
    assert!(home.is_absolute() && home.ends_with("Réunions/Wholeheard") && !home.to_string_lossy().contains('~'));
}

#[test]
fn a_file_that_is_not_settings_is_said_in_one_sentence_and_not_used() {
    for (text, expected) in [
        ("reminder = \"yes\"\n", "<file>, line 1: "),
        ("# a comment\n\nreminder = maybe\n", "<file>, line 3: "),
        ("sessions = 12\n", "<file>, line 1: "),
        ("this is not TOML\n", "<file>, line 1: "),
        // A fault that opens a line is on that line, not the one before.
        ("reminder = true\n= 3\n", "<file>, line 2: "),
        ("reminder = false\nreminder = true\n", "<file>, line 2: "),
        ("# é\r\nreminder = true\r\n\r\n[sessions]\r\nx = 1\r\n", "<file>, line 4: "),
        // A folder that is not a whole path is said where it is written.
        (
            "reminder = false\nsessions = \"Wholeheard\"\n",
            "<file>, line 2: sessions is the folder's whole path",
        ),
        ("sessions = \"\"\n", "<file>, line 1: sessions is the folder's whole path"),
        ("\n\n\nsessions = \"./here\"", "<file>, line 4: sessions is the folder's whole path"),
    ] {
        let said = written(text).expect_err(text);
        assert!(said.starts_with(expected) && said.lines().count() == 1, "{text:?}: {said}");
    }
    // A file that is there and cannot be read is said too, not taken for no file.
    let folder = tempfile::tempdir().unwrap();
    assert!(read(folder.path()).unwrap_err().contains("cannot be read"));
}

#[test]
fn the_transcript_follows_a_recording_with_the_model_the_settings_name() {
    // Out of the box the transcript is made, with the default model in Wholeheard's data folder.
    let defaults = Settings::default();
    assert!(defaults.after == After::Transcript && defaults.model.is_absolute());
    assert!(
        defaults
            .model
            .ends_with(format!("wholeheard/models/{}", wholeheard_shell::settings::MODEL))
    );
    let chosen = written(&format!("transcript = false\nmodel = \"{}/models/ggml-small.bin\"\n", '~')).unwrap();
    // Written before `after`: `transcript = false` is "nothing follows".
    assert!(chosen.after == After::Nothing && chosen.model.is_absolute() && chosen.model.ends_with("models/ggml-small.bin"));
    let said = written("\nmodel = \"ggml-small.bin\"\n").unwrap_err();
    assert!(said.starts_with("<file>, line 2: model is the file's whole path"), "{said}");
}

#[test]
fn what_follows_a_recording_who_writes_the_minutes_and_how_long_audio_is_kept() {
    let defaults = Settings::default();
    assert_eq!(
        (
            defaults.minutes_provider.clone(),
            defaults.minutes_model.clone(),
            defaults.keep_audio
        ),
        (None, None, KeepAudio::Always)
    );
    let chosen = written(
        "after = \"minutes\"\nminutes_provider = \"https://openrouter.ai/api/v1\"\nminutes_model = \"anthropic/claude-sonnet\"\nkeep_audio = 90\n",
    )
    .unwrap();
    assert_eq!(chosen.after, After::Minutes);
    assert_eq!(chosen.minutes_provider.as_deref(), Some("https://openrouter.ai/api/v1"));
    assert_eq!(chosen.minutes_model.as_deref(), Some("anthropic/claude-sonnet"));
    assert_eq!(chosen.keep_audio, KeepAudio::Days(90));
    // `after` says it when both are there.
    assert_eq!(
        written("transcript = false\nafter = \"transcript\"\n").unwrap().after,
        After::Transcript
    );
    assert_eq!(written("keep_audio = \"always\"\n").unwrap().keep_audio, KeepAudio::Always);
    for (text, expected) in [
        ("after = \"later\"\n", "<file>, line 1: "),
        ("\nkeep_audio = 0\n", "<file>, line 2: keep_audio is \"always\" or a number of days"),
        (
            "keep_audio = \"forever\"\n",
            "<file>, line 1: keep_audio is \"always\" or a number of days",
        ),
    ] {
        let said = written(text).unwrap_err();
        assert!(said.starts_with(expected), "{text:?}: {said}");
    }
}

#[test]
fn the_settings_written_read_back_as_they_were_the_user_s_lines_kept() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("wholeheard").join("settings.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "# my settings\nreminder = false\ntranscript = false\nfuture_key = 3 # kept\n",
    )
    .unwrap();
    let mut settings = read(&path).unwrap();
    settings.after = After::Minutes;
    settings.minutes_provider = Some("ollama".into());
    settings.minutes_model = Some("mistral".into());
    settings.keep_audio = KeepAudio::Days(30);
    write(&path, &settings).unwrap();
    assert_eq!(read(&path), Ok(settings.clone()));
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# my settings") && text.contains("future_key = 3 # kept"), "{text}");
    assert!(!text.contains("transcript = false"), "{text}");
    // A choice taken back is no longer in the file.
    settings.minutes_model = None;
    write(&path, &settings).unwrap();
    assert!(!fs::read_to_string(&path).unwrap().contains("minutes_model"));
    // No file yet: written whole, read back the same.
    let fresh = folder.path().join("new").join("settings.toml");
    write(&fresh, &Settings::default()).unwrap();
    assert_eq!(read(&fresh), Ok(Settings::default()));
    assert!(!fresh.with_extension("toml.part").exists());
}

#[test]
fn writing_touches_only_what_changed_and_keeps_each_line_s_comment() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    let mine = format!(
        "sessions = \"{}/Wholeheard\" # where they go\nreminder = true # said at each start\n",
        '~'
    );
    fs::write(&path, &mine).unwrap();
    let mut settings = read(&path).unwrap();
    // Nothing changed: the file is as the user wrote it — the tilde, the comments, no key added.
    write(&path, &settings).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), mine);
    // One choice changed: its line keeps its comment, the rest is untouched.
    settings.reminder = false;
    write(&path, &settings).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("reminder = false # said at each start"), "{text}");
    assert!(
        text.contains(&format!("sessions = \"{}/Wholeheard\" # where they go", '~')),
        "{text}"
    );
    assert!(!text.contains("model") && !text.contains("keep_audio"), "{text}");
}

#[test]
fn a_file_that_cannot_be_read_as_settings_is_never_written_over() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    for refused in ["keep_audio = 0\n", "reminder = \"yes\"\n", "sessions = \"Wholeheard\"\n"] {
        fs::write(&path, refused).unwrap();
        assert!(write(&path, &Settings::default()).is_err(), "{refused}");
        assert_eq!(fs::read_to_string(&path).unwrap(), refused);
        assert!(!path.with_extension("toml.part").exists());
    }
}

#[cfg(unix)]
#[test]
fn the_file_keeps_its_permissions_and_a_link_to_it_stays_a_link() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let folder = tempfile::tempdir().unwrap();
    let real = folder.path().join("real.toml");
    fs::write(&real, "reminder = true\n").unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    let link = folder.path().join("settings.toml");
    symlink(&real, &link).unwrap();
    let mut settings = read(&link).unwrap();
    settings.reminder = false;
    write(&link, &settings).unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert!(fs::read_to_string(&real).unwrap().contains("reminder = false"));
    assert_eq!(fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o600);
}

#[test]
fn values_that_are_not_choices_are_refused_and_blanks_are_none() {
    for (text, key) in [
        ("keep_audio = 36501\n", "keep_audio"),
        ("keep_audio = 30.0\n", "keep_audio"),
        ("keep_audio = \"Always\"\n", "keep_audio"),
        ("keep_audio = -1\n", "keep_audio"),
    ] {
        let said = written(text).unwrap_err();
        assert!(said.contains(key), "{text}: {said}");
    }
    assert_eq!(written("keep_audio = 36500\n").unwrap().keep_audio, KeepAudio::Days(36_500));
    let blank = written("minutes_provider = \"  \"\nminutes_model = \"\"\n").unwrap();
    assert_eq!((blank.minutes_provider, blank.minutes_model), (None, None));
    // Only `false` was "nothing follows".
    assert_eq!(written("transcript = true\n").unwrap().after, After::Transcript);
}

#[test]
fn a_write_that_fails_leaves_the_file_as_it_was() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "reminder = true\n").unwrap();
    // The part file cannot be made: the file is never written in place.
    fs::create_dir(path.with_extension("toml.part")).unwrap();
    let mut settings = read(&path).unwrap();
    settings.reminder = false;
    assert!(write(&path, &settings).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "reminder = true\n");
}

#[test]
fn the_old_key_stays_until_after_is_written() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "transcript = false\n").unwrap();
    let mut settings = read(&path).unwrap();
    settings.reminder = false;
    write(&path, &settings).unwrap();
    // Nothing follows still: the old key says it, as long as `after` is not there.
    assert_eq!(read(&path).unwrap().after, After::Nothing);
    assert!(fs::read_to_string(&path).unwrap().contains("transcript = false"));
}

#[test]
fn the_models_on_this_computer_are_the_folder_s_and_the_one_named_each_once_by_name() {
    // A folder of the test's own: no model of the user's is read.
    let data = tempfile::tempdir().unwrap();
    let models = data.path().join("wholeheard").join("models");
    fs::create_dir_all(&models).unwrap();
    // Made in an order that is not their names'.
    for name in [
        "ggml-small.bin",
        "Ggml-Medium.BIN",
        "ggml-base.bin",
        "notes.txt",
        "ggml-tiny.bin.part",
    ] {
        fs::write(models.join(name), b"x").unwrap();
    }
    fs::create_dir(models.join("old.bin")).unwrap();
    let names = |found: Vec<std::path::PathBuf>| {
        found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    };
    let mut settings = Settings {
        model: models.join("ggml-base.bin"),
        ..Settings::default()
    };
    assert_eq!(
        names(whisper_models(&models, &settings)),
        ["ggml-base.bin", "Ggml-Medium.BIN", "ggml-small.bin"]
    );
    // One the settings name elsewhere, its name before the others: offered, in its place.
    let elsewhere = data.path().join("aaa.bin");
    fs::write(&elsewhere, b"x").unwrap();
    settings.model = elsewhere;
    assert_eq!(
        names(whisper_models(&models, &settings)),
        ["aaa.bin", "ggml-base.bin", "Ggml-Medium.BIN", "ggml-small.bin"]
    );
    // The same file named another way is offered once, as the folder holds it.
    settings.model = models.join("..").join("models").join("ggml-small.bin");
    let found = whisper_models(&models, &settings);
    assert_eq!(names(found.clone()), ["ggml-base.bin", "Ggml-Medium.BIN", "ggml-small.bin"]);
    assert!(found.contains(&models.join("ggml-small.bin")));
    // A named model that is gone, or is a folder, is not offered.
    for gone in [data.path().join("gone.bin"), models.join("old.bin")] {
        settings.model = gone;
        assert_eq!(whisper_models(&models, &settings).len(), 3);
    }
}

#[cfg(unix)]
#[test]
fn a_model_reached_by_a_link_is_offered_once() {
    let data = tempfile::tempdir().unwrap();
    let models = data.path().join("models");
    fs::create_dir_all(&models).unwrap();
    let real = data.path().join("big.bin");
    fs::write(&real, b"x").unwrap();
    std::os::unix::fs::symlink(&real, models.join("big.bin")).unwrap();
    let settings = Settings {
        model: real,
        ..Settings::default()
    };
    assert_eq!(whisper_models(&models, &settings), [models.join("big.bin")]);
}

#[test]
fn the_host_agreed_to_is_one_line_of_the_file_read_back_and_taken_back() {
    assert_eq!(Settings::default().minutes_agreed, None);
    assert_eq!(
        written("minutes_agreed = \"openrouter.ai\"\n").unwrap().minutes_agreed.as_deref(),
        Some("openrouter.ai")
    );
    assert_eq!(written("minutes_agreed = \"  \"\n").unwrap().minutes_agreed, None);
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    let mut settings = Settings {
        minutes_agreed: Some("api.mistral.ai".into()),
        ..Settings::default()
    };
    write(&path, &settings).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("minutes_agreed = \"api.mistral.ai\""));
    assert_eq!(read(&path), Ok(settings.clone()));
    settings.minutes_agreed = None;
    write(&path, &settings).unwrap();
    assert!(!fs::read_to_string(&path).unwrap().contains("minutes_agreed"));
}

#[test]
fn the_developer_mode_is_off_until_the_user_turns_it_on() {
    assert!(!Settings::default().developer);
    assert!(written("developer = true\n").unwrap().developer);
    assert!(!written("developer = false\n").unwrap().developer);
    assert_eq!(
        written("# mine\ndeveloper = \"yes\"\n").map(|_| ()),
        Err("<file>, line 2: invalid type: string \"yes\", expected a boolean".into())
    );
    // Turned on, it is written; turned off again, the line says so — the rest untouched.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "reminder = false # mine\n").unwrap();
    let mut settings = read(&path).unwrap();
    settings.developer = true;
    write(&path, &settings).unwrap();
    assert_eq!(read(&path).unwrap(), settings);
    assert!(fs::read_to_string(&path).unwrap().contains("reminder = false # mine"));
    settings.developer = false;
    write(&path, &settings).unwrap();
    assert!(!read(&path).unwrap().developer);
}

#[test]
fn the_logs_level_is_the_usual_one_until_the_user_asks_for_more() {
    assert_eq!(Settings::default().log, Log::Info);
    assert_eq!(written("log = \"debug\"\n").unwrap().log, Log::Debug);
    assert!(written("log = \"loud\"\n").unwrap_err().starts_with("<file>, line 1: "));
    // As the programs read it in WHOLEHEARD_LOG.
    assert_eq!((Log::Info.as_str(), Log::Debug.as_str()), ("info", "debug"));
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    let settings = Settings {
        log: Log::Debug,
        ..Settings::default()
    };
    write(&path, &settings).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "log = \"debug\"\n");
    assert_eq!(read(&path).unwrap().log, Log::Debug);
}

#[test]
fn a_call_is_offered_to_be_recorded_unless_the_user_said_nothing_is() {
    // D30: offers are on by default, off by `offer = false`, and written back as chosen.
    assert!(Settings::default().offer);
    assert!(!written("offer = false\n").unwrap().offer);
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "# mine\nreminder = false\n").unwrap();
    let mut chosen = read(&path).unwrap();
    chosen.offer = false;
    write(&path, &chosen).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# mine") && text.contains("offer = false"), "{text}");
    assert!(!read(&path).unwrap().offer);
}

#[test]
fn wholeheard_opens_the_window_it_was_left_in_the_small_one_first() {
    // D35: small by default, `window = "full"` when the full one was left; written back as left.
    assert_eq!(Settings::default().window, Window::Small);
    assert_eq!(written("window = \"full\"\n").unwrap().window, Window::Full);
    assert_eq!(written("window = \"small\"\n").unwrap().window, Window::Small);
    assert!(written("window = \"huge\"\n").is_err());
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "# mine\nreminder = false\n").unwrap();
    let mut left = read(&path).unwrap();
    left.window = Window::Full;
    write(&path, &left).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("# mine") && text.contains("window = \"full\""), "{text}");
    assert_eq!(read(&path).unwrap().window, Window::Full);
}

#[test]
fn who_hears_is_whisper_unless_the_file_names_apple_s_speechanalyzer() {
    assert_eq!(Settings::default().engine, Engine::Whisper);
    assert_eq!(written("engine = \"speechanalyzer\"\n").unwrap().engine, Engine::SpeechAnalyzer);
    assert_eq!(written("engine = \"whisper\"\n").unwrap().engine, Engine::Whisper);
    assert!(written("engine = \"siri\"\n").is_err());
    // Written only once it changes, and read back as it was written.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    write(&path, &Settings::default()).unwrap();
    assert!(!fs::read_to_string(&path).unwrap().contains("engine"));
    let chosen = Settings {
        engine: Engine::SpeechAnalyzer,
        ..Settings::default()
    };
    write(&path, &chosen).unwrap();
    assert_eq!(read(&path).unwrap().engine, Engine::SpeechAnalyzer);
}

#[test]
fn a_transcription_service_is_named_as_the_minutes_one_is() {
    let named = written("engine = \"service\"\ntranscription_provider = \"https://api.groq.com/openai/v1\"\ntranscription_model = \"whisper-large-v3\"\ntranscription_agreed = \"api.groq.com\"\n").unwrap();
    assert_eq!(named.engine, Engine::Service);
    assert_eq!(named.transcription_provider.as_deref(), Some("https://api.groq.com/openai/v1"));
    assert_eq!(named.transcription_model.as_deref(), Some("whisper-large-v3"));
    assert_eq!(named.transcription_agreed.as_deref(), Some("api.groq.com"));
    // Written as read, and only once it changes.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    write(&path, &named).unwrap();
    assert_eq!(read(&path).unwrap(), named);
    assert!(Settings::default().transcription_provider.is_none());
}

#[test]
fn welcomed_is_false_until_written_and_kept_by_every_other_write() {
    // Absent is not welcomed yet: the first start's file, written by the autostart, says nothing of it.
    assert!(!Settings::default().welcomed);
    assert!(!written("autostart = true\n").unwrap().welcomed);
    assert!(written("welcomed = true\n").unwrap().welcomed);
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "# mine\nautostart = true\n").unwrap();
    let mut settings = read(&path).unwrap();
    settings.welcomed = true;
    write(&path, &settings).unwrap();
    assert_eq!(read(&path), Ok(settings.clone()));
    assert!(fs::read_to_string(&path).unwrap().contains("welcomed = true"));
    // Another choice written after it: still welcomed.
    settings.reminder = false;
    write(&path, &settings).unwrap();
    let again = read(&path).unwrap();
    assert!(again.welcomed && !again.reminder);
    assert!(fs::read_to_string(&path).unwrap().contains("# mine"));
}

#[test]
fn the_sidebar_is_open_until_folded_and_kept_folded_by_every_other_write() {
    // D61: open by default; folded, written, and read again as it was left.
    assert!(!Settings::default().folded);
    assert!(!written("reminder = false\n").unwrap().folded);
    assert!(written("folded = true\n").unwrap().folded);
    assert!(written("folded = \"yes\"\n").is_err());
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "# mine\nreminder = false\n").unwrap();
    let mut settings = read(&path).unwrap();
    settings.folded = true;
    write(&path, &settings).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("folded = true"));
    settings.offer = false;
    write(&path, &settings).unwrap();
    let again = read(&path).unwrap();
    assert!(again.folded && !again.offer);
    assert!(fs::read_to_string(&path).unwrap().contains("# mine"));
    settings.folded = false;
    write(&path, &settings).unwrap();
    assert!(!read(&path).unwrap().folded);
}

#[test]
fn the_assistant_is_due_only_with_no_meeting_and_not_welcomed() {
    let folder = tempfile::tempdir().unwrap();
    let mut settings = Settings {
        sessions: folder.path().join("Wholeheard"),
        ..Settings::default()
    };
    // No folder yet, or an empty one, or one holding no session: due.
    assert!(welcome_due(&settings));
    fs::create_dir_all(settings.sessions.join("not-a-session")).unwrap();
    assert!(welcome_due(&settings));
    // Welcomed: never again.
    settings.welcomed = true;
    assert!(!welcome_due(&settings));
    // A meeting there: a user who already recorded never sees it, welcomed or not.
    settings.welcomed = false;
    let meeting = settings.sessions.join("2026-10-27_10-00");
    fs::create_dir_all(&meeting).unwrap();
    fs::write(
        meeting.join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-27T10:00:00+01:00","tracks":{}}"#,
    )
    .unwrap();
    assert!(!welcome_due(&settings));
}

#[test]
fn the_meetings_language_is_a_code_of_iso_639_1_or_none_detected() {
    // D65: none — the engine detects it — unless the file names a code.
    assert_eq!(Settings::default().meeting_language, None);
    assert_eq!(
        written("meeting_language = \"en\"\n").unwrap().meeting_language.as_deref(),
        Some("en")
    );
    for not in ["\"English\"", "\"EN\"", "\"zz\"", "\"\""] {
        assert_eq!(
            written(&format!("meeting_language = {not}\n")).unwrap().meeting_language,
            None,
            "{not}"
        );
    }
    // Written as chosen, and read back.
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    let mut settings = Settings {
        meeting_language: Some("de".into()),
        ..Settings::default()
    };
    write(&path, &settings).unwrap();
    assert_eq!(read(&path).unwrap().meeting_language.as_deref(), Some("de"));
    settings.meeting_language = None;
    write(&path, &settings).unwrap();
    assert!(!fs::read_to_string(&path).unwrap().contains("meeting_language"));
}

#[test]
fn the_app_speaks_the_system_s_language_until_one_is_chosen_and_keeps_the_one_chosen() {
    // D64: none written, the system's; French or English chosen, written as its code and read again.
    assert_eq!(Settings::default().language, None);
    assert_eq!(written("language = \"fr\"\n").unwrap().language, Some(Language::French));
    assert_eq!(written("language = \" EN \"\n").unwrap().language, Some(Language::English));
    // A language it does not speak, written by hand: the system's, and the line left as it was.
    assert_eq!(written("language = \"de\"\n").unwrap().language, None);
    assert!(written("language = 3\n").is_err());
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, "# mine\nreminder = false\n").unwrap();
    let mut settings = read(&path).unwrap();
    settings.language = Some(Language::English);
    write(&path, &settings).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("language = \"en\""));
    settings.offer = false;
    write(&path, &settings).unwrap();
    let again = read(&path).unwrap();
    assert!(again.language == Some(Language::English) && !again.offer);
    // The system's again: the key gone, never the system's language of that day pinned.
    settings.language = None;
    write(&path, &settings).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("language") && text.contains("# mine"), "{text}");
    fs::write(&path, "language = \"de\"\n").unwrap();
    let mut by_hand = read(&path).unwrap();
    by_hand.offer = false;
    write(&path, &by_hand).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("language = \"de\""));
}

#[test]
fn a_failure_of_the_settings_file_names_it_without_its_path_even_through_a_link() {
    use wholeheard_shell::settings::unnamed;
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("settings.toml");
    fs::write(&file, "keep_audio = 0\n").unwrap();
    let said = read(&file).unwrap_err();
    assert_eq!(
        unnamed(&said, &file),
        "settings.toml, line 1: keep_audio is \"always\" or a number of days"
    );
    assert_eq!(unnamed("os error 28", &file), "os error 28");
    #[cfg(unix)]
    {
        let other = tempfile::tempdir().unwrap();
        let link = other.path().join("settings.toml");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        // Written through a link, the file it points to is the one its error names.
        let refused = write(&link, &Settings::default()).unwrap_err().to_string();
        assert_eq!(
            unnamed(&refused, &link),
            "settings.toml, line 1: keep_audio is \"always\" or a number of days"
        );
    }
}
