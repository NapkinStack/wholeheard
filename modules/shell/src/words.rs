//! What the user is told, in one place: every sentence of the shell is here, in each language
//! it speaks, so that it is read as a whole and a language is added by adding its sentences.

use crate::{
    recording::{Said, Source},
    transcription::Made,
};

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
    let said = ["summit-capture:", "summit-transcription:"]
        .iter()
        .find_map(|name| said.strip_prefix(name))
        .unwrap_or(said)
        .trim();
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
        (Said::Started(..), _) if !reminder => return None,
        (Said::Heard(_), _) => return None,
        // The microphone is named, so that one other than the call's shows at once.
        (Said::Started(_, microphone), English) => {
            let named = microphone
                .as_ref()
                .map(|name| format!("Microphone recorded: {name}. "))
                .unwrap_or_default();
            return notice(
                "Recording",
                format!("{named}Tell the participants that this meeting is being recorded."),
                "",
            );
        }
        (Said::Started(_, microphone), French) => {
            let named = microphone
                .as_ref()
                .map(|name| format!("Micro enregistré\u{a0}: {name}. "))
                .unwrap_or_default();
            return notice(
                "Enregistrement en cours",
                format!("{named}Prévenez les participants que cette réunion est enregistrée."),
                "",
            );
        }
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
    /// Opens the window.
    pub open: &'static str,
    /// What the tray's icon says of itself while a recording runs.
    pub recording: &'static str,
    /// Makes the transcript of the last recording, again when it failed.
    pub transcribe: &'static str,
    /// The same entry while a transcript is being made.
    pub transcribing: &'static str,
}

pub fn menu(language: Language) -> Menu {
    match language {
        Language::English => Menu {
            start: "Start recording",
            stop: "Stop recording",
            stopping: "Stopping…",
            quit: "Quit Summit",
            open: "Open Summit",
            recording: "Summit — recording",
            transcribe: "Transcribe the last recording",
            transcribing: "Transcribing…",
        },
        Language::French => Menu {
            start: "Démarrer l’enregistrement",
            stop: "Arrêter l’enregistrement",
            stopping: "Arrêt en cours…",
            quit: "Quitter Summit",
            open: "Ouvrir Summit",
            recording: "Summit — enregistrement en cours",
            transcribe: "Transcrire le dernier enregistrement",
            transcribing: "Transcription en cours…",
        },
    }
}

