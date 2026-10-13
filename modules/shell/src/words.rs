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

/// A length as a person says it: seconds under a minute, then minutes, then hours.
pub fn length(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    match (seconds / 3600, seconds % 3600 / 60) {
        (0, 0) => format!("{seconds} s"),
        (0, minutes) => format!("{minutes} min"),
        (hours, minutes) => format!("{hours} h {minutes:02}"),
    }
}

/// What the user is told of something a recording said — or nothing, when it calls for no
/// word. `reminder` is the setting: tell, at each start, that announcing the recording is
/// the user's to do.
pub fn of(said: &Said, reminder: bool, language: Language) -> Option<Notice> {
    use Language::{English, French};
    let (title, body, detail) = match (said, language) {
        (Said::Started(_), _) if !reminder => return None,
        (Said::Heard(_), _) => return None,
        (Said::Started(_), English) => (
            "Recording",
            "Tell the participants that this meeting is being recorded.".to_string(),
            "",
        ),
        (Said::Started(_), French) => (
            "Enregistrement en cours",
            "Prévenez les participants que cette réunion est enregistrée.".to_string(),
            "",
        ),
        (Said::Unavailable(Source::Me, why), English) => (
            "Your microphone is not recorded",
            "The meeting's sound is still recorded.".to_string(),
            why.as_str(),
        ),
        (Said::Unavailable(Source::Me, why), French) => (
            "Votre micro n'est pas enregistré",
            "Le son de la réunion reste enregistré.".to_string(),
            why.as_str(),
        ),
        (Said::Unavailable(Source::Others, why), English) => (
            "The meeting's sound is not recorded",
            "Your microphone is still recorded.".to_string(),
            why.as_str(),
        ),
        (Said::Unavailable(Source::Others, why), French) => (
            "Le son de la réunion n'est pas enregistré",
            "Votre micro reste enregistré.".to_string(),
            why.as_str(),
        ),
        (Said::Silent(Source::Me), English) => (
            "No sound from your microphone",
            "Nothing was heard for ten seconds. The recording goes on.".to_string(),
            "",
        ),
        (Said::Silent(Source::Me), French) => (
            "Aucun son de votre micro",
            "Rien n'a été entendu depuis dix secondes. L'enregistrement continue.".to_string(),
            "",
        ),
        (Said::Silent(Source::Others), English) => (
            "No sound from the meeting",
            "Nothing was heard for ten seconds: is the call playing on this device? The recording goes on.".to_string(),
            "",
        ),
        (Said::Silent(Source::Others), French) => (
            "Aucun son de la réunion",
            "Rien n'a été entendu depuis dix secondes : l'appel passe-t-il par cet appareil ? L'enregistrement continue.".to_string(),
            "",
        ),
        (Said::Stopped(_, seconds), English) => ("Recording stopped", format!("{} recorded.", length(*seconds)), ""),
        (Said::Stopped(_, seconds), French) => ("Enregistrement arrêté", format!("Durée : {}.", length(*seconds)), ""),
        (Said::Failed(why), English) if why.is_empty() => (
            "The recording stopped by itself",
            "The program that records ended without saying why. What it had recorded is in the session's folder.".to_string(),
            "",
        ),
        (Said::Failed(why), French) if why.is_empty() => (
            "L'enregistrement s'est arrêté tout seul",
            "Le programme d'enregistrement s'est terminé sans dire pourquoi. Ce qu'il avait enregistré est dans le dossier de la session."
                .to_string(),
            "",
        ),
        (Said::Failed(why), English) => (
            "The recording stopped by itself",
            "What it had recorded is in the session's folder.".to_string(),
            why.as_str(),
        ),
        (Said::Failed(why), French) => (
            "L'enregistrement s'est arrêté tout seul",
            "Ce qu'il avait enregistré est dans le dossier de la session.".to_string(),
            why.as_str(),
        ),
    };
    // What a program said of it comes last, as it said it — in its own language, without its name.
    let detail = detail
        .split_once(": ")
        .map_or(detail, |(name, rest)| if name.contains(' ') { detail } else { rest })
        .trim();
    let body = if detail.is_empty() { body } else { format!("{body} ({detail})") };
    Some(Notice {
        title: title.to_string(),
        body,
    })
}
