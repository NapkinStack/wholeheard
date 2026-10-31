//! The sessions as the window lists them, read from folders made here.

use std::{fs, path::Path};

use wholeheard_shell::sessions::{self, State, instant};

fn session(folder: &Path, json: &str, files: &[(&str, usize)]) {
    fs::create_dir_all(folder).unwrap();
    fs::write(folder.join("session.json"), json).unwrap();
    for (name, bytes) in files {
        let path = folder.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A transcript of no size asked is a whole one: its header, then its last line.
        if *name == "transcript.jsonl" && *bytes == 0 {
            fs::write(path, WHOLE).unwrap();
        } else {
            fs::write(path, vec![0u8; *bytes]).unwrap();
        }
    }
}

const WHOLE: &str = "{\"type\":\"header\",\"contract\":\"transcript/v1\",\"engine\":\"e\",\"model\":\"m\",\"language\":null,\"context\":null}\n{\"type\":\"complete\",\"duration\":1.0}\n";

#[test]
fn an_instant_is_read_with_its_offset_as_the_contract_writes_it() {
    assert_eq!(instant("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(instant("2026-10-13T14:05:12+02:00"), Some(1_791_893_112));
    assert_eq!(instant("2024-02-29T12:00:00.250+00:00"), Some(1_709_208_000));
    assert_eq!(instant("1969-12-31T23:00:00-05:30"), Some(16_200));
    // The other offset forms of ISO 8601 are the same instant.
    assert_eq!(instant("2026-10-13T14:05:12+0200"), Some(1_791_893_112));
    assert_eq!(instant("2026-10-13T14:05:12+02"), Some(1_791_893_112));
    for wrong in [
        "",
        "2026-10-13",
        "2026-10-13 14:05:12+02:00",
        "2026-10-13T14:05:12",
        "2026-10-13T14:05:12+02:00xyz",
        "2026-10-13T14:05:12+02x00",
        "2026-10-13T14:05:12+2:00",
        "2026-10-13T14:05:12++200",
        "2026-10-13T14:05:1299+02:00",
        "yesterday",
    ] {
        assert_eq!(instant(wrong), None, "{wrong}");
    }
}

#[test]
fn each_session_says_its_day_length_state_what_it_holds_and_weighs_newest_first() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    // The first form, stopped, with its transcript and its minutes.
    session(
        &root.join("2026-10-12_09-30"),
        r#"{"contract":"session-audio/v1","started":"2026-10-12T09:30:00+02:00","stopped":"2026-10-12T10:12:00+02:00","sources":{}}"#,
        &[("me.wav", 100), ("others.wav", 100), ("transcript.jsonl", 7), ("minutes.md", 5)],
    );
    // The second form, stopped, with what was made from its tracks: sound too.
    session(
        &root.join("2026-10-13_12-01"),
        r#"{"contract":"session-audio/v2","started":"2026-10-13T12:01:00+02:00","stopped":"2026-10-13T12:17:05+02:00","tracks":{}}"#,
        &[
            ("local.wav", 300),
            ("remote.wav", 300),
            ("clock.jsonl", 9),
            ("derived/local.wav", 50),
        ],
    );
    // Its audio freed: the transcript is all it holds.
    session(
        &root.join("2026-10-06_11-00"),
        r#"{"contract":"session-audio/v2","started":"2026-10-06T11:00:00+02:00","stopped":"2026-10-06T11:35:00+02:00","tracks":{}}"#,
        &[("transcript.jsonl", 11)],
    );
    // Never stopped: one is being recorded now, the other was left so.
    let now = root.join("2026-10-17_09-00");
    session(
        &now,
        r#"{"contract":"session-audio/v2","started":"2026-10-17T09:00:00+02:00","tracks":{}}"#,
        &[("local.wav", 10)],
    );
    session(
        &root.join("2026-10-16_08-00"),
        r#"{"contract":"session-audio/v2","started":"2026-10-16T08:00:00+02:00","tracks":{}}"#,
        &[("local.wav", 20)],
    );
    // Its folder named before the others, its instant after them, an offset west of here: listed by
    // its instant. Its sound, written in capitals, is sound.
    session(
        &root.join("2026-10-01_00-00"),
        r#"{"contract":"session-audio/v2","started":"2026-10-17T05:00:00-05:00","stopped":"2026-10-17T05:01:00-05:00","tracks":{}}"#,
        &[("LOCAL.WAV", 40)],
    );
    // A start that is no instant: listed, last, with no instant.
    session(
        &root.join("2026-10-02_00-00"),
        r#"{"contract":"session-audio/v2","started":"yesterday","tracks":{}}"#,
        &[],
    );
    // Not sessions: no session.json, a form this version does not read, a file that is not JSON, a
    // stop with no start.
    fs::create_dir_all(root.join("notes")).unwrap();
    session(
        &root.join("2026-10-15_10-00"),
        r#"{"contract":"session-audio/v9","started":"2026-10-15T10:00:00+02:00"}"#,
        &[],
    );
    session(&root.join("2026-10-14_10-00"), "not json", &[]);
    session(
        &root.join("2026-10-13_10-00"),
        r#"{"contract":"session-audio/v2","stopped":"2026-10-13T11:00:00+02:00"}"#,
        &[],
    );
    fs::write(root.join("loose.wav"), [0u8; 4]).unwrap();

    let listed = sessions::list(root, Some(&now));
    let names: Vec<_> = listed.iter().map(|s| s.folder.file_name().unwrap().to_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "2026-10-01_00-00",
            "2026-10-17_09-00",
            "2026-10-16_08-00",
            "2026-10-13_12-01",
            "2026-10-12_09-30",
            "2026-10-06_11-00",
            "2026-10-02_00-00"
        ]
    );

    let [west, recording, interrupted, second, first, freed, undated] = &listed[..] else {
        unreachable!()
    };
    assert_eq!((west.seconds, west.audio), (Some(60.0), 40));
    assert_eq!((undated.instant, undated.seconds, undated.state), (None, None, State::Interrupted));
    assert_eq!((recording.state, recording.seconds), (State::Recording, None));
    assert_eq!((interrupted.state, interrupted.seconds), (State::Interrupted, None));
    assert_eq!(
        (second.state, second.seconds, second.transcript, second.minutes),
        (State::Done, Some(965.0), false, false)
    );
    assert_eq!(
        (second.audio, second.text),
        (650, 9 + fs::metadata(second.folder.join("session.json")).unwrap().len())
    );
    assert_eq!(
        (first.seconds, first.transcript, first.minutes, first.audio),
        (Some(2520.0), true, true, 200)
    );
    assert_eq!((freed.audio, freed.transcript), (0, true));
    assert_eq!(first.started, "2026-10-12T09:30:00+02:00");
    assert_eq!(first.instant, Some(1_791_790_200));

    // Nobody records: the session never stopped is interrupted too.
    let quiet = sessions::list(root, None);
    assert_eq!(quiet[1].state, State::Interrupted);
}

