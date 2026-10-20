//! Summit became Wholeheard (D28): what a user held under the old name is carried over once,
//! nothing lost, nothing overwritten.

use std::fs;
use std::path::Path;

use wholeheard_shell::migrate::{Carried, Places, carry_over};

fn places(root: &Path) -> Places {
    Places {
        config: root.join("config"),
        data: root.join("data"),
        documents: root.join("Documents"),
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn summits_folders_are_moved_and_the_meetings_kept_where_they_were() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    let model = here.data.join("summit/models/ggml-large-v3-turbo-q8_0.bin");
    write(&model, "a model of 874 MB");
    write(
        &here.config.join("summit/settings.toml"),
        &format!("# mine\nmodel = \"{}\"\n", model.display()),
    );
    write(&here.documents.join("Summit/2026-10-18_10-00/session.json"), "{}");

    let carried = carry_over(&here).unwrap();

    assert!(carried.settings && carried.data, "{carried:?}");
    assert!(!here.config.join("summit").exists() && !here.data.join("summit").exists());
    let moved = here.data.join("wholeheard/models/ggml-large-v3-turbo-q8_0.bin");
    assert_eq!(
        fs::read_to_string(&moved).unwrap(),
        "a model of 874 MB",
        "the model is moved, never fetched again"
    );
    let settings = fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap();
    assert!(settings.starts_with("# mine\n"), "what the user wrote stays: {settings}");
    assert!(settings.contains(&format!("model = \"{}\"", moved.display())), "{settings}");
    let meetings = here.documents.join("Summit");
    assert!(settings.contains(&format!("sessions = \"{}\"", meetings.display())), "{settings}");
    assert_eq!(carried.meetings_kept.as_deref(), Some(meetings.as_path()));
    assert!(
        meetings.join("2026-10-18_10-00/session.json").is_file(),
        "the meetings are not moved"
    );
    let read = wholeheard_shell::settings::read(&here.config.join("wholeheard/settings.toml")).unwrap();
    assert_eq!(read.sessions, meetings, "Wholeheard reads Summit's meetings");
}

#[test]
fn a_folder_the_settings_name_is_left_as_named() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    let elsewhere = root.path().join("my meetings");
    write(
        &here.config.join("summit/settings.toml"),
        &format!("sessions = \"{}\"\n", elsewhere.display()),
    );
    fs::create_dir_all(here.documents.join("Summit")).unwrap();

    let carried = carry_over(&here).unwrap();

    assert!(carried.settings && carried.meetings_kept.is_none(), "{carried:?}");
    let settings = fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap();
    assert_eq!(settings.matches("sessions").count(), 1, "{settings}");
}

#[test]
fn meetings_made_with_no_settings_saved_are_found() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    fs::create_dir_all(here.documents.join("Summit/2026-10-18_10-00")).unwrap();

    let carried = carry_over(&here).unwrap();

    let meetings = here.documents.join("Summit");
    assert_eq!(carried.meetings_kept.as_deref(), Some(meetings.as_path()));
    let read = wholeheard_shell::settings::read(&here.config.join("wholeheard/settings.toml")).unwrap();
    assert_eq!(read.sessions, meetings);
}

#[test]
fn nothing_of_wholeheards_is_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    write(&here.config.join("summit/settings.toml"), "reminder = false\n");
    write(&here.config.join("wholeheard/settings.toml"), "reminder = true\n");
    write(&here.data.join("summit/models/a.bin"), "old");
    write(&here.data.join("wholeheard/models/a.bin"), "new");

    let carried = carry_over(&here).unwrap();

    assert_eq!(carried, Carried::default());
    assert_eq!(
        fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap(),
        "reminder = true\n"
    );
    assert_eq!(fs::read_to_string(here.data.join("wholeheard/models/a.bin")).unwrap(), "new");
    assert!(here.config.join("summit/settings.toml").is_file(), "Summit's is left as it is");
}

#[test]
fn carried_once_then_nothing_more() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    write(&here.config.join("summit/settings.toml"), "reminder = false\n");
    fs::create_dir_all(here.documents.join("Summit")).unwrap();

    carry_over(&here).unwrap();
    let after_first = fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap();
    let second = carry_over(&here).unwrap();

    assert_eq!(second, Carried::default());
    assert_eq!(
        fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap(),
        after_first
    );
}

#[test]
fn a_user_who_never_had_summit_gets_nothing_written() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());

    assert_eq!(carry_over(&here).unwrap(), Carried::default());
    assert!(!here.config.join("wholeheard").exists());
}
