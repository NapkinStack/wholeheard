//! Summit became Wholeheard (D28): what a user held under the old name is carried over, nothing
//! lost, nothing overwritten; a step interrupted is done at the next start.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use wholeheard_shell::migrate::{Carried, Places, carry_over};
use wholeheard_shell::settings;

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

fn a_meeting_in(folder: &Path) {
    write(&folder.join("2026-10-18_10-00/session.json"), "{}");
}

fn now(here: &Places) -> settings::Settings {
    settings::read(&here.config.join("wholeheard/settings.toml")).unwrap()
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
    a_meeting_in(&here.documents.join("Summit"));

    let carried = carry_over(&here).unwrap();

    assert!(carried.settings && carried.data && carried.model_named, "{carried:?}");
    assert!(!here.config.join("summit").exists() && !here.data.join("summit").exists());
    let moved = here.data.join("wholeheard/models/ggml-large-v3-turbo-q8_0.bin");
    assert_eq!(
        fs::read_to_string(&moved).unwrap(),
        "a model of 874 MB",
        "the model is moved, never fetched again"
    );
    let text = fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap();
    assert!(text.starts_with("# mine\n"), "what the user wrote stays: {text}");
    let meetings = here.documents.join("Summit");
    assert_eq!(carried.meetings_kept.as_deref(), Some(meetings.as_path()));
    let read = now(&here);
    assert_eq!(
        (read.sessions, read.model),
        (meetings.clone(), moved),
        "Wholeheard reads Summit's meetings and model"
    );
    assert!(
        meetings.join("2026-10-18_10-00/session.json").is_file(),
        "the meetings are not moved"
    );
}

#[test]
fn a_settings_file_with_no_line_end_stays_readable() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    write(&here.config.join("summit/settings.toml"), "minutes_model = \"mistral\"");
    a_meeting_in(&here.documents.join("Summit"));

    carry_over(&here).unwrap();

    let read = now(&here);
    assert_eq!(read.minutes_model.as_deref(), Some("mistral"));
    assert_eq!(read.sessions, here.documents.join("Summit"));
}

#[test]
fn a_move_that_failed_is_done_at_the_next_start_and_the_meetings_never_lost() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    write(&here.config.join("summit/settings.toml"), "reminder = false\n");
    write(&here.data.join("summit/models/a.bin"), "a model");
    a_meeting_in(&here.documents.join("Summit"));
    // The data's folder cannot be moved this time: its parent refuses.
    let data_mode = fs::metadata(&here.data).unwrap().permissions();
    fs::set_permissions(&here.data, fs::Permissions::from_mode(0o500)).unwrap();

    assert!(carry_over(&here).is_err());
    let read = now(&here);
    assert_eq!(read.sessions, here.documents.join("Summit"), "the meetings were named first");

    fs::set_permissions(&here.data, data_mode).unwrap();
    let carried = carry_over(&here).unwrap();
    assert!(carried.data && !carried.settings, "{carried:?}");
    assert_eq!(fs::read_to_string(here.data.join("wholeheard/models/a.bin")).unwrap(), "a model");
    assert_eq!(now(&here).sessions, here.documents.join("Summit"));
}

#[test]
fn an_empty_folder_under_the_new_name_gives_way() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    let mine = root.path().join("my meetings");
    write(
        &here.config.join("summit/settings.toml"),
        &format!("sessions = \"{}\"\n", mine.display()),
    );
    fs::create_dir_all(here.config.join("wholeheard")).unwrap();
    a_meeting_in(&here.documents.join("Summit"));

    let carried = carry_over(&here).unwrap();

    assert!(carried.settings && carried.meetings_kept.is_none(), "{carried:?}");
    assert_eq!(now(&here).sessions, mine, "the folder the user named is kept");
}

#[test]
fn a_model_written_from_home_is_named_where_it_went() {
    let root = tempfile::tempdir().unwrap();
    let home = dirs::home_dir().unwrap();
    // Summit's data folder under the home, as the user wrote it: `~/…`.
    let data = tempfile::tempdir_in(&home).unwrap();
    let here = Places {
        config: root.path().join("config"),
        data: data.path().to_path_buf(),
        documents: root.path().join("Documents"),
    };
    write(&here.data.join("summit/models/a.bin"), "a model");
    let written = format!("~/{}/summit/models/a.bin", data.path().strip_prefix(&home).unwrap().display());
    write(&here.config.join("summit/settings.toml"), &format!("model = \"{written}\"\n"));

    let carried = carry_over(&here).unwrap();

    assert!(carried.model_named, "{carried:?}");
    assert_eq!(now(&here).model, here.data.join("wholeheard/models/a.bin"));
}

#[test]
fn a_model_beside_summits_folder_is_not_renamed_by_its_text() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    let beside = here.data.join("summit-models/m.bin");
    write(&beside, "m");
    write(&here.data.join("summit/models/a.bin"), "a model");
    write(
        &here.config.join("summit/settings.toml"),
        &format!("model = \"{}\"\n", beside.display()),
    );

    carry_over(&here).unwrap();

    assert_eq!(now(&here).model, beside);
}

#[test]
fn a_folder_named_summit_with_no_meeting_is_not_taken() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    write(&here.documents.join("Summit/holiday.jpg"), "a photo");

    assert_eq!(carry_over(&here).unwrap(), Carried::default());
    assert!(!here.config.join("wholeheard").exists());
}

#[test]
fn meetings_made_with_no_settings_saved_are_found() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    a_meeting_in(&here.documents.join("Summit"));

    let carried = carry_over(&here).unwrap();

    let meetings = here.documents.join("Summit");
    assert_eq!(carried.meetings_kept.as_deref(), Some(meetings.as_path()));
    assert_eq!(now(&here).sessions, meetings);
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
    a_meeting_in(&here.documents.join("Summit"));

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

#[test]
fn a_folder_the_user_took_out_of_their_settings_stays_out() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    write(&here.config.join("summit/settings.toml"), "reminder = false\n");
    a_meeting_in(&here.documents.join("Summit"));
    carry_over(&here).unwrap();
    // The user takes the folder out, on purpose: back to Wholeheard's own.
    write(&here.config.join("wholeheard/settings.toml"), "reminder = false\n");

    carry_over(&here).unwrap();

    assert_eq!(
        fs::read_to_string(here.config.join("wholeheard/settings.toml")).unwrap(),
        "reminder = false\n"
    );
}

#[test]
fn a_settings_file_kept_elsewhere_by_a_link_is_written_there_with_its_permissions() {
    let root = tempfile::tempdir().unwrap();
    let here = places(root.path());
    let dotfiles = root.path().join("dotfiles/settings.toml");
    write(&dotfiles, "reminder = false\n");
    fs::set_permissions(&dotfiles, fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir_all(here.config.join("summit")).unwrap();
    std::os::unix::fs::symlink(&dotfiles, here.config.join("summit/settings.toml")).unwrap();
    a_meeting_in(&here.documents.join("Summit"));

    carry_over(&here).unwrap();

    let link = here.config.join("wholeheard/settings.toml");
    assert!(
        fs::symlink_metadata(&link).unwrap().file_type().is_symlink(),
        "the link stays a link"
    );
    assert!(
        fs::read_to_string(&dotfiles).unwrap().contains("sessions"),
        "written where it leads"
    );
    assert_eq!(fs::metadata(&dotfiles).unwrap().permissions().mode() & 0o777, 0o600);
}