#[test]
fn a_folder_that_is_not_there_lists_nothing() {
    let folder = tempfile::tempdir().unwrap();
    assert!(sessions::list(&folder.path().join("missing"), None).is_empty());
}

#[test]
fn a_session_is_named_by_its_folder_alone_never_by_a_path() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().join("sessions");
    session(
        &root.join("2026-10-12_09-30"),
        r#"{"contract":"session-audio/v2","started":"2026-10-12T09:30:00+02:00","tracks":{}}"#,
        &[],
    );
    fs::create_dir_all(root.join("notes")).unwrap();
    // Beside the sessions' folder, a folder that holds a session too: never reached from it.
    session(
        &folder.path().join("elsewhere"),
        r#"{"contract":"session-audio/v2","started":"2026-10-12T09:30:00+02:00","tracks":{}}"#,
        &[],
    );
    assert_eq!(sessions::folder(&root, "2026-10-12_09-30"), Some(root.join("2026-10-12_09-30")));
    let elsewhere = folder.path().join("elsewhere");
    for wrong in [
        "",
        ".",
        "..",
        "../elsewhere",
        "2026-10-12_09-30/..",
        "a/b",
        "notes",
        "missing",
        elsewhere.to_str().unwrap(),
    ] {
        assert_eq!(sessions::folder(&root, wrong), None, "{wrong}");
    }
}

