//! The language a meeting is spoken in (D65): one line, a code of ISO 639-1.

use std::fs;

use wholeheard_shell::spoken::{FILE, code, mark, read};

fn folder(name: &str) -> std::path::PathBuf {
    let made = std::env::temp_dir().join(format!("wholeheard-spoken-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&made);
    fs::create_dir_all(&made).unwrap();
    made
}

#[test]
fn a_code_is_two_lower_case_letters_and_nothing_else() {
    assert_eq!(code(" fr\n"), Some("fr".to_string()));
    assert_eq!(code("zh"), Some("zh".to_string()));
    for not in ["", "FR", "fr-FR", "fra", "f", "f1", "é1", "  ", "zz", "qq", "xx"] {
        assert_eq!(code(not), None, "{not:?}");
    }
}

#[test]
fn said_written_whole_read_back_removed_and_refused_before_session_json_or_for_no_code() {
    let dir = folder("mark");
    assert_eq!(mark(&dir, Some("en")).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    assert!(!dir.join(FILE).exists());
    fs::write(dir.join("session.json"), "{}").unwrap();
    mark(&dir, Some("en")).unwrap();
    assert_eq!(fs::read_to_string(dir.join(FILE)).unwrap(), "en\n");
    assert_eq!(read(&dir), Some("en".to_string()));
    assert_eq!(mark(&dir, Some("English")).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(read(&dir), Some("en".to_string()));
    mark(&dir, None).unwrap();
    assert!(!dir.join(FILE).exists());
    mark(&dir, None).unwrap();
    assert_eq!(read(&dir), None);
    // Written by hand, not a code: the meeting names none of its own.
    fs::write(dir.join(FILE), "Français\n").unwrap();
    assert_eq!(read(&dir), None);
    assert!(!dir.join(".language.part").exists());
    fs::remove_dir_all(dir).unwrap();
}
