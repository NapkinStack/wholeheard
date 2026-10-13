//! What the user is told, in one place: every sentence of the shell is here, in each language
//! it speaks, so that it is read as a whole and a language is added by adding its sentences.

use crate::recording::{Said, Source};

/// A language the shell speaks. The interface follows the system's; any other is English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    French,
}

impl Language {
    /// The language of a locale as a system names it: `fr-FR`, `fr_CA.UTF-8`, `en-GB`.
    pub fn of(locale: &str) -> Self {
        let language = locale.split(['-', '_', '.', '@']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("fr") {
            Self::French
        } else {
            Self::English
        }
    }

    /// The language the user's system is set to.
    pub fn system() -> Self {
        Self::of(&sys_locale::get_locale().unwrap_or_default())
    }
}

/// A notification: a title that says it all at a glance, and one or two sentences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

/// A length as a person says it: seconds under a minute, then minutes, then hours — each
/// to the nearest, so that a minute and fifty-nine seconds is two minutes.
pub fn length(seconds: f64) -> String {
    let seconds = if seconds.is_finite() { seconds.max(0.0) } else { 0.0 };
    if seconds < 59.5 {
        return format!("{} s", seconds.round() as u64);
    }
    let minutes = (seconds / 60.0).round() as u64;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, minutes) => format!("{hours} h {minutes:02}"),
    }
}

/// What a program said of a failure, made fit to show: without the name it puts before its
/// sentence, without brackets of its own, on one line, and short. A program that said nothing
/// gives nothing.
fn detail(said: &str) -> String {
    let said = said.trim();
    let said = said.strip_prefix("summit-capture:").unwrap_or(said).trim();
    let said = said.strip_prefix('(').and_then(|inner| inner.strip_suffix(')')).unwrap_or(said);
    // One line, whatever it held: a notification is no place for a program's line ends.
    let said = said
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match said.char_indices().nth(200) {
        Some((cut, _)) => format!("{}…", &said[..cut]),
        None => said.to_string(),
    }
}

/// What the user is told of something a recording said — or nothing, when it calls for no
/// word. `reminder` is the setting: tell, at each start, that announcing the recording is
/// the user's to do. French is written as French is printed: a space that does not break
/// before `:` and `?`.
pub fn of(said: &Said, reminder: bool, language: Language) -> Option<Notice> {
    use Language::{English, French};
    let (title, body, why) = match (said, language) {
        (Said::Started(_), _) if !reminder => return None,
        (Said::Heard(_), _) => return None,
        (Said::Started(_), English) => ("Recording", "Tell the participants that this meeting is being recorded.", ""),
        (Said::Started(_), French) => (
            "Enregistrement en cours",
            "Prévenez les participants que cette réunion est enregistrée.",
            "",
        ),
        (Said::Unavailable(Source::Me, why), English) => (
            "Your microphone is not recorded",
            "The meeting’s sound is still recorded.",
            why.as_str(),
        ),
        (Said::Unavailable(Source::Me, why), French) => (
            "Votre micro n’est pas enregistré",
            "Le son de la réunion est toujours enregistré.",
            why.as_str(),
        ),
        (Said::Unavailable(Source::Others, why), English) => (
            "The meeting’s sound is not recorded",
            "Your microphone is still recorded.",
            why.as_str(),
        ),
        (Said::Unavailable(Source::Others, why), French) => (
            "Le son de la réunion n’est pas enregistré",
            "Votre micro est toujours enregistré.",
            why.as_str(),
        ),
        (Said::Silent(Source::Me), English) => (
            "No sound from your microphone",
            "Nothing was heard for ten seconds. The recording goes on.",
            "",
        ),
        (Said::Silent(Source::Me), French) => (
            "Votre micro ne capte aucun son",
            "Rien n’a été entendu depuis dix secondes. L’enregistrement continue.",
            "",
        ),
        (Said::Silent(Source::Others), English) => (
            "No sound from the meeting",
            "Nothing was heard for ten seconds: is the call playing on this device? The recording goes on.",
            "",
        ),
        (Said::Silent(Source::Others), French) => (
            "Aucun son venant de la réunion",
            "Rien n’a été entendu depuis dix secondes\u{a0}: l’appel passe-t-il par cet appareil\u{202f}? L’enregistrement continue.",
            "",
        ),
        (Said::Stopped(_, seconds), English) => return notice("Recording stopped", format!("{} recorded.", length(*seconds)), ""),
        (Said::Stopped(_, seconds), French) => return notice("Enregistrement arrêté", format!("Durée\u{a0}: {}.", length(*seconds)), ""),
        (Said::Failed(why), English) if detail(why).is_empty() => (
            "The recording stopped by itself",
            "The program that records ended without saying why. What was recorded is in the session’s folder.",
            "",
        ),
        (Said::Failed(why), French) if detail(why).is_empty() => (
            "L’enregistrement s’est arrêté tout seul",
            "Le programme d’enregistrement s’est terminé sans dire pourquoi. Ce qui a été enregistré se trouve dans le dossier de la session.",
            "",
        ),
        (Said::Failed(why), English) => (
            "The recording stopped by itself",
            "What was recorded is in the session’s folder.",
            why.as_str(),
        ),
        (Said::Failed(why), French) => (
            "L’enregistrement s’est arrêté tout seul",
            "Ce qui a été enregistré se trouve dans le dossier de la session.",
            why.as_str(),
        ),
    };
    notice(title, body.to_string(), why)
}

