//! What the desktop says of the shortcut, read as the window shows it.

use wholeheard_shell::desktop::{gnome, keys};

#[test]
fn keys_said_empty_are_no_keys() {
    assert_eq!(keys("Super+Alt+M").as_deref(), Some("Super+Alt+M"));
    assert_eq!(keys("  Ctrl+Alt+R "), Some("Ctrl+Alt+R".to_string()));
    assert_eq!(keys(""), None);
    assert_eq!(keys("   "), None);
}

#[test]
fn gnome_s_settings_are_offered_under_gnome_only() {
    for yes in ["GNOME", "ubuntu:GNOME", "GNOME-Classic:GNOME", "gnome"] {
        assert!(gnome(yes), "{yes}");
    }
    for no in ["KDE", "XFCE", "", "X-Cinnamon", "GNOMEISH"] {
        assert!(!gnome(no), "{no}");
    }
}

#[test]
fn keys_said_in_gtk_s_notation_are_shown_as_a_person_reads_them() {
    // GNOME's portal describes the keys it granted as a sentence in English around GTK's own
    // notation; the window shows them as D11's fourth board does, whatever the language (#312).
    assert_eq!(keys("Press <Alt><Super>m").as_deref(), Some("Super + Alt + M"));
    assert_eq!(keys("<Primary><Shift>r").as_deref(), Some("Ctrl + Shift + R"));
    assert_eq!(keys("Press <Control>F9").as_deref(), Some("Ctrl + F9"));
    assert_eq!(keys("<Super>space").as_deref(), Some("Super + Space"));
    // Keys said otherwise are shown as said.
    assert_eq!(keys("Meta+Alt+M").as_deref(), Some("Meta+Alt+M"));
}

#[test]
fn keys_are_read_out_of_the_portal_s_sentence_in_any_language_one_or_two_of_them() {
    // GNOME's portal says "Press %s", or "Press %s or %s", in its own language (#347's verifier).
    assert_eq!(keys("Appuyer sur <Alt><Super>m").as_deref(), Some("Super + Alt + M"));
    assert_eq!(
        keys("Press <Alt><Super>m or <Control>F9").as_deref(),
        Some("Super + Alt + M / Ctrl + F9")
    );
    assert_eq!(
        keys("Appuyer sur <Alt><Super>m ou <Primary>Page_Up").as_deref(),
        Some("Super + Alt + M / Ctrl + Page Up")
    );
    assert_eq!(keys("<Mod4><Mod1>r").as_deref(), Some("Super + Alt + R"));
    assert_eq!(keys("<Control><Super>x").as_deref(), Some("Super + Ctrl + X"));
    assert_eq!(keys("<Meta><Hyper>x").as_deref(), Some("Meta + Hyper + X"));
    // A modifier GTK has and the board does not name: the word shown as said.
    assert_eq!(keys("<Release>m").as_deref(), Some("<Release>m"));
    assert_eq!(keys("<Alt>m to start").as_deref(), Some("Alt + M"));
    // Not GTK's notation: shown as said, a lone "<" included.
    assert_eq!(keys("Meta+<").as_deref(), Some("Meta+<"));
    assert_eq!(keys("LOGO+ALT+m").as_deref(), Some("LOGO+ALT+m"));
}
