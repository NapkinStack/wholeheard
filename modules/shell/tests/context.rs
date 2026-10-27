//! A meeting's context (D59): `context.txt` as the window writes it, when the field is closed,
//! and when what the meeting was made with is another context.

use std::{fs, io::ErrorKind, path::Path};

use wholeheard_shell::{
    context::{self, Closed},
    meeting, minutes,
};

const SECTIONS: [&str; 3] = ["Points saillants", "Actions", "Points clés"];

fn meeting_folder() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    fs::write(
        folder.path().join("session.json"),
        r#"{"contract":"session-audio/v2","started":"2026-10-27T14:05:00+01:00","tracks":{}}"#,
    )
    .unwrap();
    folder
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn transcript(folder: &Path, context: Option<&str>, complete: bool) {
    let context = context.map_or("null".to_string(), |text| serde_json::to_string(text).unwrap());
    let mut text = format!(
        "{{\"type\":\"header\",\"contract\":\"transcript/v1\",\"engine\":\"whisper.cpp\",\"model\":\"m\",\"language\":null,\"context\":{context}}}\n\
         {{\"type\":\"segment\",\"speaker\":\"me\",\"track\":\"local\",\"start\":0.0,\"end\":1.0,\"text\":\"Bonjour.\"}}\n"
    );
    if complete {
        text.push_str("{\"type\":\"complete\",\"duration\":1.0}\n");
    }
    fs::write(folder.join("transcript.jsonl"), text).unwrap();
}

fn minutes_with(folder: &Path, context: Option<&str>) {
    let context = context.map_or("null".to_string(), |text| serde_json::to_string(text).unwrap());
    fs::write(
        folder.join("minutes.md"),
        format!(
            "---\ncontract: minutes/v1\nmade: 2026-10-27T15:00:00+01:00\nprovider: \"local\"\nmodel: \"m\"\ncontext: {context}\n---\n\n## A\n\nx\n\n## B\n\n- y\n\n## C\n\n- z\n"
        ),
    )
    .unwrap();
}

#[test]
fn the_context_is_written_whole_and_nothing_is_left_beside_it() {
    let folder = meeting_folder();
    let said = "Budget du T4 avec Inès Durand, Jiří Novák et l’équipe achats.\nDeux lignes.";
    context::write(folder.path(), said).unwrap();
    assert_eq!(fs::read_to_string(folder.path().join("context.txt")).unwrap(), said);
    assert_eq!(context::read(folder.path()).as_deref(), Some(said));
    // Written again, replaced whole: the file beside is renamed over it, never left.
    context::write(folder.path(), "Comité produit.").unwrap();
    assert_eq!(fs::read_to_string(folder.path().join("context.txt")).unwrap(), "Comité produit.");
    assert_eq!(names(folder.path()), ["context.txt", "session.json"]);
}

#[test]
fn a_context_emptied_leaves_no_file() {
    let folder = meeting_folder();
    context::write(folder.path(), "Quelqu’un.").unwrap();
    context::write(folder.path(), "  \n ").unwrap();
    assert_eq!(names(folder.path()), ["session.json"]);
    assert_eq!(context::read(folder.path()), None);
    // Nothing there and nothing written: still nothing, and no failure.
    context::write(folder.path(), "").unwrap();
    assert_eq!(names(folder.path()), ["session.json"]);
}

#[test]
fn no_context_is_written_before_the_meeting_s_folder_holds_its_session() {
    let folder = tempfile::tempdir().unwrap();
    let refused = context::write(folder.path(), "Trop tôt.").unwrap_err();
    assert_eq!(refused.kind(), ErrorKind::NotFound);
    assert!(names(folder.path()).is_empty(), "{:?}", names(folder.path()));
    // A folder that is not there either.
    assert!(context::write(&folder.path().join("absent"), "Trop tôt.").is_err());
    assert!(!folder.path().join("absent").exists());
}