#[test]
fn the_sound_of_sessions_stopped_more_than_the_days_kept_is_what_goes() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path();
    // Stopped on 2026-10-01 at 10:00 and on 2026-10-15 at 10:00 (UTC); one never stopped.
    session(
        &root.join("old"),
        r#"{"contract":"session-audio/v2","started":"2026-10-01T09:00:00Z","stopped":"2026-10-01T10:00:00Z","tracks":{}}"#,
        &[
            ("local.wav", 10),
            ("remote.WAV", 10),
            ("derived/local.wav", 5),
            ("transcript.jsonl", 0),
            ("minutes.md", 3),
        ],
    );
    session(
        &root.join("new"),
        r#"{"contract":"session-audio/v2","started":"2026-10-15T09:00:00Z","stopped":"2026-10-15T10:00:00Z","tracks":{}}"#,
        &[("local.wav", 10), ("transcript.jsonl", 0)],
    );
    session(
        &root.join("cut"),
        r#"{"contract":"session-audio/v2","started":"2026-09-01T09:00:00Z","tracks":{}}"#,
        &[("local.wav", 10)],
    );
    session(
        &root.join("undated"),
        r#"{"contract":"session-audio/v2","started":"long ago","tracks":{}}"#,
        &[("local.wav", 10)],
    );
    let now = instant("2026-10-17T10:00:00Z").unwrap();
    let mut gone = sessions::expired_audio(root, 14, now);
    gone.sort();
    assert_eq!(
        gone,
        [
            root.join("old/derived/local.wav"),
            root.join("old/local.wav"),
            root.join("old/remote.WAV")
        ]
    );
    // Exactly the days kept: it goes; a day less, it stays.
    assert_eq!(sessions::expired_audio(root, 16, now).len(), 3);
    assert!(sessions::expired_audio(root, 17, now).is_empty());
    // Counted from its stop, not its start: between the two, it stays.
    let between = instant("2026-10-17T09:30:00Z").unwrap();
    assert!(sessions::expired_audio(root, 16, between).is_empty());
    // Two days kept: the other stopped session goes too; never the one without its end.
    assert_eq!(sessions::expired_audio(root, 2, now).len(), 4);

    // Stopped before it started, a clock gone back: it keeps its sound. So does one never transcribed.
    session(
        &root.join("backwards"),
        r#"{"contract":"session-audio/v2","started":"2026-10-16T09:00:00Z","stopped":"1970-01-01T00:00:00Z","tracks":{}}"#,
        &[("local.wav", 10), ("transcript.jsonl", 0)],
    );
    session(
        &root.join("untranscribed"),
        r#"{"contract":"session-audio/v2","started":"2026-09-01T09:00:00Z","stopped":"2026-09-01T10:00:00Z","tracks":{}}"#,
        &[("local.wav", 10)],
    );
    assert_eq!(sessions::expired_audio(root, 2, now).len(), 4);

    // A transcript that died after its header keeps the sound; so does a session never stopped,
    // even transcribed from the menu.
    session(
        &root.join("died"),
        r#"{"contract":"session-audio/v2","started":"2026-09-01T09:00:00Z","stopped":"2026-09-01T10:00:00Z","tracks":{}}"#,
        &[("local.wav", 10)],
    );
    fs::write(root.join("died/transcript.jsonl"), WHOLE.lines().next().unwrap().to_string() + "\n").unwrap();
    session(
        &root.join("killed"),
        r#"{"contract":"session-audio/v2","started":"2026-09-01T09:00:00Z","tracks":{}}"#,
        &[("local.wav", 10), ("transcript.jsonl", 0)],
    );
    assert_eq!(sessions::expired_audio(root, 2, now).len(), 4);
}

#[cfg(unix)]
#[test]
fn a_link_in_a_session_never_sends_what_it_points_to() {
    let folder = tempfile::tempdir().unwrap();
    let root = folder.path().join("sessions");
    let outside = folder.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("keep.wav"), [0u8; 4]).unwrap();
    session(
        &root.join("old"),
        r#"{"contract":"session-audio/v2","started":"2026-10-01T09:00:00Z","stopped":"2026-10-01T10:00:00Z","tracks":{}}"#,
        &[("local.wav", 10), ("transcript.jsonl", 0)],
    );
    std::os::unix::fs::symlink(outside.join("keep.wav"), root.join("old/linked.wav")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("old/derived")).unwrap();
    let now = instant("2026-10-17T10:00:00Z").unwrap();
    assert_eq!(sessions::expired_audio(&root, 1, now), [root.join("old/local.wav")]);
}

#[test]
fn a_file_set_aside_never_writes_over_another() {
    let folder = tempfile::tempdir().unwrap();
    let minutes = folder.path().join("minutes.md");
    std::fs::write(&minutes, "A").unwrap();
    // Copied aside twice in the same second: two names, neither written over.
    let first = sessions::set_aside(&minutes, false).unwrap();
    std::fs::write(&minutes, "B").unwrap();
    let second = sessions::set_aside(&minutes, false).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        (std::fs::read_to_string(&first).unwrap(), std::fs::read_to_string(&second).unwrap()),
        ("A".into(), "B".into())
    );
    assert_eq!(std::fs::read_to_string(&minutes).unwrap(), "B");
    let name = first.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("minutes-before-") && name.ends_with(".md"), "{name}");
    // Moved aside: the file itself goes, under a name of its own.
    let transcript = folder.path().join("transcript.jsonl");
    std::fs::write(&transcript, "T").unwrap();
    let moved = sessions::set_aside(&transcript, true).unwrap();
    assert!(!transcript.exists() && std::fs::read_to_string(&moved).unwrap() == "T");
    assert!(moved.file_name().unwrap().to_string_lossy().ends_with(".jsonl"));
    // Nothing there: nothing set aside, and said so; there and unreadable, refused, nothing left.
    assert_eq!(
        sessions::set_aside(&folder.path().join("absent.md"), false).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&minutes, std::fs::Permissions::from_mode(0o000)).unwrap();
        let refused = sessions::set_aside(&minutes, false);
        std::fs::set_permissions(&minutes, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(refused.is_err());
        assert_eq!(folder.path().read_dir().unwrap().count(), 4, "a copy was left");
    }
}

