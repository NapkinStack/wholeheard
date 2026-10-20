//! How long the logs are kept, and what they may weigh (ADR-0007).

use std::{
    fs,
    time::{Duration, SystemTime},
};

use wholeheard_summary::log::prune;

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
    prune(folder.path(), SystemTime::now());
    let mut left: Vec<_> = fs::read_dir(folder.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(left, ["a.jsonl", "notes.txt", "summary.2026-10-10.jsonl"]);
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
