//! Minutes made, tried with a program played here: a script given the same arguments as
//! `wholeheard-summary`, saying what `contracts/summarising/v1` says. No provider, no key of anyone.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Mutex, time::Duration};

use wholeheard_shell::summarising::{Cause, Event, Made, Summarising};

/// What a making by a script said, to its end — one at a time, as for transcripts.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn said(folder: &Path, script: &str, key: Option<&str>) -> Vec<Event> {
    let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    said_held(folder, script, key)
}

/// The same, the lock already held by the caller.
fn said_held(folder: &Path, script: &str, key: Option<&str>) -> Vec<Event> {
    let path = folder.join("wholeheard-summary");
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
[ "$WHOLEHEARD_PROVIDER_KEY" = a-key ] || exit 8
echo 'a line that is no object'
echo '{"event":"sending","host":"openrouter.ai","model":"a-model","characters":120}'
echo '{"event":"of a later version","host":"elsewhere","model":"m","characters":1}'
echo '{"event":"sending","host":"elsewhere","model":"m","characters":1.5}'
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
    // A key in Wholeheard's own environment is never passed on: only the one kept for the provider.
    // Set while no other test starts a program.
    let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe { std::env::set_var("WHOLEHEARD_PROVIDER_KEY", "wholehearts-own") };
    let events = said_held(
        folder.path(),
        r#"[ -z "$WHOLEHEARD_PROVIDER_KEY" ] || exit 8
echo '{"event":"sending","host":"openrouter.ai","model":"a-model","characters":1}'
echo 'wholeheard-summary: the provider at openrouter.ai refused the key (HTTP 401)' >&2
exit 1"#,
        None,
    );
    unsafe { std::env::remove_var("WHOLEHEARD_PROVIDER_KEY") };
    assert_eq!(
        events.last(),
        Some(&Event::Ended(Made::Failed(
            "the provider at openrouter.ai refused the key (HTTP 401)".into(),
            None
        )))
    );
}

#[test]
fn written_is_believed_only_from_a_program_that_ended_well_with_the_file_there() {
    for script in [
        r#"echo '{"event":"written","file":"x"}'; exit 1"#,
        r#"echo '{"event":"written","file":"x"}'"#,
        "exit 3",
        // The file there — minutes of before, or written — is not enough: `written` is said.
        r#"echo '## A' > "$1/minutes.md""#,
        r#"echo '## A' > "$1/minutes.md"; echo '{"event":"written","file":"x"}'; echo 'wholeheard-summary: late' >&2; exit 1"#,
    ] {
        let folder = session();
        let events = said(folder.path(), script, None);
        let Some(Event::Ended(Made::Failed(why, _))) = events.last() else {
            panic!("{script}: {events:?}")
        };
        assert!(!why.is_empty(), "{script}");
        // The sentence said last, just before the end, is the one told.
        if script.contains("late") {
            assert_eq!(why, "late");
        }
    }
}

#[test]
fn a_making_ended_by_wholeheard_ends_failed_and_soon() {
    let folder = session();
    let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let path = folder.path().join("wholeheard-summary");
    fs::write(
        &path,
        "#!/bin/sh
exec sleep 30
",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let (making, events) = Summarising::start(&path, &folder.path().join("session"), "http://127.0.0.1:9/v1", "m", None).unwrap();
    making.end();
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(5)),
        Ok(Event::Ended(Made::Failed(..)))
    ));
}

#[test]
fn a_making_that_failed_says_its_cause_when_its_program_names_one_this_version_knows() {
    for (code, cause) in [
        ("provider.unreachable", Some(Cause::Unreachable)),
        ("provider.too_slow", Some(Cause::TooSlow)),
        ("provider.redirected", Some(Cause::Redirected)),
        ("answer.cut_short", Some(Cause::CutShort)),
        ("provider.refused_key", Some(Cause::RefusedKey)),
        ("provider.no_credit", Some(Cause::NoCredit)),
        ("provider.unknown_model", Some(Cause::UnknownModel)),
        ("provider.failed", Some(Cause::ProviderFailed)),
        ("answer.not_minutes", Some(Cause::NotMinutes)),
        ("transcript.unusable", Some(Cause::TranscriptUnusable)),
        ("transcript.no_speech", Some(Cause::NoSpeech)),
        ("settings.invalid", Some(Cause::Settings)),
        ("minutes.unwritable", Some(Cause::Unwritable)),
        // A code of a later version is read as none (summarising/v1).
        ("provider.on_fire", None),
    ] {
        let folder = session();
        let events = said(
            folder.path(),
            &format!(
                r#"echo '{{"event":"failed","code":"{code}"}}'
echo 'wholeheard-summary: a sentence' >&2
exit 1"#
            ),
            None,
        );
        assert_eq!(
            events.last(),
            Some(&Event::Ended(Made::Failed("a sentence".into(), cause))),
            "{code}"
        );
    }
}

#[test]
fn each_code_of_a_test_names_its_cause_and_an_unknown_one_none() {
    use wholeheard_shell::summarising::{Cause, failed};
    for (code, cause) in [
        ("provider.too_many", Cause::TooMany),
        ("provider.down", Cause::Down),
        ("provider.no_times", Cause::NoTimes),
    ] {
        assert_eq!(Cause::of(code), Some(cause));
    }
    assert_eq!(Cause::of("provider.of_a_later_version"), None);
    assert_eq!(
        failed(r#"{"event":"failed","code":"provider.down"}"#).as_deref(),
        Some("provider.down")
    );
    assert_eq!(failed(r#"{"event":"sending"}"#), None);
    assert_eq!(failed("the program's own English"), None, "a sentence is never read for a cause");
}

#[test]
fn a_test_is_read_from_its_last_lines() {
    use wholeheard_shell::summarising::{Cause, last_said, test_cause};
    let failed = r#"{"event": "failed", "code": "provider.down"}"#;
    assert_eq!(test_cause(&format!("{{\"event\": \"sending\"}}\n{failed}\n\n")), Some(Cause::Down));
    assert_eq!(
        test_cause(&format!("{failed}\n{{\"event\": \"answered\"}}\n")),
        None,
        "failed is the last line, or nothing"
    );
    assert_eq!(test_cause(r#"{"event": "failed", "code": "of a later version"}"#), None);
    assert_eq!(test_cause(""), None);
    assert_eq!(
        last_said("an earlier line\n a-program: what failed \n\n", "a-program", 1),
        "what failed"
    );
    assert_eq!(last_said("not named: what failed", "a-program", 1), "not named: what failed");
    assert_eq!(
        last_said(" \n", "a-program", "exit status: 3"),
        "a-program stopped without saying why (exit status: 3)"
    );
}
