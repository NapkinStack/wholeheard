//! The name the user gave a meeting (D70): one line of plain text, before the minutes' title.

use std::{fs, io::ErrorKind};

use wholeheard_shell::{
    named::{FILE, MOST, read, within, write},
    sessions,
};

fn folder(name: &str) -> std::path::PathBuf {
    let made = std::env::temp_dir().join(format!("wholeheard-named-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&made);
    fs::create_dir_all(&made).unwrap();
    made
}

fn session(folder: &std::path::Path) {
    fs::write(
        folder.join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-28T14:00:00+01:00","stopped":"2026-10-28T14:10:00+01:00","tracks":{}}"#,
    )
    .unwrap();
}

fn minutes(folder: &std::path::Path, title: &str) {
    fs::write(
        folder.join("minutes.md"),
        format!(
            "---\ncontract: minutes/v2\nmade: 2026-10-28T15:00:00+01:00\nprovider: local\nmodel: m\ncontext: null\ntitle: {title}\n---\n\n## 1.\n\nA.\n\n## 2.\n\n- B\n\n## 3.\n\n- C\n"
        ),
    )
    .unwrap();
}

fn listed(sessions_folder: &std::path::Path) -> Option<String> {
    sessions::list(sessions_folder, None).remove(0).title
}

#[test]
fn a_name_is_one_line_not_blank_within_its_length_counted_in_characters() {
    assert_eq!(within("  Point budget \n"), Some("Point budget"));
    assert_eq!(within("🎉"), Some("🎉"), "a name need hold no letter");
    assert_eq!(
        within("../../etc/passwd"),
        Some("../../etc/passwd"),
        "a path is a name like another"
    );
    let longest = "é".repeat(MOST);
    assert_eq!(within(&longest), Some(longest.as_str()));
    for not in [
        "\u{200b}",
        "\u{feff}\u{200b}",
        "",
        "  \n",
        "two\nlines",
        "tab\there",
        "a\u{2028}b",
        "a\u{7}b",
        &"é".repeat(MOST + 1),
    ] {
        assert_eq!(within(not), None, "{not:?}");
    }
}

#[test]
fn written_whole_read_back_removed_blank_refused_before_session_json_or_for_no_line() {
    let dir = folder("write");
    assert_eq!(write(&dir, "Point").unwrap_err().kind(), ErrorKind::NotFound);
    assert!(!dir.join(FILE).exists());
    session(&dir);
    write(&dir, "  Point budget ").unwrap();
    assert_eq!(fs::read_to_string(dir.join(FILE)).unwrap(), "Point budget\n");
    assert_eq!(read(&dir).as_deref(), Some("Point budget"));
    assert_eq!(write(&dir, "two\nlines").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(read(&dir).as_deref(), Some("Point budget"), "refused, the name stays");
    write(&dir, "   ").unwrap();
    assert!(!dir.join(FILE).exists());
    write(&dir, "").unwrap();
    assert_eq!(read(&dir), None);
    let names: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["session.json"], "nothing is left beside it");
}

#[cfg(unix)]
#[test]
fn it_is_renamed_over_the_old_one_never_written_in_place_and_a_failure_leaves_nothing_beside() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("whole");
    session(&dir);
    write(&dir, "Avant").unwrap();
    // Refused to anyone writing in place; a rename does not ask the file.
    fs::set_permissions(dir.join(FILE), fs::Permissions::from_mode(0o444)).unwrap();
    write(&dir, "Après").unwrap();
    assert_eq!(read(&dir).as_deref(), Some("Après"));
    fs::remove_file(dir.join(FILE)).unwrap();
    // The rename refused: a folder in its place, that holds something.
    fs::create_dir(dir.join(FILE)).unwrap();
    fs::write(dir.join(FILE).join("x"), "").unwrap();
    assert!(write(&dir, "Encore").is_err());
    assert!(!dir.join(".title.part").exists(), "nothing is left beside it");
}

#[test]
fn a_file_that_is_no_such_line_gives_no_name() {
    let dir = folder("read");
    session(&dir);
    for not in [&b"two\nlines\n"[..], b"   \n", b"", b"\xff\xfe", "é".repeat(MOST + 1).as_bytes()] {
        fs::write(dir.join(FILE), not).unwrap();
        assert_eq!(read(&dir), None, "{not:?}");
    }
}

#[test]
fn the_name_comes_before_the_minutes_title_which_comes_back_when_it_goes() {
    let all = folder("order");
    let dir = all.join("2026-10-28_14-00");
    fs::create_dir(&dir).unwrap();
    session(&dir);
    assert_eq!(listed(&all), None, "neither: named by its date");
    minutes(&dir, "Du compte rendu");
    assert_eq!(listed(&all).as_deref(), Some("Du compte rendu"));
    write(&dir, "De la main").unwrap();
    assert_eq!(listed(&all).as_deref(), Some("De la main"));
    minutes(&dir, "Refait");
    assert_eq!(listed(&all).as_deref(), Some("De la main"), "minutes made again: the name stays");
    write(&dir, "").unwrap();
    assert_eq!(listed(&all).as_deref(), Some("Refait"));
    fs::remove_file(dir.join("minutes.md")).unwrap();
    assert_eq!(listed(&all), None);
    assert!(all.join("2026-10-28_14-00").is_dir(), "the folder keeps its name");
}
