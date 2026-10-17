//! Minutes made, tried with a program played here: a script given the same arguments as
//! `summit-summary`, saying what `contracts/summarising/v1` says. No provider, no key of anyone.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Mutex, time::Duration};

use summit_shell::summarising::{Event, Made, Summarising};

/// What a making by a script said, to its end — one at a time, as for transcripts.
fn said(folder: &Path, script: &str, key: Option<&str>) -> Vec<Event> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap();
    let path = folder.join("summit-summary");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let (_making, events) = Summarising::start(&path, &folder.join("session"), "https://openrouter.ai/api/v1", "a-model", key).unwrap();
    let mut all = Vec::new();
    while let Ok(event) = events.recv_timeout(Duration::from_secs(10)) {
        let ended = matches!(event, Event::Ended(_));
        all.push(event);
        if ended {
            return all;
        }
    }
    panic!("the making never ended: {all:?}")
}

fn session() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir(folder.path().join("session")).unwrap();
    folder
}

#[test]
fn minutes_made_say_where_they_go_then_that_they_are_written() {
    let folder = session();
    let events = said(
        folder.path(),
        r#"[ "$2" = --provider ] && [ "$3" = https://openrouter.ai/api/v1 ] && [ "$4" = --model ] && [ "$5" = a-model ] || exit 9
[ "$SUMMIT_PROVIDER_KEY" = a-key ] || exit 8
echo 'a line that is no object'
echo '{"event":"sending","host":"openrouter.ai","model":"a-model","characters":120}'
echo '{"event":"of a later version"}'
echo '## A' > "$1/minutes.md"
echo '{"event":"written","file":"x"}'"#,
        Some("a-key"),
    );
    assert_eq!(
        events,
        [
            Event::Sending {
                host: "openrouter.ai".into(),
                model: "a-model".into(),
                characters: 120
            },
            Event::Ended(Made::Written(folder.path().join("session"))),
        ]
    );
}

#[test]
fn a_making_that_failed_says_the_programs_sentence_and_no_key_is_given_unless_kept() {
    let folder = session();
    // A key in Summit's own environment is never passed on: only the one kept for the provider.
    unsafe { std::env::set_var("SUMMIT_PROVIDER_KEY", "summits-own") };
    let events = said(
        folder.path(),
        r#"[ -z "$SUMMIT_PROVIDER_KEY" ] || exit 8
echo '{"event":"sending","host":"openrouter.ai","model":"a-model","characters":1}'
echo 'summit-summary: the provider at openrouter.ai refused the key (HTTP 401)' >&2
exit 1"#,
        None,
    );
    unsafe { std::env::remove_var("SUMMIT_PROVIDER_KEY") };
    assert_eq!(
        events.last(),
        Some(&Event::Ended(Made::Failed(
            "the provider at openrouter.ai refused the key (HTTP 401)".into()
        )))
    );
}

#[test]
fn written_is_believed_only_from_a_program_that_ended_well_with_the_file_there() {
    for script in [
        r#"echo '{"event":"written","file":"x"}'; exit 1"#,
        r#"echo '{"event":"written","file":"x"}'"#,
        "exit 3",
    ] {
        let folder = session();
        let events = said(folder.path(), script, None);
        let Some(Event::Ended(Made::Failed(why))) = events.last() else {
            panic!("{script}: {events:?}")
        };
        assert!(!why.is_empty(), "{script}");
    }
}
