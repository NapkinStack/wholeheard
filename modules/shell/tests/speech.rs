//! What Apple's SpeechAnalyzer offers on this Mac (D42), as `wholeheard-speech status` says it,
//! tried with a program played in its place: on Linux, and anywhere it is not, nothing is offered.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use wholeheard_shell::speech::{self, Status};

fn played(folder: &Path, answer: &str) -> std::path::PathBuf {
    let program = folder.join("wholeheard-speech");
    fs::write(
        &program,
        format!("#!/bin/sh\necho \"$@\" >> '{}/asked'\n{answer}\n", folder.display()),
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    program
}

#[test]
fn what_the_program_says_of_a_locale_is_read() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(folder.path(), r#"echo '{"available":true,"installed":false,"supported":true}'"#);
    assert_eq!(
        speech::status(&program, "fr"),
        Some(Status {
            offered: true,
            installed: false
        })
    );
    assert_eq!(fs::read_to_string(folder.path().join("asked")).unwrap().trim(), "status fr-FR");
}

#[test]
fn nothing_is_offered_where_it_is_not_heard() {
    let folder = tempfile::tempdir().unwrap();
    for answer in [
        r#"echo '{"available":false,"installed":false,"supported":false}'"#,
        r#"echo '{"available":true,"installed":false,"supported":false}'"#,
        "echo 'wholeheard-speech: SpeechTranscriber is not available on this Mac' >&2; exit 1",
        "echo 'not json'",
        "sleep 30",
    ] {
        let program = played(folder.path(), answer);
        assert!(speech::status(&program, "fr").is_none_or(|status| !status.offered), "{answer}");
    }
    assert_eq!(speech::status(&folder.path().join("nowhere"), "fr"), None);
    assert_eq!(speech::status(&played(folder.path(), "true"), "de"), None);
}

#[test]
fn apple_s_model_is_installed_only_when_asked_and_its_refusal_is_said() {
    let folder = tempfile::tempdir().unwrap();
    let program = played(folder.path(), r#"echo '{"installed":true}'"#);
    assert_eq!(speech::install(&program, "en"), Ok(()));
    assert_eq!(fs::read_to_string(folder.path().join("asked")).unwrap().trim(), "install en-US");
    let refused = played(folder.path(), "echo 'wholeheard-speech: the download was refused' >&2; exit 1");
    assert!(speech::install(&refused, "fr").unwrap_err().contains("the download was refused"));
}
