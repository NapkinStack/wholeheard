//! How long the logs are kept, and what they may weigh (ADR-0007).

use std::{
    fs,
    time::{Duration, SystemTime},
};

use wholeheard_shell::log::prune;

const DAY: Duration = Duration::from_secs(24 * 3600);

fn written(folder: &std::path::Path, name: &str, size: u64, ago: Duration) {
    let file = fs::File::create(folder.join(name)).unwrap();
    file.set_len(size).unwrap();
    file.set_modified(SystemTime::now() - ago).unwrap();
}

#[test]
fn a_day_file_older_than_fourteen_days_goes_and_nothing_else() {
    let folder = tempfile::tempdir().unwrap();
    written(folder.path(), "summary.2026-09-01.jsonl", 10, 15 * DAY);
    written(folder.path(), "shell.2026-09-01.jsonl", 10, 15 * DAY);
    written(folder.path(), "summary.2026-10-10.jsonl", 10, 13 * DAY);
    written(folder.path(), "notes.txt", 10, 90 * DAY);
    written(folder.path(), "a.jsonl", 10, 90 * DAY);
    // Named nearly as a day file, and not one: the user's, if the folder is theirs.
    written(folder.path(), "export.abcdefghij.jsonl", 10, 90 * DAY);
    written(folder.path(), "My meeting.2026-09-01.jsonl", 10, 90 * DAY);
    written(folder.path(), "notes.2026-09-01.jsonl.bak", 10, 90 * DAY);
    // A link named as a day file is not a file: neither it nor what it leads to goes.
    let elsewhere = tempfile::tempdir().unwrap();
    written(elsewhere.path(), "kept.txt", 10, 90 * DAY);
    std::os::unix::fs::symlink(elsewhere.path().join("kept.txt"), folder.path().join("shell.2026-08-01.jsonl")).unwrap();
    prune(folder.path(), SystemTime::now());
    let mut left: Vec<_> = fs::read_dir(folder.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(
        left,
        [
            "My meeting.2026-09-01.jsonl",
            "a.jsonl",
            "export.abcdefghij.jsonl",
            "notes.2026-09-01.jsonl.bak",
            "notes.txt",
            "shell.2026-08-01.jsonl",
            "summary.2026-10-10.jsonl"
        ]
    );
    assert!(elsewhere.path().join("kept.txt").exists());
}

#[test]
fn the_oldest_go_until_the_folder_weighs_under_fifty_megabytes() {
    let folder = tempfile::tempdir().unwrap();
    let megabytes = 1024 * 1024;
    written(folder.path(), "transcription.2026-10-17.jsonl", 30 * megabytes, 3 * DAY);
    written(folder.path(), "summary.2026-10-18.jsonl", 15 * megabytes, 2 * DAY);
    written(folder.path(), "shell.2026-10-19.jsonl", 15 * megabytes, DAY);
    prune(folder.path(), SystemTime::now());
    let mut left: Vec<_> = fs::read_dir(folder.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(left, ["shell.2026-10-19.jsonl", "summary.2026-10-18.jsonl"]);
}
