//! What the desktop says of the shortcut, as the window shows it: the keys it granted, and
//! whether its settings, where the keys change, are GNOME's.

/// The keys a desktop says it granted; said empty, none.
pub fn keys(said: &str) -> Option<String> {
    let said = said.trim();
    (!said.is_empty()).then(|| said.to_string())
}

/// Whether the desktop running is GNOME, as `XDG_CURRENT_DESKTOP` names it — a list of names
/// separated by colons.
pub fn gnome(current_desktop: &str) -> bool {
    current_desktop.split(':').any(|name| name.eq_ignore_ascii_case("gnome"))
}
