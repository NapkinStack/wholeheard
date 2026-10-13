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
/// sentence, without brackets of its own, and short. A program that said nothing gives nothing.
fn detail(said: &str) -> String {
    let said = said.trim();
    let said = said.strip_prefix("summit-capture:").unwrap_or(said).trim();
    let said = said
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(said)
        .trim();
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
