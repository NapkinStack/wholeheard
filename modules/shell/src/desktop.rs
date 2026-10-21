//! What the desktop says of the shortcut, as the window shows it: the keys it granted, and
//! whether its settings, where the keys change, are GNOME's.

/// The keys a desktop says it granted, as a person reads them; said empty, none. GNOME's portal
/// says them in GTK's notation inside a sentence of its own language — "Press <Alt><Super>m",
/// "Appuyer sur <Alt><Super>m ou <Control>F9" —: each one found is shown as D11's fourth board
/// shows it, "Super + Alt + M", two of them apart by a slash. Keys said otherwise are shown as said.
pub fn keys(said: &str) -> Option<String> {
    let said = said.trim();
    if said.is_empty() {
        return None;
    }
    let found: Vec<String> = said.split_whitespace().filter_map(accelerator).collect();
    Some(if found.is_empty() { said.to_string() } else { found.join(" / ") })
}

/// One word in GTK's notation — modifiers in angle brackets, then a key —, as a person reads it;
/// any other word, none.
fn accelerator(word: &str) -> Option<String> {
    let (mut held, mut rest) = (Vec::new(), word);
    while let Some(after) = rest.strip_prefix('<') {
        let (name, next) = after.split_once('>')?;
        held.push(match name.to_ascii_lowercase().as_str() {
            "primary" | "control" | "ctrl" => "Ctrl",
            "alt" | "mod1" => "Alt",
            "shift" => "Shift",
            "super" | "mod4" => "Super",
            "meta" => "Meta",
            "hyper" => "Hyper",
            _ => return None,
        });
        rest = next;
    }
    if held.is_empty() || rest.is_empty() || rest.contains(['<', '>']) {
        return None;
    }
    // A key's name as GTK writes it — "m", "F9", "Page_Up" —, as it reads.
    let mut key: Vec<char> = rest.replace('_', " ").chars().collect();
    key[0] = key[0].to_ascii_uppercase();
    let key: String = key.into_iter().collect();
    // As the board orders them: Super first, then Ctrl, Alt, Shift, Meta, Hyper.
    let mut shown: Vec<&str> = ["Super", "Ctrl", "Alt", "Shift", "Meta", "Hyper"]
        .into_iter()
        .filter(|name| held.contains(name))
        .collect();
    shown.push(&key);
    Some(shown.join(" + "))
}

/// Whether the desktop running is GNOME, as `XDG_CURRENT_DESKTOP` names it — a list of names
/// separated by colons.
pub fn gnome(current_desktop: &str) -> bool {
    current_desktop.split(':').any(|name| name.eq_ignore_ascii_case("gnome"))
}
