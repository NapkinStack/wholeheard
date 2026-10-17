//! The settings, read as a user wrote them — or did not.

use std::{fs, path::Path};

use summit_shell::settings::{After, KeepAudio, Settings, file, read, write};

fn written(text: &str) -> Result<Settings, String> {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, text).unwrap();
    read(&path).map_err(|said| said.replace(&path.display().to_string(), "<file>"))
}

#[test]
fn no_file_and_an_empty_one_are_every_default() {
    let defaults = Settings::default();
    // The reminder is shown unless the user says otherwise; sessions go to a folder named Summit.
    assert!(defaults.reminder && defaults.sessions.is_absolute() && defaults.sessions.ends_with("Summit"));
    assert_eq!(read(Path::new("/nowhere/settings.toml")), Ok(defaults.clone()));
    for nothing in ["", "\n\n", "# only a comment\n"] {
        assert_eq!(written(nothing), Ok(defaults.clone()), "{nothing:?}");
    }
    assert!(file().is_some_and(|path| path.ends_with("summit/settings.toml")));
}

#[test]
fn a_file_names_what_it_changes_and_leaves_the_rest() {
    let only = written("reminder = false\n").unwrap();
    assert!(!only.reminder && only.sessions == Settings::default().sessions);
    let both = written("sessions = \"/srv/meetings\"\nreminder = true\n").unwrap();
    assert!(both.reminder && both.sessions == Path::new("/srv/meetings"));
    // A key of a later version is left alone: the file is not refused for it.
    assert_eq!(
        written("after_recording = \"minutes\"\n[provider]\nhost = \"x\"\n"),
        Ok(Settings::default())
    );
    // A folder from the user's home, as people write it.
    let home = written(&format!("sessions = \"{}/Réunions/Summit\"\n", '~')).unwrap().sessions;
    assert!(home.is_absolute() && home.ends_with("Réunions/Summit") && !home.to_string_lossy().contains('~'));
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
            "reminder = false\nsessions = \"Summit\"\n",
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
    // Out of the box the transcript is made, with the default model in Summit's data folder.
    let defaults = Settings::default();
    assert!(defaults.after == After::Transcript && defaults.model.is_absolute());
    assert!(defaults.model.ends_with(format!("summit/models/{}", summit_shell::settings::MODEL)));
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
    let path = folder.path().join("summit").join("settings.toml");
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
