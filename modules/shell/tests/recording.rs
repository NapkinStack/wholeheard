//! A recording supervised, tried with a program played here: a script that says what
//! `contracts/recording/v1` describes. No sound, no device, nobody recorded.
#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{Mutex, mpsc::Receiver},
    time::Duration,
};

use wholeheard_shell::recording::{Devices, Recording, Said, Source, read};

/// A recording by a program played here: a script given the same arguments, `record
/// <sessions>`. One at a time: a script still open for writing in one thread while another
/// starts a program cannot be run — "text file busy".
fn played(folder: &Path, script: &str) -> (Recording, Receiver<Said>) {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _held = ONE_AT_A_TIME.lock().unwrap();
    let path = folder.join("wholeheard-capture");
    fs::write(&path, format!("#!/bin/sh\nsessions=\"$2\"\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    Recording::start(&path, folder).unwrap()
}

/// Everything a recording says until it closes — a test that waits ten seconds has failed.
fn all(heard: &Receiver<Said>) -> Vec<Said> {
    let mut said = Vec::new();
    while let Ok(event) = heard.recv_timeout(Duration::from_secs(10)) {
        said.push(event);
    }
    said
}

#[test]
fn a_recording_is_started_listened_to_and_stopped_as_its_contract_says() {
    let folder = tempfile::tempdir().unwrap();
    let (mut recording, heard) = played(
        folder.path(),
        r#"[ "$1" = record ] || exit 9
echo '{"event":"started","folder":"'"$sessions"'/2026-10-13_14-05","sources":{"me":"Mic","others":"Speaker"}}'
echo 'a line that is no event'
printf 'neither is one that is not text: caf\351\n'
printf 'nor does one end the listening on the other stream: caf\351\n' >&2
echo '{"event":"of-a-later-version","source":"me"}'
echo '{"event":"silent","source":"others"}'
echo '{"event":"heard","source":"others"}'
echo '{"event":"level","source":"me","level":0.42}'
echo '{"event":"level","source":"others"}'
echo '{"event":"level","source":"others","level":1.5}'
read line
[ "$line" = stop ] && echo '{"event":"stopped","folder":"'"$sessions"'/2026-10-13_14-05","seconds":1912.4,"new":"field"}'
echo '{"event":"silent","source":"me"}'"#,
    );
    let session = folder.path().join("2026-10-13_14-05");
    let before = [
        Said::Started(session.clone(), Some("Mic".into()), Some("Speaker".into())),
        Said::Silent(Source::Others),
        Said::Heard(Source::Others),
        // A level read; one without its level is no event.
        Said::Level(Source::Me, 0.42),
        // The meeting's own, and never louder than the loudest.
        Said::Level(Source::Others, 1.0),
    ];
    for expected in before {
        assert_eq!(heard.recv_timeout(Duration::from_secs(10)).unwrap(), expected);
    }
    // Nothing more until it is told to stop: it is still recording.
    assert!(heard.recv_timeout(Duration::from_millis(300)).is_err() && !recording.ended());
    recording.stop().unwrap();
    assert_eq!(all(&heard), [Said::Stopped(session, 1912.4)]);
}

#[test]
fn a_recording_that_ends_by_itself_is_said_to_have_failed_with_its_own_sentence() {
    let folder = tempfile::tempdir().unwrap();
    // Refused before it started: the contract's "unavailable", then one sentence and a failure.
    let (_recording, heard) = played(
        folder.path(),
        r#"echo '{"event":"unavailable","source":"me","reason":"the system names no default device"}'
head -c 3000000 /dev/zero | tr '\0' x >&2
printf '\nan earlier line, and one that is not text: caf\351\n' >&2
echo 'wholeheard-capture: no source could be opened: nothing to record' >&2
exit 1"#,
    );
    assert_eq!(
        all(&heard),
        [
            Said::Unavailable(Source::Me, "the system names no default device".into()),
            Said::Failed("wholeheard-capture: no source could be opened: nothing to record".into()),
        ]
    );

    // Killed while it recorded: it says nothing of it, and that is said too.
    let (_recording, heard) = played(
        folder.path(),
        r#"echo '{"event":"started","folder":"'"$sessions"'/s","sources":{"me":"Mic","others":"Speaker"}}'; kill -9 $$"#,
    );
    assert_eq!(
        all(&heard),
        [
            Said::Started(folder.path().join("s"), Some("Mic".into()), Some("Speaker".into())),
            Said::Failed(String::new())
        ]
    );

    // A program that is not there is an error at once, not a recording.
    assert!(Recording::start(&folder.path().join("nowhere"), folder.path()).is_err());
}

#[test]
fn a_recording_goes_on_when_the_shell_lets_go_of_it() {
    let folder = tempfile::tempdir().unwrap();
    let (recording, heard) = played(
        folder.path(),
        r#"echo '{"event":"started","folder":"'"$sessions"'/s","sources":{"me":"Mic","others":"Speaker"}}'
sleep 1
touch "$sessions/still-recording""#,
    );
    assert_eq!(
        heard.recv_timeout(Duration::from_secs(10)).unwrap(),
        Said::Started(folder.path().join("s"), Some("Mic".into()), Some("Speaker".into()))
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
    // Exact zeros are no quiet room (#361): a silence that says nothing came is told apart.
    assert_eq!(read(r#"{"event":"silent","source":"me","nothing":true}"#), Some(Said::Nothing));
    assert_eq!(
        read(r#"{"event":"silent","source":"me","nothing":false}"#),
        Some(Said::Silent(Source::Me))
    );
    // An output plays exact zeros whenever nothing plays: the meeting's side stays a silence.
    assert_eq!(
        read(r#"{"event":"silent","source":"others","nothing":true}"#),
        Some(Said::Silent(Source::Others))
    );
    // The microphone, as the system names it; a default named for itself, or none, says nothing.
    let long = "Mic ".repeat(30);
    let cut = format!("{}…", &long[..80]);
    for (me, named) in [
        ("Headset Mic", Some("Headset Mic")),
        // A tab and a line break, as JSON escapes them.
        (r"  Headset\t\nMic ", Some("Headset Mic")),
        (long.as_str(), Some(cut.as_str())),
        ("default_input", None),
        ("unavailable", None),
        ("unnamed", None),
        ("", None),
        ("   ", None),
    ] {
        let line = format!(r#"{{"event":"started","folder":"/s","sources":{{"me":"{me}","others":"Speaker"}}}}"#);
        assert_eq!(
            read(&line),
            Some(Said::Started("/s".into(), named.map(str::to_string), Some("Speaker".into()))),
            "{me}"
        );
    }
    // Another device, named as `started` names one; none left (D31).
    assert_eq!(
        read(r#"{"event":"device","source":"me","device":"  Headset\tMic"}"#),
        Some(Said::Moved(Source::Me, Some("Headset Mic".into())))
    );
    assert_eq!(
        read(r#"{"event":"device","source":"others","device":"unnamed"}"#),
        Some(Said::Moved(Source::Others, None))
    );
    assert_eq!(
        read(r#"{"event":"device","source":"others","device":null}"#),
        Some(Said::Lost(Source::Others))
    );
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
        r#"{"event":"device","source":"me"}"#,
        r#"{"event":"device","source":"me","device":7}"#,
        r#"{"event":"device","device":"Mic"}"#,
        r#"{"source":"me"}"#,
    ] {
        assert_eq!(read(passed_over), None, "{passed_over}");
    }
}

#[test]
fn what_each_side_is_recorded_from_is_followed() {
    // The output too, as `started` names it (D33).
    assert_eq!(
        read(r#"{"event":"started","folder":"/s","sources":{"me":"Mic","others":"  Speaker\tout"}}"#),
        Some(Said::Started("/s".into(), Some("Mic".into()), Some("Speaker out".into())))
    );
    let mut devices = Devices::default();
    assert_eq!(devices.names(), [None, None]);
    devices.heard(&Said::Started("/s".into(), Some("Mic".into()), Some("Speaker".into())));
    assert_eq!(devices.names(), [Some("Mic".into()), Some("Speaker".into())]);
    devices.heard(&Said::Moved(Source::Me, Some("AirPods Pro".into())));
    devices.heard(&Said::Level(Source::Me, 0.5));
    devices.heard(&Said::Silent(Source::Others));
    assert_eq!(devices.names(), [Some("AirPods Pro".into()), Some("Speaker".into())]);
    devices.heard(&Said::Lost(Source::Others));
    assert_eq!(devices.names(), [Some("AirPods Pro".into()), None]);
    devices.heard(&Said::Moved(Source::Others, None));
    devices.heard(&Said::Unavailable(Source::Me, "no microphone".into()));
    assert_eq!(devices.names(), [None, None]);
}