/// A notice, with what a program said of it last, in brackets, as it said it.
fn notice(title: &str, body: String, why: &str) -> Option<Notice> {
    let why = detail(why);
    let body = if why.is_empty() { body } else { format!("{body} ({why})") };
    Some(Notice {
        title: title.to_string(),
        body,
    })
}

/// The tray's menu: what each entry says, in the state the recording is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    pub start: &'static str,
    pub stop: &'static str,
    pub stopping: &'static str,
    pub quit: &'static str,
    /// What the tray's icon says of itself while a recording runs.
    pub recording: &'static str,
}

pub fn menu(language: Language) -> Menu {
    match language {
        Language::English => Menu {
            start: "Start recording",
            stop: "Stop recording",
            stopping: "Stopping…",
            quit: "Quit Summit",
            recording: "Summit — recording",
        },
        Language::French => Menu {
            start: "Démarrer l’enregistrement",
            stop: "Arrêter l’enregistrement",
            stopping: "Arrêt en cours…",
            quit: "Quitter Summit",
            recording: "Summit — enregistrement en cours",
        },
    }
}

/// What can stand in the shell's own way, before or beside a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// The program that records could not be started: what the system said of it.
    NoRecorder(String),
    /// The settings file was not read: the sentence that says which file, and what in it.
    Settings(String),
    /// The desktop did not bind the global shortcut: what it said of it.
    Shortcut(String),
}

/// What the user is told of a trouble: what it is, and what still works.
pub fn trouble(trouble: &Trouble, language: Language) -> Notice {
    use Language::{English, French};
    let (title, body, why) = match (trouble, language) {
        (Trouble::NoRecorder(why), English) => (
            "Summit cannot record",
            "Its recording program could not be started. Installing Summit again puts it back.",
            why,
        ),
        (Trouble::NoRecorder(why), French) => (
            "Summit ne peut pas enregistrer",
            "Son programme d’enregistrement n’a pas pu démarrer. Réinstaller Summit le remet en place.",
            why,
        ),
        (Trouble::Settings(why), English) => (
            "Your settings were not read",
            "Summit runs with its defaults until the file is corrected.",
            why,
        ),
        (Trouble::Settings(why), French) => (
            "Vos réglages n’ont pas été lus",
            "Summit fonctionne avec ses réglages par défaut tant que le fichier n’est pas corrigé.",
            why,
        ),
        (Trouble::Shortcut(why), English) => (
            "The keyboard shortcut is not set",
            "The menu of Summit’s icon still starts and stops a recording.",
            why,
        ),
        (Trouble::Shortcut(why), French) => (
            "Le raccourci clavier n’est pas actif",
            "Le menu de l’icône Summit permet toujours de démarrer et d’arrêter un enregistrement.",
            why,
        ),
    };
    notice(title, body.to_string(), why).expect("a trouble is always said")
}
