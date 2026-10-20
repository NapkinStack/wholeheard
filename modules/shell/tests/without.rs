//! A program's sentence as the shell logs it: the folder of sessions taken out, and only it.

use std::path::Path;

use wholeheard_shell::log::without;

#[test]
fn the_folder_of_sessions_is_taken_out_wherever_it_is_named() {
    let sessions = Some(Path::new("/srv/Meet"));
    assert_eq!(
        without(
            "wholeheard-transcription: /srv/Meet/2026-10-20_10-00/derived/ cannot be written",
            sessions
        ),
        "wholeheard-transcription: <sessions>/2026-10-20_10-00/derived/ cannot be written"
    );
    assert_eq!(without("/srv/Meet and /srv/Meet/x", sessions), "<sessions> and <sessions>/x");
    // Written with its last separator, it is the same folder.
    assert_eq!(without("/srv/Meet/x", Some(Path::new("/srv/Meet/"))), "<sessions>/x");
}

#[test]
fn another_name_that_starts_the_same_is_left_as_it_is() {
    let sessions = Some(Path::new("/srv/Meet"));
    assert_eq!(without("/srv/Meetings/x", sessions), "/srv/Meetings/x");
    assert_eq!(without("/srv/Meet.bak and /srv/Meet", sessions), "/srv/Meet.bak and <sessions>");
    // The root, or nothing, takes nothing out.
    assert_eq!(without("/a/b", Some(Path::new("/"))), "/a/b");
    assert_eq!(without("/a/b", None), "/a/b");
}
