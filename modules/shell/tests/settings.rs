//! The settings, read as a user wrote them — or did not.

use std::{fs, path::Path};

use summit_shell::settings::{Settings, file, read};

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
    assert!(defaults.transcript && defaults.model.is_absolute());
    assert!(defaults.model.ends_with(format!("summit/models/{}", summit_shell::settings::MODEL)));
    let chosen = written("transcript = false\nmodel = \"~/models/ggml-small.bin\"\n").unwrap();
    assert!(!chosen.transcript && chosen.model.is_absolute() && chosen.model.ends_with("models/ggml-small.bin"));
    let said = written("\nmodel = \"ggml-small.bin\"\n").unwrap_err();
    assert!(said.starts_with("<file>, line 2: model is the file's whole path"), "{said}");
}