#[test]
fn a_meeting_s_audio_is_every_sound_in_its_folder_and_nothing_else() {
    let folder = tempfile::tempdir().unwrap();
    let meeting = folder.path();
    fs::create_dir_all(meeting.join("derived")).unwrap();
    for name in [
        "local.wav",
        "remote.WAV",
        "derived/local.wav",
        "transcript.jsonl",
        "minutes.md",
        "session.json",
    ] {
        fs::write(meeting.join(name), "x").unwrap();
    }
    let mut found = sessions::audio_of(meeting);
    found.sort();
    assert_eq!(
        found,
        [
            meeting.join("derived/local.wav"),
            meeting.join("local.wav"),
            meeting.join("remote.WAV")
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_folder_of_sessions_is_writable_or_said_not_to_be_and_nothing_is_left_in_it() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    // Not there yet: made, written, left empty.
    let new = root.path().join("Wholeheard");
    assert!(wholeheard_shell::sessions::writable(&new).is_ok());
    assert_eq!(std::fs::read_dir(&new).unwrap().count(), 0);
    // There, and refusing to be written: said, before any recorder is started.
    let locked = root.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let refused = wholeheard_shell::sessions::writable(&locked);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(refused.is_err());
    // Under a file: not made.
    std::fs::write(root.path().join("a-file"), "").unwrap();
    assert!(wholeheard_shell::sessions::writable(&root.path().join("a-file").join("Wholeheard")).is_err());
}

#[test]
fn a_file_set_aside_for_a_making_is_put_back_when_it_fails_and_let_go_when_it_is_made() {
    let folder = tempfile::tempdir().unwrap();
    let transcript = folder.path().join("transcript.jsonl");
    // The making fails, a half of its own written: the one set aside goes back in its place.
    fs::write(&transcript, "before").unwrap();
    let aside = sessions::set_aside(&transcript, true).unwrap();
    fs::write(&transcript, "half").unwrap();
    let mut disposed = Vec::new();
    let kept = sessions::settle(&transcript, &aside, false, |file| {
        disposed.push(file.to_path_buf());
        Ok(())
    });
    assert_eq!(kept, None);
    assert!(disposed.is_empty());
    assert_eq!(fs::read_to_string(&transcript).unwrap(), "before");
    assert!(!aside.exists());
    // Nothing written at all: put back the same.
    let aside = sessions::set_aside(&transcript, true).unwrap();
    assert_eq!(sessions::settle(&transcript, &aside, false, |_| Ok(())), None);
    assert_eq!(fs::read_to_string(&transcript).unwrap(), "before");
    // Made whole: the new one stays, the one set aside is let go — to the trash, in the shell.
    let aside = sessions::set_aside(&transcript, true).unwrap();
    fs::write(&transcript, "after").unwrap();
    let kept = sessions::settle(&transcript, &aside, true, |file| {
        disposed.push(file.to_path_buf());
        fs::remove_file(file)
    });
    assert_eq!(kept, None);
    assert_eq!(disposed, std::slice::from_ref(&aside));
    assert_eq!(fs::read_to_string(&transcript).unwrap(), "after");
    // Let go refused: it stays beside, and that is said — not put back.
    let aside = sessions::set_aside(&transcript, true).unwrap();
    fs::write(&transcript, "again").unwrap();
    let kept = sessions::settle(&transcript, &aside, true, |_| Err(std::io::Error::other("no trash")));
    assert_eq!(kept, Some(false));
    assert!(aside.exists());
    assert_eq!(fs::read_to_string(&transcript).unwrap(), "again");
    // Put back refused — gone meanwhile —: said where it is, not put back.
    fs::remove_file(&aside).unwrap();
    assert_eq!(sessions::settle(&transcript, &aside, false, |_| Ok(())), Some(true));
}

#[test]
fn a_folder_that_cannot_be_read_is_no_empty_list_and_one_not_made_yet_is() {
    let root = tempfile::tempdir().unwrap();
    assert!(!sessions::unreadable(&root.path().join("not made yet")));
    assert!(!sessions::unreadable(root.path()));
    // A file where the folder should be: what a read refuses, whoever runs the test.
    let file = root.path().join("a file");
    std::fs::write(&file, "").unwrap();
    assert!(sessions::unreadable(&file));
}