/// The window's words. A `{}` is where the window puts a number, a size or a month: it formats
/// those itself, in the language named by `locale`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Window {
    pub locale: &'static str,
    /// Bytes, kilobytes, megabytes, gigabytes.
    pub units: [&'static str; 4],
    /// What a screen reader calls the window's sections.
    pub sections: &'static str,
    pub meetings: &'static str,
    /// The weight of everything: its size, how many meetings, how long their audio lasts.
    pub weight: &'static str,
    /// The same, for one meeting.
    pub weight_one: &'static str,
    pub audio_kept: &'static str,
    pub text_kept: &'static str,
    /// A picture of the two weights, for whoever does not see it.
    pub weight_described: &'static str,
    pub this_week: &'static str,
    pub last_week: &'static str,
    /// Earlier in the current month: the month's name.
    pub earlier_in: &'static str,
    /// Sessions whose start cannot be read as a day.
    pub undated: &'static str,
    pub minutes: &'static str,
    pub transcript: &'static str,
    pub audio: &'static str,
    pub audio_freed: &'static str,
    pub recording: &'static str,
    pub interrupted: &'static str,
    /// No session in the folder: the folder.
    pub none: &'static str,
    /// One meeting's page.
    pub back: &'static str,
    pub open_folder: &'static str,
    pub minutes_title: &'static str,
    pub transcript_title: &'static str,
    pub search: &'static str,
    /// The minutes' maker: the model, the provider, when.
    pub written_by: &'static str,
    /// The transcript's maker: the model, on this computer.
    pub heard_by: &'static str,
    pub here: &'static str,
    pub remote: &'static str,
    /// A voice told within a track: its label.
    pub voice: &'static str,
    pub here_described: &'static str,
    pub remote_described: &'static str,
    /// Who spoke over the meeting, for whoever does not see the drawing.
    pub who_spoke: &'static str,
    pub no_transcript: &'static str,
    pub no_minutes: &'static str,
    pub being_made: &'static str,
    pub unreadable: &'static str,
    /// Lines of a transcript that are not the contract's, left out: how many.
    pub unread: &'static str,
    pub no_match: &'static str,
    /// The settings' page.
    pub settings: &'static str,
    pub after_title: &'static str,
    /// Each choice of what follows a recording: its name, then what it does.
    pub after_nothing: [&'static str; 2],
    pub after_transcript: [&'static str; 2],
    pub after_minutes: [&'static str; 2],
    pub models_title: &'static str,
    pub transcription: &'static str,
    pub on_this_computer: &'static str,
    pub reminder: [&'static str; 2],
    pub keep_audio: [&'static str; 2],
    pub keep_always: &'static str,
    /// A number of days the audio is kept.
    pub keep_days: &'static str,
    /// A change could not be saved: why.
    pub not_saved: &'static str,
    pub minutes_step: &'static str,
    pub ollama_group: &'static str,
    pub online_group: &'static str,
    pub other_group: &'static str,
    pub custom: &'static str,
    pub address: &'static str,
    pub model_name: &'static str,
    pub key: &'static str,
    pub key_kept: &'static str,
    pub save: &'static str,
    pub where_online: &'static str,
    pub not_chosen: &'static str,
}

pub fn window(language: Language) -> Window {
    match language {
        Language::English => Window {
            locale: "en",
            units: ["B", "kB", "MB", "GB"],
            sections: "Sections",
            meetings: "Meetings",
            weight: "taken by {} meetings · {} of audio",
            weight_one: "taken by {} meeting · {} of audio",
            audio_kept: "Audio, as recorded — {}",
            text_kept: "Transcripts and minutes — {}",
            weight_described: "The audio takes {}; the transcripts and minutes, {}.",
            this_week: "This week",
            last_week: "Last week",
            earlier_in: "Earlier in {}",
            undated: "Date unreadable",
            minutes: "Minutes",
            transcript: "Transcript",
            audio: "Audio · {}",
            audio_freed: "Audio deleted",
            recording: "Recording now",
            interrupted: "Interrupted — never stopped",
            none: "No meeting in {} yet.",
            back: "Meetings",
            open_folder: "Open the folder",
            minutes_title: "Minutes",
            transcript_title: "Transcript",
            search: "Search the meeting",
            written_by: "Written by {}, via {} · {}",
            heard_by: "Written by {}, on this computer",
            here: "Here",
            remote: "Remote",
            voice: "voice {}",
            here_described: "Here — your microphone",
            remote_described: "Remote — what the machine plays",
            who_spoke: "Who spoke over the meeting: here above, remote below",
            no_transcript: "No transcript yet.",
            no_minutes: "No minutes yet.",
            being_made: "Being written — what is here is true all the same.",
            unreadable: "This file cannot be read by this version of Summit.",
            unread: "{} line(s) of the file could not be read and are left out.",
            no_match: "Nothing said matches.",
            settings: "Settings",
            after_title: "After a recording",
            after_nothing: ["Do nothing", "The audio is kept; you start what follows when you want."],
            after_transcript: ["Write the transcript", "On this computer, with no network."],
            after_minutes: [
                "Write the transcript, then the minutes",
                "Each follows the other; a notification says when the minutes are ready.",
            ],
            models_title: "Models",
            transcription: "Transcription",
            on_this_computer: "On this computer — nothing leaves it",
            reminder: [
                "Remind me to tell the participants",
                "At the start, a line reminds you to say the meeting is recorded.",
            ],
            keep_audio: [
                "Keep the audio",
                "After that, the audio goes to the trash; the transcript and minutes stay.",
            ],
            keep_always: "Always",
            keep_days: "{} days",
            not_saved: "The setting was not saved: {}",
            minutes_step: "Minutes",
            ollama_group: "On this computer — found in Ollama",
            online_group: "Online",
            other_group: "Other",
            custom: "Custom…",
            address: "Address (OpenAI-compatible)",
            model_name: "Model",
            key: "Key",
            key_kept: "A key is kept in the system's keyring; a new one replaces it.",
            save: "Save",
            where_online: "Online — the transcript's text is sent",
            not_chosen: "Not chosen yet",
        },
        Language::French => Window {
            locale: "fr",
            units: ["o", "ko", "Mo", "Go"],
            sections: "Rubriques",
            meetings: "Réunions",
            weight: "occupés par {} réunions · {} d’audio",
            weight_one: "occupés par {} réunion · {} d’audio",
            audio_kept: "Audio, tel qu’enregistré — {}",
            text_kept: "Transcripts et comptes rendus — {}",
            weight_described: "L’audio occupe {} ; les transcripts et les comptes rendus, {}.",
            this_week: "Cette semaine",
            last_week: "La semaine dernière",
            earlier_in: "Plus tôt en {}",
            undated: "Date illisible",
            minutes: "Compte rendu",
            transcript: "Transcript",
            audio: "Audio · {}",
            audio_freed: "Audio supprimé",
            recording: "Enregistrement en cours",
            interrupted: "Interrompu — jamais arrêté",
            none: "Aucune réunion dans {} pour l’instant.",
            back: "Réunions",
            open_folder: "Ouvrir le dossier",
            minutes_title: "Compte rendu",
            transcript_title: "Transcript",
            search: "Chercher dans la réunion",
            written_by: "Écrit par {}, via {} · {}",
            heard_by: "Écrit par {}, sur cet ordinateur",
            here: "Ici",
            remote: "À distance",
            voice: "voix {}",
            here_described: "Ici — votre micro",
            remote_described: "À distance — ce que joue la machine",
            who_spoke: "Qui a parlé au fil de la réunion : ici en haut, à distance en bas",
            no_transcript: "Pas encore de transcript.",
            no_minutes: "Pas encore de compte rendu.",
            being_made: "En cours d’écriture — ce qui est là est vrai quand même.",
            unreadable: "Ce fichier ne peut pas être lu par cette version de Summit.",
            unread: "{} ligne(s) du fichier n’ont pas pu être lues et sont laissées de côté.",
            no_match: "Rien de ce qui a été dit ne correspond.",
            settings: "Réglages",
            after_title: "Après un enregistrement",
            after_nothing: ["Ne rien faire", "L’audio est gardé ; vous lancez la suite quand vous voulez."],
            after_transcript: ["Écrire le transcript", "Sur cet ordinateur, sans réseau."],
            after_minutes: [
                "Écrire le transcript, puis le compte rendu",
                "Tout s’enchaîne ; une notification prévient quand le compte rendu est prêt.",
            ],
            models_title: "Modèles",
            transcription: "Transcription",
            on_this_computer: "Sur cet ordinateur — rien ne sort",
            reminder: [
                "Rappeler de prévenir les participants",
                "Au démarrage, une ligne rappelle de dire que la réunion est enregistrée.",
            ],
            keep_audio: [
                "Garder l’audio",
                "Passé ce délai, l’audio part à la corbeille ; le transcript et le compte rendu restent.",
            ],
            keep_always: "Toujours",
            keep_days: "{} jours",
            not_saved: "Le réglage n’a pas été enregistré : {}",
            minutes_step: "Compte rendu",
            ollama_group: "Sur cet ordinateur — trouvés dans Ollama",
            online_group: "En ligne",
            other_group: "Autre",
            custom: "Personnalisé…",
            address: "Adresse (compatible OpenAI)",
            model_name: "Modèle",
            key: "Clé",
            key_kept: "Une clé est gardée dans le trousseau du système ; une nouvelle la remplace.",
            save: "Enregistrer",
            where_online: "En ligne — le texte du transcript est envoyé",
            not_chosen: "Pas encore choisi",
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
    /// The model a transcript is heard with is not on the machine: where it was looked for.
    NoModel(String),
    /// The program that hears could not be started: what the system said of it.
    NoTranscriber(String),
    /// A transcript was asked for, and no recording is in the folder of sessions.
    NoSession,
}

/// What the user is told of a trouble: what it is, and what still works.
pub fn trouble(trouble: &Trouble, language: Language) -> Notice {
    use Language::{English, French};
    let nothing = String::new();
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
        (Trouble::NoModel(at), English) => (
            "No transcript: the model is missing",
            "The recording is kept. Place the Whisper model there, then use “Transcribe the last recording”.",
            at,
        ),
        (Trouble::NoModel(at), French) => (
            "Pas de transcription\u{a0}: le modèle manque",
            "L’enregistrement est conservé. Placez le modèle Whisper à cet endroit, puis utilisez «\u{a0}Transcrire le dernier enregistrement\u{a0}».",
            at,
        ),
        (Trouble::NoTranscriber(why), English) => (
            "Summit cannot transcribe",
            "Its transcription program could not be started. The recording is kept; installing Summit again puts the program back.",
            why,
        ),
        (Trouble::NoSession, English) => (
            "No recording to transcribe",
            "Record a meeting first: its transcript follows the stop.",
            &nothing,
        ),
        (Trouble::NoSession, French) => (
            "Aucun enregistrement à transcrire",
            "Enregistrez d’abord une réunion\u{a0}: sa transcription suit l’arrêt.",
            &nothing,
        ),
        (Trouble::NoTranscriber(why), French) => (
            "Summit ne peut pas transcrire",
            "Son programme de transcription n’a pas pu démarrer. L’enregistrement est conservé\u{a0}; réinstaller Summit remet le programme en place.",
            why,
        ),
    };
    notice(title, body.to_string(), why).expect("a trouble is always said")
}

/// What the user is told of a transcript's end: there, with where it is — or not, with why,
/// and how to have it made again.
pub fn made(made: &Made, language: Language) -> Notice {
    use Language::{English, French};
    let (title, body, why) = match (made, language) {
        (Made::Written(_), English) => ("Transcript ready", "Click to open the session’s folder.", ""),
        (Made::Written(_), French) => ("Transcription prête", "Cliquez pour ouvrir le dossier de la session.", ""),
        (Made::Failed(why), English) => (
            "The transcript was not made",
            "The recording is untouched. “Transcribe the last recording” in Summit’s menu tries again from where it stopped.",
            why.as_str(),
        ),
        (Made::Failed(why), French) => (
            "La transcription n’a pas été faite",
            "L’enregistrement est intact. «\u{a0}Transcrire le dernier enregistrement\u{a0}», dans le menu de Summit, reprend là où elle s’est arrêtée.",
            why.as_str(),
        ),
    };
    notice(title, body.to_string(), why).expect("a transcript's end is always said")
}

/// Said after a recording's stop when its transcript is made next, without another gesture.
pub fn follows(language: Language) -> &'static str {
    match language {
        Language::English => "The transcript follows.",
        Language::French => "La transcription suit.",
    }
}

/// The menu's entry that fetches the default model: what it is, how big, and from where — the
/// user's click on it is their agreement.
pub fn download(language: Language, size: &str, host: &str) -> String {
    match language {
        Language::English => format!("Download the model ({size}, {host})"),
        Language::French => format!("Télécharger le modèle ({size}, {host})"),
    }
}

/// The same entry while the model comes.
pub fn downloading(language: Language, percent: u64) -> String {
    match language {
        Language::English => format!("Downloading the model… {percent} %"),
        Language::French => format!("Téléchargement du modèle… {percent}\u{a0}%"),
    }
}

/// The entry while the model comes, from the bytes held: the share come — then, once every
/// byte is there, that its fingerprint is being checked.
pub fn fetching(language: Language, held: u64, bytes: u64) -> String {
    if held >= bytes {
        return match language {
            Language::English => "Checking the model…",
            Language::French => "Vérification du modèle…",
        }
        .to_string();
    }
    downloading(language, held * 100 / bytes.max(1))
}

/// Why the system's trash did not take the audio, in the user's words rather than the system's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAllowed,
    NoSpace,
    Other,
}

/// Said when the audio of meetings kept longer than the settings say went to the system's trash;
/// or, `Err`, why it could not.
pub fn audio_trashed(language: Language, outcome: &Result<(usize, u32), Refusal>) -> Notice {
    let (title, body) = match (language, outcome) {
        (Language::English, Ok((1, days))) => (
            "Audio moved to the trash".to_string(),
            format!(
                "The audio of 1 meeting kept more than {} is in the trash; its transcript and minutes stay.",
                english_days(*days)
            ),
        ),
        (Language::English, Ok((meetings, days))) => (
            "Audio moved to the trash".to_string(),
            format!(
                "The audio of {meetings} meetings kept more than {} is in the trash; their transcripts and minutes stay.",
                english_days(*days)
            ),
        ),
        (Language::French, Ok((1, days))) => (
            "Audio mis à la corbeille".to_string(),
            format!(
                "L’audio d’une réunion gardé plus {} est dans la corbeille ; son transcript et son compte rendu restent.",
                french_days(*days)
            ),
        ),
        (Language::French, Ok((meetings, days))) => (
            "Audio mis à la corbeille".to_string(),
            format!(
                "L’audio de {meetings} réunions gardé plus {} est dans la corbeille ; leurs transcripts et leurs comptes rendus restent.",
                french_days(*days)
            ),
        ),
        // The trash may have taken some before it refused: what is said holds either way.
        (Language::English, Err(refusal)) => (
            "The audio was not all moved to the trash".to_string(),
            format!(
                "{} What was not moved stays where it was; nothing was deleted.",
                match refusal {
                    Refusal::NotAllowed => "The trash refused access.",
                    Refusal::NoSpace => "The trash has no room left.",
                    Refusal::Other => "The system's trash did not take it.",
                }
            ),
        ),
        (Language::French, Err(refusal)) => (
            "L’audio n’a pas été entièrement mis à la corbeille".to_string(),
            format!(
                "{} Ce qui n’a pas été déplacé reste à sa place ; rien n’a été supprimé.",
                match refusal {
                    Refusal::NotAllowed => "La corbeille a refusé l’accès.",
                    Refusal::NoSpace => "La corbeille n’a plus de place.",
                    Refusal::Other => "La corbeille du système ne l’a pas pris.",
                }
            ),
        ),
    };
    Notice { title, body }
}

fn english_days(days: u32) -> String {
    if days == 1 { "1 day".to_string() } else { format!("{days} days") }
}

fn french_days(days: u32) -> String {
    if days == 1 {
        "d’un jour".to_string()
    } else {
        format!("de {days} jours")
    }
}

/// Said when a transcript needs the default model and it is not on the machine: nothing is
/// fetched until the user chooses the menu's entry.
pub fn model_needed(language: Language, size: &str, host: &str) -> Notice {
    let entry = download(language, size, host);
    let (title, body) = match language {
        Language::English => (
            "The transcription model is needed",
            format!("It is fetched once, on your word: choose “{entry}” in Summit’s menu. The recording is kept."),
        ),
        Language::French => (
            "Le modèle de transcription est nécessaire",
            format!(
                "Il est téléchargé une fois, sur votre accord\u{a0}: choisissez «\u{a0}{entry}\u{a0}» dans le menu de Summit. L’enregistrement est conservé."
            ),
        ),
    };
    notice(title, body, "").expect("always said")
}

/// Said when the model's fetch ended: ready, and the transcript begun — or not, with why, and
/// that nothing of it was used.
pub fn fetched(outcome: &Result<(), String>, language: Language) -> Notice {
    use Language::{English, French};
    let (title, body, why) = match (outcome, language) {
        (Ok(()), English) => (
            "Model ready",
            "Checked against its published fingerprint. The transcript is being made.",
            "",
        ),
        (Ok(()), French) => (
            "Modèle prêt",
            "Vérifié par son empreinte publiée. La transcription est en cours.",
            "",
        ),
        (Err(why), English) => (
            "The model was not downloaded",
            "Nothing of it is used. The menu’s entry tries again.",
            why.as_str(),
        ),
        (Err(why), French) => (
            "Le modèle n’a pas été téléchargé",
            "Rien n’en est utilisé. L’entrée du menu réessaie.",
            why.as_str(),
        ),
    };
    notice(title, body.to_string(), why).expect("always said")
}
