//! What the shell's own program logs reaches the file: its lines are named `wholeheard`, its
//! library's `wholeheard_shell`, and a level asked reaches both — a library's never.

use std::fs;

#[test]
fn the_programs_lines_and_its_librarys_are_written_and_no_other() {
    let logs = tempfile::tempdir().unwrap();
    // Set before anything reads them: this test is alone in its program.
    unsafe {
        std::env::set_var("WHOLEHEARD_LOG_DIR", logs.path());
        std::env::remove_var("WHOLEHEARD_LOG");
    }
    let run = wholeheard_shell::log::start("shell");
    tracing::info!(target: "wholeheard", "from.the.program");
    tracing::info!(target: "wholeheard_shell::recording", "from.the.library");
    tracing::info!(target: "ureq", "from.another.crate");
    drop(run);
    let file = fs::read_dir(logs.path()).unwrap().next().unwrap().unwrap().path();
    let text = fs::read_to_string(file).unwrap();
    assert!(text.contains("from.the.program") && text.contains("from.the.library"), "{text}");
    assert!(!text.contains("from.another.crate"), "{text}");
}
