//! A diagnostic of one meeting (D29): every program's lines of that meeting's makings, read
//! together by their id, in the order they were written — nothing of another meeting, nothing
//! that is not a log line. Written logs, no meeting of anyone.

use std::fs;

use wholeheard_shell::log::diagnostic;

const MEETING: &str = "2026-10-18_11-00";

fn line(time: &str, message: &str, trace: Option<&str>, more: &str) -> String {
    let span = trace
        .map(|id| format!(r#","span":{{"name":"making","trace":"{id}"}}"#))
        .unwrap_or_default();
    format!(r#"{{"timestamp":"{time}","level":"INFO","message":"{message}"{more}{span}}}"#)
}

#[test]
fn a_meetings_diagnostic_holds_its_makings_lines_from_every_program_in_order() {
    let folder = tempfile::tempdir().unwrap();
    let at = folder.path();
    fs::write(
        at.join("shell.2026-10-20.jsonl"),
        [
            line("2026-10-20T09:00:00Z", "shell.started", None, ""),
            line(
                "2026-10-20T09:05:00Z",
                "shell.minutes",
                Some("m1"),
                &format!(r#","session":"{MEETING}""#),
            ),
            line(
                "2026-10-20T09:06:00Z",
                "shell.minutes",
                Some("other"),
                r#","session":"2026-10-19_08-00""#,
            ),
            line(
                "2026-10-20T09:07:00Z",
                "shell.minutes_failed",
                Some("m1"),
                r#","detail":"the provider refused the key""#,
            ),
            "not a line of ours".to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    // The recording was made the day before: its lines are in that day's files.
    fs::write(
        at.join("shell.2026-10-18.jsonl"),
        line(
            "2026-10-18T11:00:01Z",
            "shell.recording_started",
            Some("r1"),
            &format!(r#","session":"{MEETING}""#),
        ),
    )
    .unwrap();
    fs::write(
        at.join("capture.2026-10-18.jsonl"),
        line("2026-10-18T11:00:00Z", "capture.started", Some("r1"), ""),
    )
    .unwrap();
    fs::write(
        at.join("summary.2026-10-20.jsonl"),
        [
            line("2026-10-20T09:06:30Z", "summary.sent", Some("m1"), ""),
            line("2026-10-20T09:06:40Z", "summary.sent", Some("other"), ""),
        ]
        .join("\n"),
    )
    .unwrap();
    fs::write(at.join("notes.txt"), line("2026-10-20T09:06:31Z", "not a log file", Some("m1"), "")).unwrap();

    let said = diagnostic(at, MEETING);
    let messages: Vec<String> = said
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("each line is JSON")["message"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        messages,
        [
            "capture.started",
            "shell.recording_started",
            "shell.minutes",
            "summary.sent",
            "shell.minutes_failed"
        ],
        "{said}"
    );
    // A meeting whose name starts another's is that meeting only.
    fs::write(
        at.join("shell.2026-10-19.jsonl"),
        line(
            "2026-10-19T08:00:00Z",
            "shell.transcript",
            Some("longer"),
            &format!(r#","session":"{MEETING}1""#),
        ),
    )
    .unwrap();
    assert!(!diagnostic(at, MEETING).contains("\"longer\""));
    // A link named as a day's file is not followed: the folder's own files only, as pruned.
    #[cfg(unix)]
    {
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("x"), line("2026-10-20T09:06:35Z", "linked", Some("m1"), "")).unwrap();
        std::os::unix::fs::symlink(elsewhere.path().join("x"), at.join("capture.2026-10-20.jsonl")).unwrap();
        assert!(!diagnostic(at, MEETING).contains("linked"));
    }
    // A meeting with no making logged has nothing to say.
    assert_eq!(diagnostic(at, "2026-01-01_00-00"), "");
    // No folder, no diagnostic — never a failure.
    assert_eq!(diagnostic(&at.join("absent"), MEETING), "");
}
