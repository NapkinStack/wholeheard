//! A recording supervised, tried with a program played here: a script that says what
//! `contracts/recording/v1` describes. No sound, no device, nobody recorded.
#![cfg(unix)]

use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Mutex, time::Duration};

use summit_shell::recording::{Recording, Said, Source, read};

/// A recording by a program played here: a script given the same arguments, `record
/// <sessions>`. One at a time: a script still open for writing in one thread while another
/// starts a program cannot be run — "text file busy".
fn played(folder: &Path, script: &str) -> Recording {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap();
    let path = folder.join("summit-capture");
    fs::write(&path, format!("#!/bin/sh\nsessions=\"$2\"\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    Recording::start(&path, folder).unwrap()
}

/// Everything a recording says until it closes — a test that waits ten seconds has failed.
fn all(recording: &Recording) -> Vec<Said> {
    let mut said = Vec::new();
    while let Ok(event) = recording.said.recv_timeout(Duration::from_secs(10)) {
        said.push(event);
    }
    said
}

#[test]
fn a_recording_is_started_listened_to_and_stopped_as_its_contract_says() {
    let folder = tempfile::tempdir().unwrap();
    let mut recording = played(
        folder.path(),
        r#"echo '{"event":"started","folder":"'"$sessions"'/2026-10-13_14-05","sources":{"me":"Mic","others":"Speaker"}}'
echo 'a line that is no event'
echo '{"event":"of-a-later-version","source":"me"}'
echo '{"event":"silent","source":"others"}'
echo '{"event":"heard","source":"others"}'
read line
[ "$line" = stop ] && echo '{"event":"stopped","folder":"'"$sessions"'/2026-10-13_14-05","seconds":1912.4,"new":"field"}'"#,
    );
    let session = folder.path().join("2026-10-13_14-05");
    let before = [
        Said::Started(session.clone()),
        Said::Silent(Source::Others),
        Said::Heard(Source::Others),
    ];
    for expected in before {
        assert_eq!(recording.said.recv_timeout(Duration::from_secs(10)).unwrap(), expected);
    }
    // Nothing more until it is told to stop: it is still recording.
    assert!(recording.said.recv_timeout(Duration::from_millis(300)).is_err() && !recording.ended());
    recording.stop().unwrap();
    assert_eq!(all(&recording), [Said::Stopped(session, 1912.4)]);
}

#[test]
fn a_recording_that_ends_by_itself_is_said_to_have_failed_with_its_own_sentence() {
    let folder = tempfile::tempdir().unwrap();
    // Refused before it started: the contract's "unavailable", then one sentence and a failure.
    let recording = played(
        folder.path(),
        r#"echo '{"event":"unavailable","source":"me","reason":"the system names no default device"}'
echo 'an earlier line' >&2
echo 'summit-capture: no source could be opened: nothing to record' >&2
exit 1"#,
    );
    assert_eq!(
        all(&recording),
        [
            Said::Unavailable(Source::Me, "the system names no default device".into()),
            Said::Failed("summit-capture: no source could be opened: nothing to record".into()),
        ]
    );

    // Killed while it recorded: it says nothing of it, and that is said too.
    let recording = played(
        folder.path(),
        r#"echo '{"event":"started","folder":"'"$sessions"'/s"}'; kill -9 $$"#,
    );
    assert_eq!(
        all(&recording),
        [Said::Started(folder.path().join("s")), Said::Failed(String::new())]
    );

    // A program that is not there is an error at once, not a recording.
    assert!(Recording::start(&folder.path().join("nowhere"), folder.path()).is_err());
}

#[test]
fn a_recording_goes_on_when_the_shell_lets_go_of_it() {
    let folder = tempfile::tempdir().unwrap();
    let recording = played(
        folder.path(),
        r#"echo '{"event":"started","folder":"'"$sessions"'/s"}'
sleep 1
touch "$sessions/still-recording""#,
    );
    assert_eq!(
        recording.said.recv_timeout(Duration::from_secs(10)).unwrap(),
        Said::Started(folder.path().join("s"))
    );
    drop(recording);
    std::thread::sleep(Duration::from_millis(1800));
    assert!(
        folder.path().join("still-recording").exists(),
        "the program was stopped with the shell's hold on it"
    );
}

#[test]
fn a_line_is_read_as_the_contract_describes_it_or_passed_over() {
    assert_eq!(read(r#"{"event":"silent","source":"me"}"#), Some(Said::Silent(Source::Me)));
    assert_eq!(
        read(r#"{"event":"stopped","folder":"/s","seconds":3}"#),
        Some(Said::Stopped("/s".into(), 3.0))
    );
    for passed_over in [
        "",
        "stopped",
        "[]",
        r#"{"event":"silent"}"#,
        r#"{"event":"silent","source":"someone"}"#,
        r#"{"event":"started"}"#,
        r#"{"event":7}"#,
        r#"{"source":"me"}"#,
    ] {
        assert_eq!(read(passed_over), None, "{passed_over}");
    }
}
