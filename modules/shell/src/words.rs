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
    let said = ["wholeheard-capture:", "wholeheard-transcription:"]
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
        (Said::Heard(_) | Said::Level(..), _) => return None,
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
    /// Has the minutes of the last recording written, again when it failed.
    pub minutes: &'static str,
    /// The same entry while they are being written.
    pub minuting: &'static str,
}

pub fn menu(language: Language) -> Menu {
    match language {
        Language::English => Menu {
            start: "Start recording",
            stop: "Stop recording",
            stopping: "Stopping…",
            quit: "Quit Wholeheard",
            open: "Open Wholeheard",
            recording: "Wholeheard — recording",
            transcribe: "Transcribe the last recording",
            transcribing: "Transcribing…",
            minutes: "Write the minutes of the last recording",
            minuting: "Writing the minutes…",
        },
        Language::French => Menu {
            start: "Démarrer l’enregistrement",
            stop: "Arrêter l’enregistrement",
            stopping: "Arrêt en cours…",
            quit: "Quitter Wholeheard",
            open: "Ouvrir Wholeheard",
            recording: "Wholeheard — enregistrement en cours",
            transcribe: "Transcrire le dernier enregistrement",
            transcribing: "Transcription en cours…",
            minutes: "Faire le compte rendu du dernier enregistrement",
            minuting: "Compte rendu en cours…",
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
    /// Between a day and its hour: "mar. 13 oct. à 12:21", "Tue, Oct 13 at 12:21 PM".
    pub at: &'static str,
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
    /// What the models' card says under its title, and under its steps.
    pub models_intro: &'static str,
    pub models_note: &'static str,
    /// The transcription model's state, at the page's end.
    pub model_state: &'static str,
    pub model_ready: &'static str,
    pub model_here: &'static str,
    pub model_missing: &'static str,
    /// The model's name as people say it, and its size once installed.
    pub installed: &'static str,
    /// Another number of days, typed; and what is said of one that is none.
    pub keep_other: &'static str,
    pub keep_not_days: &'static str,
    /// The engine that hears, shipped with Wholeheard; and the user's home, as a folder is said.
    pub builtin: &'static str,
    pub home: &'static str,
    pub transcription: &'static str,
    pub on_this_computer: &'static str,
    pub reminder: [&'static str; 2],
    pub keep_audio: [&'static str; 2],
    /// The folder the sessions go to, and the button that opens the system's dialog for it.
    pub sessions_folder: [&'static str; 2],
    pub choose: &'static str,
    /// The small window while recording (D11's sixth board).
    pub small: &'static str,
    pub small_mic: &'static str,
    pub small_meeting: &'static str,
    pub small_stop: &'static str,
    pub small_note: &'static str,
    pub small_full: &'static str,
    /// Freeing space (D11's third board).
    pub free: &'static str,
    pub free_what: &'static str,
    pub free_audio: [&'static str; 2],
    pub free_whole: [&'static str; 2],
    pub free_heaviest: &'static str,
    pub free_older: &'static str,
    pub free_total: &'static str,
    pub free_one: &'static str,
    pub free_note: &'static str,
    pub free_audio_note: &'static str,
    pub free_do_audio: &'static str,
    pub free_do_whole: &'static str,
    pub free_busy: &'static str,
    pub free_done: &'static str,
    /// *Refaire…* on the minutes (D11's sixth board): the panel's title, question, note and gesture.
    pub redo: &'static str,
    pub redo_title: &'static str,
    pub redo_with: &'static str,
    pub redo_note: &'static str,
    pub redo_do: &'static str,
    pub on_computer: &'static str,
    pub redo_transcript_title: &'static str,
    pub redo_transcript_note: &'static str,
    /// What a meeting lacks, offered on its page; what is said once asked; where a step ran.
    pub make_minutes: &'static str,
    pub make_transcript: &'static str,
    /// What is made in the background, said on the pages (D25): the minutes being written (by,
    /// for how long), waiting their turn ({} before), waiting for the user's agreement to send them
    /// ({} the host) with the way to give it, not written ({} the reason); on a list's row.
    pub minutes_making: &'static str,
    pub minutes_waiting: &'static str,
    pub minutes_awaiting: &'static str,
    pub agree_open: &'static str,
    pub minutes_not_made: &'static str,
    pub try_again: &'static str,
    pub chip_making: &'static str,
    pub chip_waiting: &'static str,
    pub chip_awaiting: &'static str,
    pub chip_transcribing: &'static str,
    pub chip_failed: &'static str,
    /// The same for the transcript: being made (by, for how long), not made ({} the reason).
    pub transcript_making: &'static str,
    pub transcript_not_made: &'static str,
    pub chip_transcript_failed: &'static str,
    /// Who writes the minutes chosen on the settings' page, not yet saved: who still does ({}).
    pub minutes_unsaved: &'static str,
    pub minutes_unsaved_none: &'static str,
    /// A transcript waiting for the default Whisper model: offered in the menu, or coming.
    pub transcript_model_needed: &'static str,
    pub transcript_model_coming: &'static str,
    pub ran_online: &'static str,
    pub ran_here: &'static str,
    /// The header's button, at rest and while a recording runs.
    pub record_start: &'static str,
    pub record_stop: &'static str,
    /// The agreement before a meeting first leaves for a host (D11's fifth board): what leaves,
    /// what stays, to whom; the user's word, once or for this service; on this computer instead.
    pub agree_eyebrow: &'static str,
    pub agree_title: &'static str,
    pub agree_lead: &'static str,
    pub agree_leaves: [&'static str; 2],
    pub agree_context: [&'static str; 2],
    pub agree_stays: [&'static str; 2],
    pub agree_to: &'static str,
    pub agree_model: &'static str,
    pub agree_remember: &'static str,
    pub agree_send: &'static str,
    pub agree_not: &'static str,
    pub agree_local: &'static str,
    pub agree_local_write: &'static str,
    /// The global shortcut: its keys as granted, none granted, where it changes, the button there.
    pub shortcut: [&'static str; 2],
    pub shortcut_none: &'static str,
    pub shortcut_where: &'static str,
    pub shortcut_open: &'static str,
    /// *Tester*: the button, what it sends and to whom, and what it found.
    pub test: &'static str,
    pub test_sends: &'static str,
    pub testing: &'static str,
    pub test_answered: &'static str,
    pub test_failed: &'static str,
    pub keep_always: &'static str,
    /// A number of days the audio is kept.
    pub keep_days: &'static str,
    /// The settings file could not be read: why.
    pub settings_unreadable: &'static str,
    /// A change could not be saved: why.
    pub not_saved: &'static str,
    /// Who writes the minutes was saved, but the keyring did not keep the key: why.
    pub key_not_kept: &'static str,
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
    /// The model saved, which this computer's Ollama does not have ({} the model).
    pub ollama_lacks: &'static str,
    /// This computer's Ollama, saved, not answering — or answering with no model.
    pub ollama_silent: &'static str,
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
            at: "at",
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
            unreadable: "This file cannot be read by this version of Wholeheard.",
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
            models_intro: "For each step, you choose who works: a model on this computer, an online service, or your own address.",
            models_note: "The models “on this computer” are those Wholeheard finds installed. A key is kept in your system’s keyring, never in a file. The audio never leaves this computer: only the transcript’s text goes to an online service, after your agreement.",
            model_state: "Transcription model",
            model_ready: "Ready",
            model_here: "Installed · {} · runs on this computer, with no network",
            model_missing: "Not on this computer yet: it is offered at the first transcript.",
            installed: "installed, {}",
            keep_other: "Another number of days…",
            keep_not_days: "{} is no number of days: from 1 to 36,500",
            builtin: "built into Wholeheard",
            home: "Home folder",
            transcription: "Transcription",
            on_this_computer: "On this computer — nothing leaves it",
            reminder: [
                "Remind me to tell the participants",
                "At the start, a line reminds you to say the meeting is recorded.",
            ],
            sessions_folder: [
                "Meetings folder",
                "New meetings are written here. Those already recorded stay where they were: the window and the audio's keeping follow this folder.",
            ],
            choose: "Choose…",
            small: "Small window",
            small_mic: "Your microphone",
            small_meeting: "The meeting",
            small_stop: "Stop",
            small_note: "Once stopped, the transcript writes itself. You will be told.",
            small_full: "The whole window",
            free: "Free space",
            free_what: "What do you want to remove?",
            free_audio: [
                "The audio only",
                "The minutes and the transcript still read. The audio is nearly all the weight.",
            ],
            free_whole: [
                "The whole meeting",
                "The audio, the transcript and the minutes: nothing of the meeting is left.",
            ],
            free_heaviest: "Heaviest first",
            free_older: "Tick those older than 30 days",
            free_total: "freed · {} meetings",
            free_one: "freed · 1 meeting",
            free_note: "Nothing is erased for good: everything goes to your system’s trash. To undo, restore from the trash.",
            free_audio_note: "Without its audio, a meeting can no longer be transcribed again by a better engine.",
            free_do_audio: "Move their audio to the trash",
            free_do_whole: "Move these meetings to the trash",
            free_busy: "being recorded, transcribed or its minutes written",
            free_done: "Done: in the trash.",
            redo: "Redo…",
            redo_title: "Write the minutes again",
            redo_with: "With which model",
            redo_note: "The current minutes stay in place until the new ones are ready, then go to the trash.",
            redo_do: "Redo",
            on_computer: "on this computer",
            redo_transcript_title: "Write the transcript again",
            redo_transcript_note: "The current transcript is kept until the new one is whole, then goes to the trash; if the new one fails, the current comes back.",
            make_minutes: "Write the minutes",
            make_transcript: "Write the transcript",
            minutes_making: "Writing the minutes",
            minutes_waiting: "Waiting its turn — {} before it",
            minutes_awaiting: "Waiting for your agreement to send it to {}",
            agree_open: "Review and agree",
            minutes_not_made: "The minutes were not written: {}",
            try_again: "Try again",
            chip_making: "Minutes…",
            chip_waiting: "Waiting",
            chip_awaiting: "Your agreement",
            chip_transcribing: "Transcript…",
            chip_failed: "Minutes not written",
            transcript_making: "Writing the transcript",
            transcript_not_made: "The transcript was not written: {}",
            chip_transcript_failed: "Transcript not written",
            minutes_unsaved: "Not saved yet — the minutes are still written by {}.",
            minutes_unsaved_none: "Not saved yet — no minutes are written until it is.",
            transcript_model_needed: "Waiting for the Whisper model — download it from Wholeheard's menu, in the tray.",
            transcript_model_coming: "Waiting for the Whisper model, being downloaded.",
            ran_online: "online",
            ran_here: "on this machine",
            record_start: "Start a recording",
            record_stop: "Stop the recording",
            agree_eyebrow: "Before the first minutes online",
            agree_title: "This meeting’s text is about to leave your computer",
            agree_lead: "To write the minutes, Wholeheard sends the transcript to the service you chose. Here is exactly what leaves.",
            agree_leaves: [
                "Leaves: the transcript",
                "The words spoken, on your side and remote — {} minutes, about {} words.",
            ],
            agree_context: ["Leaves: the context you attached", "About {} words."],
            agree_stays: ["Stays here: the audio", "No sound is ever sent."],
            agree_to: "Recipient",
            agree_model: "Model",
            agree_remember: "Don’t ask again for this service",
            agree_send: "Send the transcript",
            agree_not: "Don’t send",
            agree_local: "Write the minutes on this computer instead",
            agree_local_write: "Write here",
            shortcut: ["Shortcut", "Starts and stops a recording, wherever you are."],
            shortcut_none: "Not granted by the desktop",
            shortcut_where: "The desktop grants it: it changes in the desktop's keyboard settings.",
            shortcut_open: "Keyboard settings",
            test: "Test",
            test_sends: "Test sends {} one sentence of Wholeheard's, nothing of a meeting.",
            testing: "Testing…",
            test_answered: "The model answers.",
            test_failed: "The model does not answer: {}",
            keep_audio: [
                "Keep the audio",
                "After that, the audio goes to the trash; the transcript and minutes stay.",
            ],
            keep_always: "Always",
            keep_days: "{} days",
            settings_unreadable: "The settings file cannot be read ({}): Wholeheard does not write over it until it is corrected.",
            not_saved: "The setting was not saved: {}",
            key_not_kept: "The choice is saved, but the key was not kept in the system's keyring: {}",
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
            ollama_lacks: "Ollama, on this computer, has no model named {}: choose one in the list, or fetch it with Ollama.",
            ollama_silent: "Ollama, on this computer, does not answer, or has no model: start it, or fetch a model with it.",
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
            at: "à",
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
            unreadable: "Ce fichier ne peut pas être lu par cette version de Wholeheard.",
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
            models_intro: "Pour chaque étape, vous choisissez qui travaille : un modèle sur cet ordinateur, un service en ligne, ou votre propre adresse.",
            models_note: "Les modèles « sur cet ordinateur » sont ceux que Wholeheard trouve installés. Une clé est gardée dans le trousseau de votre système, jamais dans un fichier. L’audio ne quitte jamais cet ordinateur : seul le texte du transcript part vers un service en ligne, après votre accord.",
            model_state: "Modèle de transcription",
            model_ready: "Prêt",
            model_here: "Installé · {} · tourne sur cet ordinateur, sans réseau",
            model_missing: "Pas encore sur cet ordinateur : il est proposé au premier transcript.",
            installed: "installé, {}",
            keep_other: "Autre nombre de jours…",
            keep_not_days: "{} n’est pas un nombre de jours : de 1 à 36 500",
            builtin: "intégré à Wholeheard",
            home: "Dossier personnel",
            transcription: "Transcription",
            on_this_computer: "Sur cet ordinateur — rien ne sort",
            reminder: [
                "Rappeler de prévenir les participants",
                "Au démarrage, une ligne rappelle de dire que la réunion est enregistrée.",
            ],
            sessions_folder: [
                "Dossier des réunions",
                "Les nouvelles réunions y sont écrites. Celles déjà enregistrées restent où elles étaient : la fenêtre et la garde de l’audio suivent ce dossier.",
            ],
            choose: "Choisir…",
            small: "Petite fenêtre",
            small_mic: "Votre micro",
            small_meeting: "La réunion",
            small_stop: "Arrêter",
            small_note: "À l’arrêt, le transcript s’écrit tout seul. Vous serez prévenu.",
            small_full: "Toute la fenêtre",
            free: "Faire de la place",
            free_what: "Que voulez-vous retirer ?",
            free_audio: [
                "L’audio seulement",
                "Le compte rendu et le transcript restent lisibles. C’est l’audio qui pèse presque tout.",
            ],
            free_whole: [
                "La réunion entière",
                "L’audio, le transcript et le compte rendu : il ne reste rien de la réunion.",
            ],
            free_heaviest: "Les plus lourdes d’abord",
            free_older: "Cocher celles de plus de 30 jours",
            free_total: "libérés · {} réunions",
            free_one: "libérés · 1 réunion",
            free_note: "Rien n’est effacé pour de bon : tout part dans la corbeille de votre système. Pour annuler, restaurez depuis la corbeille.",
            free_audio_note: "Sans son audio, une réunion ne pourra plus être transcrite à nouveau par un meilleur moteur.",
            free_do_audio: "Mettre leur audio à la corbeille",
            free_do_whole: "Mettre ces réunions à la corbeille",
            free_busy: "en cours d’enregistrement, de transcription ou de rédaction du compte rendu",
            free_done: "C’est fait : dans la corbeille.",
            redo: "Refaire…",
            redo_title: "Refaire le compte rendu",
            redo_with: "Avec quel modèle",
            redo_note: "Le compte rendu actuel reste en place tant que le nouveau n’est pas prêt, et se retrouve ensuite dans la corbeille.",
            redo_do: "Refaire",
            on_computer: "sur cet ordinateur",
            redo_transcript_title: "Refaire le transcript",
            redo_transcript_note: "Le transcript actuel est gardé tant que le nouveau n’est pas entier, puis part à la corbeille ; si le nouveau échoue, l’actuel revient.",
            make_minutes: "Faire le compte rendu",
            make_transcript: "Faire le transcript",
            minutes_making: "Rédaction du compte rendu",
            minutes_waiting: "En attente de son tour — {} avant",
            minutes_awaiting: "En attente de votre accord pour l’envoyer à {}",
            agree_open: "Voir et accepter",
            minutes_not_made: "Le compte rendu n’a pas été fait : {}",
            try_again: "Réessayer",
            chip_making: "Compte rendu…",
            chip_waiting: "En attente",
            chip_awaiting: "Votre accord",
            chip_transcribing: "Transcript…",
            chip_failed: "Compte rendu non fait",
            transcript_making: "Transcription en cours",
            transcript_not_made: "Le transcript n’a pas été fait : {}",
            chip_transcript_failed: "Transcript non fait",
            minutes_unsaved: "Pas encore enregistré — le compte rendu reste fait par {}.",
            minutes_unsaved_none: "Pas encore enregistré — aucun compte rendu n’est fait tant que ce n’est pas le cas.",
            transcript_model_needed: "En attente du modèle Whisper — téléchargez-le depuis le menu de Wholeheard, dans la barre système.",
            transcript_model_coming: "En attente du modèle Whisper, en cours de téléchargement.",
            ran_online: "en ligne",
            ran_here: "sur cette machine",
            record_start: "Démarrer un enregistrement",
            record_stop: "Arrêter l’enregistrement",
            agree_eyebrow: "Avant le premier compte rendu en ligne",
            agree_title: "Le texte de cette réunion va quitter votre ordinateur",
            agree_lead: "Pour écrire le compte rendu, Wholeheard envoie le transcript au service que vous avez choisi. Voici exactement ce qui part.",
            agree_leaves: [
                "Part : le transcript",
                "Les mots prononcés, de votre côté et à distance — {} minutes, environ {} mots.",
            ],
            agree_context: ["Part : le contexte que vous avez joint", "Environ {} mots."],
            agree_stays: ["Reste ici : l’audio", "Aucun son n’est envoyé, jamais."],
            agree_to: "Destinataire",
            agree_model: "Modèle",
            agree_remember: "Ne plus demander pour ce service",
            agree_send: "Envoyer le transcript",
            agree_not: "Ne pas envoyer",
            agree_local: "Écrire plutôt le compte rendu sur cet ordinateur",
            agree_local_write: "Écrire ici",
            shortcut: ["Raccourci", "Démarre et arrête un enregistrement, où que vous soyez."],
            shortcut_none: "Pas accordé par le bureau",
            shortcut_where: "Le bureau l’accorde : il se change dans les réglages du clavier du bureau.",
            shortcut_open: "Réglages du clavier",
            test: "Tester",
            test_sends: "Tester envoie à {} une phrase de Wholeheard, rien d’une réunion.",
            testing: "Test en cours…",
            test_answered: "Le modèle répond.",
            test_failed: "Le modèle ne répond pas : {}",
            keep_audio: [
                "Garder l’audio",
                "Passé ce délai, l’audio part à la corbeille ; le transcript et le compte rendu restent.",
            ],
            keep_always: "Toujours",
            keep_days: "{} jours",
            settings_unreadable: "Le fichier des réglages ne peut pas être lu ({}) : Wholeheard ne l’écrit pas tant qu’il n’est pas corrigé.",
            not_saved: "Le réglage n’a pas été enregistré : {}",
            key_not_kept: "Le choix est enregistré, mais la clé n’a pas été gardée dans le trousseau du système : {}",
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
            ollama_lacks: "Ollama, sur cet ordinateur, n’a pas de modèle nommé {} : choisissez-en un dans la liste, ou récupérez-le avec Ollama.",
            ollama_silent: "Ollama, sur cet ordinateur, ne répond pas, ou n’a aucun modèle : démarrez-le, ou récupérez un modèle avec lui.",
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
            "Wholeheard cannot record",
            "Its recording program could not be started. Installing Wholeheard again puts it back.",
            why,
        ),
        (Trouble::NoRecorder(why), French) => (
            "Wholeheard ne peut pas enregistrer",
            "Son programme d’enregistrement n’a pas pu démarrer. Réinstaller Wholeheard le remet en place.",
            why,
        ),
        (Trouble::Settings(why), English) => (
            "Your settings were not read",
            "Wholeheard runs with its defaults until the file is corrected.",
            why,
        ),
        (Trouble::Settings(why), French) => (
            "Vos réglages n’ont pas été lus",
            "Wholeheard fonctionne avec ses réglages par défaut tant que le fichier n’est pas corrigé.",
            why,
        ),
        (Trouble::Shortcut(why), English) => (
            "The keyboard shortcut is not set",
            "The menu of Wholeheard’s icon still starts and stops a recording.",
            why,
        ),
        (Trouble::Shortcut(why), French) => (
            "Le raccourci clavier n’est pas actif",
            "Le menu de l’icône Wholeheard permet toujours de démarrer et d’arrêter un enregistrement.",
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
            "Wholeheard cannot transcribe",
            "Its transcription program could not be started. The recording is kept; installing Wholeheard again puts the program back.",
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
            "Wholeheard ne peut pas transcrire",
            "Son programme de transcription n’a pas pu démarrer. L’enregistrement est conservé\u{a0}; réinstaller Wholeheard remet le programme en place.",
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
            "The recording is untouched. “Transcribe the last recording” in Wholeheard’s menu tries again from where it stopped.",
            why.as_str(),
        ),
        (Made::Failed(why), French) => (
            "La transcription n’a pas été faite",
            "L’enregistrement est intact. «\u{a0}Transcrire le dernier enregistrement\u{a0}», dans le menu de Wholeheard, reprend là où elle s’est arrêtée.",
            why.as_str(),
        ),
    };
    notice(title, body.to_string(), why).expect("a transcript's end is always said")
}

/// What becomes of the minutes asked for, or chained after a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Minutes {
    Ready,
    /// Ready, and the minutes they replace could not go to the trash: kept beside them, by this name.
    ReadyKeptBeside(String),
    /// Not made: wholeheard-summary's sentence.
    Failed(String),
    /// Made again (*Refaire…*) and not made: the ones there stayed; wholeheard-summary's sentence.
    RedoFailed(String),
    /// Online, and the user has not agreed to send this meeting to this host yet.
    Waiting(String),
    /// No one is chosen to write them.
    Unchosen,
    /// The last recording has no whole transcript.
    NoTranscript,
}

impl Minutes {
    /// Not made: as a first making, or as minutes made again (*Refaire…*) — the ones there stayed.
    pub fn failed(why: String, again: bool) -> Self {
        if again { Minutes::RedoFailed(why) } else { Minutes::Failed(why) }
    }
}

pub fn minutes(said: &Minutes, language: Language) -> Notice {
    use Language::{English, French};
    let entry = menu(language).minutes;
    let (title, body, why) = match (said, language) {
        (Minutes::Ready, English) => (
            "Minutes ready",
            "They are in Wholeheard’s window, beside the transcript.".to_string(),
            "",
        ),
        (Minutes::ReadyKeptBeside(name), English) => (
            "Minutes ready",
            format!("The ones they replace could not go to the trash: they are kept beside them, as {name}."),
            "",
        ),
        (Minutes::ReadyKeptBeside(name), French) => (
            "Compte rendu prêt",
            format!("Celui qu’il remplace n’a pas pu aller à la corbeille : il est gardé à côté, sous {name}."),
            "",
        ),
        (Minutes::Ready, French) => (
            "Compte rendu prêt",
            "Il est dans la fenêtre de Wholeheard, à côté du transcript.".to_string(),
            "",
        ),
        (Minutes::Failed(why), English) => (
            "The minutes were not written",
            format!("The transcript is untouched. “{entry}”, in Wholeheard’s menu, tries again."),
            why.as_str(),
        ),
        (Minutes::Failed(why), French) => (
            "Le compte rendu n’a pas été fait",
            format!("Le transcript est intact. «\u{a0}{entry}\u{a0}», dans le menu de Wholeheard, le refait."),
            why.as_str(),
        ),
        (Minutes::RedoFailed(why), English) => (
            "The minutes were not made again",
            "The previous ones stayed in place.".to_string(),
            why.as_str(),
        ),
        (Minutes::RedoFailed(why), French) => (
            "Le compte rendu n’a pas été refait",
            "Celui d’avant est resté en place.".to_string(),
            why.as_str(),
        ),
        (Minutes::Waiting(host), English) => (
            "The minutes wait for your agreement",
            format!("Nothing is sent to {host} before you agree, in Wholeheard’s window."),
            "",
        ),
        (Minutes::Waiting(host), French) => (
            "Le compte rendu attend votre accord",
            format!("Rien n’est envoyé à {host} avant votre accord, dans la fenêtre de Wholeheard."),
            "",
        ),
        (Minutes::Unchosen, English) => (
            "No one is chosen to write the minutes",
            "Choose in Wholeheard’s settings.".to_string(),
            "",
        ),
        (Minutes::Unchosen, French) => (
            "Personne n’est choisi pour écrire le compte rendu",
            "Choisissez-le dans les réglages de Wholeheard.".to_string(),
            "",
        ),
        (Minutes::NoTranscript, English) => (
            "No minutes without a transcript",
            format!("The last recording has no whole transcript: “{}” first.", menu(language).transcribe),
            "",
        ),
        (Minutes::NoTranscript, French) => (
            "Pas de compte rendu sans transcript",
            format!(
                "Le dernier enregistrement n’a pas de transcript entier : «\u{a0}{}\u{a0}» d’abord.",
                menu(language).transcribe
            ),
            "",
        ),
    };
    notice(title, body, why).expect("the minutes' end is always said")
}

/// Said when a transcript made again replaced one the trash refused: kept beside it; or, the
/// new one failed (`not_put_back`), when the one there could not be put back: where it is.
pub fn transcript_kept_beside(language: Language, name: &str, not_put_back: bool) -> Notice {
    let (title, body) = match (language, not_put_back) {
        (Language::English, false) => (
            "The previous transcript is kept",
            format!("It could not go to the trash: it is beside the new one, as {name}."),
        ),
        (Language::French, false) => (
            "L’ancien transcript est gardé",
            format!("Il n’a pas pu aller à la corbeille : il est à côté du nouveau, sous {name}."),
        ),
        (Language::English, true) => (
            "The transcript could not be put back",
            format!("It is in the meeting’s folder, as {name}."),
        ),
        (Language::French, true) => (
            "Le transcript n’a pas pu être remis en place",
            format!("Il est dans le dossier de la réunion, sous {name}."),
        ),
    };
    Notice { title: title.into(), body }
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
/// Summit became Wholeheard (D28): said once, at the first start, when something was carried over.
pub fn carried_over(language: Language) -> Notice {
    let (title, body) = match language {
        Language::English => (
            "Summit is now Wholeheard",
            "Your settings, your meetings and the transcription model are carried over; the keyboard shortcut may be asked once more.",
        ),
        Language::French => (
            "Summit s’appelle maintenant Wholeheard",
            "Vos réglages, vos réunions et le modèle de transcription sont repris\u{a0}; le raccourci clavier peut être redemandé une fois.",
        ),
    };
    Notice {
        title: title.into(),
        body: body.into(),
    }
}

pub fn model_needed(language: Language, size: &str, host: &str) -> Notice {
    let entry = download(language, size, host);
    let (title, body) = match language {
        Language::English => (
            "The transcription model is needed",
            format!("It is fetched once, on your word: choose “{entry}” in Wholeheard’s menu. The recording is kept."),
        ),
        Language::French => (
            "Le modèle de transcription est nécessaire",
            format!(
                "Il est téléchargé une fois, sur votre accord\u{a0}: choisissez «\u{a0}{entry}\u{a0}» dans le menu de Wholeheard. L’enregistrement est conservé."
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
