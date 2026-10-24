//! What the log holds of a failure of the window's page (D29): its kind and the page's file it
//! was thrown in — names of the page's code —, never anything else the page could say.

use std::{
    io,
    sync::{Arc, Mutex},
};

use wholeheard_shell::log::page_failed;

/// Where the log is written, for the test to read.
#[derive(Clone, Default)]
struct Kept(Arc<Mutex<Vec<u8>>>);

impl io::Write for Kept {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The line `page_failed` logs, as the shell's log writes it.
fn logged(kind: &str, file: &str) -> serde_json::Value {
    let kept = Kept::default();
    let writer = kept.clone();
    let subscriber = tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_target(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || page_failed(kind, file, 17, 74));
    let text = String::from_utf8(kept.0.lock().unwrap().clone()).unwrap();
    serde_json::from_str(text.trim()).unwrap()
}

fn said(kind: &str, file: &str) -> (String, String) {
    let line = logged(kind, file);
    assert_eq!((&line["message"], &line["level"]), (&"shell.page_failed".into(), &"ERROR".into()));
    // Nothing more than these: no field may carry what the page sent.
    let mut fields: Vec<&str> = line.as_object().unwrap().keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(fields, ["column", "file", "kind", "level", "line", "message", "timestamp"]);
    (
        line["kind"].as_str().unwrap().to_string(),
        line["file"].as_str().unwrap().to_string(),
    )
}

#[test]
fn a_failure_of_the_page_is_its_kind_and_its_file() {
    assert_eq!(
        said("TypeError", "tauri://localhost/window.js"),
        ("TypeError".into(), "window.js".into())
    );
    assert_eq!(
        said("RangeError", "http://tauri.localhost/window.js"),
        ("RangeError".into(), "window.js".into())
    );
    let line = logged("TypeError", "tauri://localhost/window.js");
    assert_eq!((&line["line"], &line["column"]), (&17.into(), &74.into()));
}

#[test]
fn what_is_no_name_of_the_page_is_left_out() {
    // A kind made of words, a file that says more than its name: neither is a name of the page's code.
    assert_eq!(
        said("Réunion avec Paul", "tauri://localhost/window.js?q=Paul"),
        ("".into(), "".into())
    );
    assert_eq!(said("Réunion", "tauri://localhost/Réunion.js"), ("".into(), "".into()));
    assert_eq!(said("Type.Error", "tauri://localhost/window.js"), ("".into(), "window.js".into()));
    assert_eq!(said("", ""), ("".into(), "".into()));
    assert_eq!(
        said(&"E".repeat(65), &format!("tauri://localhost/{}.js", "a".repeat(62))),
        ("".into(), "".into())
    );
    assert_eq!(said("Error", "tauri://localhost/"), ("Error".into(), "".into()));
}
