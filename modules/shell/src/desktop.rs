//! What the desktop says of the shortcut, as the window shows it: the keys it granted, and
//! whether its settings, where the keys change, are GNOME's.

/// The keys a desktop says it granted, as a person reads them; said empty, none. GNOME's portal
/// says them in GTK's notation inside an English sentence — "Press <Alt><Super>m" —: they are
/// shown as GNOME's settings show them, "Super+Alt+M". Keys said otherwise are shown as said.
pub fn keys(said: &str) -> Option<String> {
    let said = said.trim();
    let Some(start) = said.find('<') else {
        return (!said.is_empty()).then(|| said.to_string());
    };
    let (mut held, mut rest) = (Vec::new(), &said[start..]);
    while let Some(after) = rest.strip_prefix('<') {
        let (name, next) = after.split_once('>')?;
        held.push(match name.to_ascii_lowercase().as_str() {
            "primary" | "control" | "ctrl" => "Ctrl",
            "alt" | "mod1" => "Alt",
            "shift" => "Shift",
            "super" | "meta" | "hyper" => "Super",
            _ => return Some(said.to_string()),
        });
        rest = next;
    }
    let key = rest.trim();
    if key.is_empty() {
        return Some(said.to_string());
    }
    let mut key: Vec<char> = key.chars().collect();
    key[0] = key[0].to_ascii_uppercase();
    // As GNOME's settings order them: Super first, then Ctrl, Alt, Shift.
    let mut shown: Vec<&str> = ["Super", "Ctrl", "Alt", "Shift"]
        .into_iter()
        .filter(|name| held.contains(name))
        .collect();
    let key: String = key.into_iter().collect();
    shown.push(&key);
    Some(shown.join("+"))
}

/// Whether the desktop running is GNOME, as `XDG_CURRENT_DESKTOP` names it — a list of names
/// separated by colons.
pub fn gnome(current_desktop: &str) -> bool {
    current_desktop.split(':').any(|name| name.eq_ignore_ascii_case("gnome"))
}
