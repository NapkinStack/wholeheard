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
