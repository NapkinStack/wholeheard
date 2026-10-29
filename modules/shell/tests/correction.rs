//! A line of a transcript corrected by whoever listens to it (D62, ADR-0014).

use std::fs;

use wholeheard_shell::correction::{Refused, correct, tidy};

const HEARD: &str = concat!(
    r#"{"type":"header","contract":"transcript/v1","engine":"whisper.cpp","model":"m","language":null,"context":null}"#,
    "\n",
    r#"{"type":"segment","speaker":"me","track":"local","voice":null,"start":0.4,"end":1.2,"text":"Bonjour à tous."}"#,
    "\n",
    r#"{"type":"segment","speaker":"others","track":"remote","voice":null,"start":0.4,"end":2.0,"text":"Salut Clair."}"#,
    "\n",
    r#"{"type":"covered","speaker":"me","until":3.0}"#,
    "\n",
    r#"{"type":"complete","duration":3.0}"#,
    "\n",
);

fn folder(name: &str) -> std::path::PathBuf {
    let made = std::env::temp_dir().join(format!("wholeheard-correction-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&made);
    fs::create_dir_all(&made).unwrap();
    made
}

#[test]
fn a_line_corrected_changes_its_text_and_edited_alone_the_file_replaced_whole() {
    let dir = folder("one");
    let file = dir.join("transcript.jsonl");
    fs::write(&file, HEARD).unwrap();
    correct(
        &file,
        "remote",
        0.4,
        "  Salut Claire,\nça va ? 🙂 Jiří  ",
        "2026-10-29T10:00:00+01:00",
    )
    .unwrap();
    let after = fs::read_to_string(&file).unwrap();
    let (before, now): (Vec<_>, Vec<_>) = (HEARD.lines().collect(), after.lines().collect());
    assert_eq!(before.len(), now.len());
    for (at, (was, is)) in before.iter().zip(&now).enumerate() {
        if at == 2 {
            let line: serde_json::Value = serde_json::from_str(is).unwrap();
            assert_eq!(line["text"], "Salut Claire, ça va ? 🙂 Jiří");
            assert_eq!(line["edited"], "2026-10-29T10:00:00+01:00");
            assert_eq!(
                (line["speaker"].as_str(), line["start"].as_f64(), line["end"].as_f64()),
                (Some("others"), Some(0.4), Some(2.0))
            );
        } else {
            // Every other line as it was, byte for byte, in its order: the local one at 0.4 too.
            assert_eq!(was, is);
        }
    }
    assert!(after.ends_with('\n'));
    assert!(!dir.join("transcript.jsonl.part").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_correction_is_refused_before_complete_on_no_line_and_empty_and_leaves_the_file_as_it_was() {
    let dir = folder("refused");
    let file = dir.join("transcript.jsonl");
    let unfinished: String = HEARD
        .lines()
        .filter(|line| !line.contains("complete"))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::write(&file, &unfinished).unwrap();
    assert_eq!(correct(&file, "local", 0.4, "Bonjour.", "t"), Err(Refused::NotComplete));
    assert_eq!(fs::read_to_string(&file).unwrap(), unfinished);
    fs::write(&file, HEARD).unwrap();
    assert_eq!(correct(&file, "local", 9.0, "Bonjour.", "t"), Err(Refused::NoSuchLine));
    // Known by its speaker and its start: no local line at 0.4 is a remote one.
    assert_eq!(correct(&file, "local", 0.41, "x", "t"), Err(Refused::NoSuchLine));
    assert_eq!(correct(&file, "local", 0.4, " \n\r ", "t"), Err(Refused::Empty));
    assert_eq!(fs::read_to_string(&file).unwrap(), HEARD);
    assert!(matches!(
        correct(&dir.join("none.jsonl"), "local", 0.4, "x", "t"),
        Err(Refused::File(_))
    ));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_text_kept_is_one_line_its_ends_trimmed() {
    assert_eq!(tidy(" a\r\nb\nc\rd "), Some("a b c d".to_string()));
    assert_eq!(tidy("\n \t"), None);
}

// Unix's: a read-only file is replaced by a rename there, never on Windows.
#[cfg(unix)]
#[test]
fn a_correction_replaces_the_file_whole_even_one_that_cannot_be_written_in_place() {
    // Read-only, the file cannot be written in place; written beside and renamed over, it is
    // replaced all the same — a reader never sees half of it.
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("beside");
    let file = dir.join("transcript.jsonl");
    fs::write(&file, HEARD).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o444)).unwrap();
    correct(&file, "local", 0.4, "Bonjour à toutes.", "2026-10-29T10:00:00+01:00").unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("Bonjour à toutes."));
    assert!(!dir.join("transcript.jsonl.part").exists());
    fs::remove_dir_all(dir).unwrap();
}