#[test]
fn four_hundred_characters_are_taken_and_one_more_is_refused() {
    let folder = meeting_folder();
    // Counted in characters, as the field counts them — not in bytes: each of these is two or more.
    for one in ["é", "Ж", "名", "🙂"] {
        let most = one.repeat(context::MOST);
        context::write(folder.path(), &most).unwrap();
        assert_eq!(fs::read_to_string(folder.path().join("context.txt")).unwrap(), most);
        let refused = context::write(folder.path(), &format!("{most}{one}")).unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InvalidInput);
        // Refused, what was there stays, and nothing is left beside it.
        assert_eq!(fs::read_to_string(folder.path().join("context.txt")).unwrap(), most);
        assert_eq!(names(folder.path()), ["context.txt", "session.json"]);
    }
    assert_eq!(context::MOST, 400);
}

#[test]
fn the_field_is_closed_while_a_making_runs_or_a_transcript_is_left_unfinished() {
    let folder = meeting_folder();
    // Nothing made yet, nothing being made: open — the meeting being recorded too.
    assert_eq!(context::closed(false, &meeting::transcript(folder.path())), None);
    // Its transcript or its minutes being made: closed, whatever is there.
    assert_eq!(context::closed(true, &meeting::transcript(folder.path())), Some(Closed::Making));
    transcript(folder.path(), None, false);
    assert_eq!(context::closed(true, &meeting::transcript(folder.path())), Some(Closed::Making));
    // A transcript left unfinished, nothing making it: closed — continued with another context,
    // it would be refused.
    assert_eq!(
        context::closed(false, &meeting::transcript(folder.path())),
        Some(Closed::Unfinished)
    );
    transcript(folder.path(), None, true);
    assert_eq!(context::closed(false, &meeting::transcript(folder.path())), None);
}

#[test]
fn the_context_changed_is_one_the_transcript_or_the_minutes_do_not_name() {
    let folder = meeting_folder();
    let changed = |folder: &Path| {
        context::changed(
            context::read(folder).as_deref(),
            &meeting::transcript(folder),
            &minutes::minutes(folder, SECTIONS),
        )
    };
    // Nothing made: nothing to make again.
    context::write(folder.path(), "Inès et Jiří.").unwrap();
    assert!(!changed(folder.path()));
    // Made with it — the step trims it as it reads it —: the same.
    transcript(folder.path(), Some("Inès et Jiří."), true);
    context::write(folder.path(), "  Inès et Jiří.\n").unwrap();
    assert!(!changed(folder.path()));
    // Changed after.
    context::write(folder.path(), "Inès, Jiří et Siobhán.").unwrap();
    assert!(changed(folder.path()));
    // Emptied: the transcript names one, so it changed; made with none, the same.
    context::write(folder.path(), "").unwrap();
    assert!(changed(folder.path()));
    transcript(folder.path(), None, true);
    assert!(!changed(folder.path()));
    // The minutes made with another than the transcript's: changed too.
    minutes_with(folder.path(), Some("Avant."));
    assert!(changed(folder.path()));
    minutes_with(folder.path(), None);
    assert!(!changed(folder.path()));
}

#[test]
fn the_readers_say_the_context_a_transcript_and_minutes_were_made_with() {
    let folder = meeting_folder();
    transcript(folder.path(), Some("Quatre personnes, « réunion » d’organisation."), true);
    minutes_with(folder.path(), Some("Quatre personnes, « réunion » d’organisation."));
    let meeting::Read::Present(heard) = meeting::transcript(folder.path()) else {
        panic!("no transcript read");
    };
    let meeting::Read::Present(written) = minutes::minutes(folder.path(), SECTIONS) else {
        panic!("no minutes read");
    };
    assert_eq!(heard.context.as_deref(), Some("Quatre personnes, « réunion » d’organisation."));
    assert_eq!(written.context.as_deref(), Some("Quatre personnes, « réunion » d’organisation."));
}
