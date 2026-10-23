//! What the log holds of a failure of the window's page (D29): its kind and the page's file it
//! was thrown in — names of the page's code —, never anything else the page could say.

use wholeheard_shell::log::page_failure;

#[test]
fn a_failure_of_the_page_is_its_kind_and_its_file() {
    assert_eq!(page_failure("TypeError", "tauri://localhost/window.js"), ("TypeError", "window.js"));
    assert_eq!(
        page_failure("RangeError", "http://tauri.localhost/window.js"),
        ("RangeError", "window.js")
    );
}

#[test]
fn what_is_no_name_of_the_page_is_left_out() {
    // A kind made of words, a file that says more than its name: neither is a name of the page's code.
    assert_eq!(page_failure("Réunion avec Paul", "tauri://localhost/window.js?q=Paul"), ("", ""));
    assert_eq!(page_failure("", ""), ("", ""));
    assert_eq!(
        page_failure(&"E".repeat(65), &format!("tauri://localhost/{}.js", "a".repeat(62))),
        ("", "")
    );
    assert_eq!(page_failure("Error", "tauri://localhost/"), ("Error", ""));
}
