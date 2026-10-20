//! One meeting's minutes, read from folders made here.

use std::fs;

use wholeheard_shell::{meeting::Read, minutes};

const MINUTES: &str = "---
contract: minutes/v1
made: 2026-10-13T15:12:40+02:00
provider: local
model: mistral
context: null
---

## Points forts

La date de l'événement est fixée au jeudi 3 juin,
avec le 10 en seconde date.

## Actions

- Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril
- Pré-réserver les salles — non précisé — non précisé

## Points clés

- Réponse des intervenants attendue pour le 26 avril ; un plan B reste à trouver.
";

fn read(text: &str) -> Read<minutes::Minutes> {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("minutes.md"), text).unwrap();
    minutes::minutes(folder.path())
}

#[test]
fn the_minutes_are_shown_as_written_with_who_made_them() {
    let Read::Present(minutes) = read(MINUTES) else {
        panic!("not read")
    };
    assert_eq!(
        (minutes.provider.as_deref(), minutes.model.as_deref()),
        (Some("local"), Some("mistral"))
    );
    assert_eq!(minutes.made.as_deref(), Some("2026-10-13T15:12:40+02:00"));
    for shown in [
        "<h2>Points forts</h2>",
        "<li>Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril</li>",
        "<li>Pré-réserver les salles — non précisé — non précisé</li>",
        "<h2>Points clés</h2>",
    ] {
        assert!(minutes.body.contains(shown), "{shown} not in {}", minutes.body);
    }
}

#[test]
fn whatever_the_producer_writes_is_shown_nothing_left_out() {
    let written = "---
contract: minutes/v1
made: 2026-10-13T15:12:40+02:00
provider: \"api.example.org\"
model: \"mod\\\"el\\nname\"
context: null
---

Faits marquants
---------------

- La date est fixée.
- Le budget aussi.

  ## Actions ##

* Appeler les intervenants

  Alexandre — 26 avril

Aucune autre action n'a été décidée.

## Points clés

Pour commencer :

1. Un premier point
2. Un second

Reste ouvert : la salle.
";
    let Read::Present(minutes) = read(written) else {
        panic!("not read")
    };
    assert_eq!(minutes.provider.as_deref(), Some("api.example.org"));
    assert_eq!(minutes.model.as_deref(), Some("mod\"el\nname"));
    for shown in [
        "<h2>Faits marquants</h2>",
        "La date est fixée.",
        "<h2>Actions</h2>",
        "Appeler les intervenants",
        "Alexandre — 26 avril",
        "Aucune autre action n'a été décidée.",
        "Pour commencer :",
        "<li>Un second</li>",
        "Reste ouvert : la salle.",
    ] {
        assert!(minutes.body.contains(shown), "{shown} not in {}", minutes.body);
    }
}

#[test]
fn the_minutes_never_drive_the_page_that_shows_them() {
    let written = MINUTES.replace(
        "avec le 10 en seconde date.",
        "avec le 10 en seconde date. <script>alert(1)</script> [ici](javascript:alert(1)) ![une image](file:///etc/passwd)",
    );
    let Read::Present(minutes) = read(&written) else {
        panic!("not read")
    };
    assert!(!minutes.body.contains("<script"), "{}", minutes.body);
    assert!(minutes.body.contains("&lt;script&gt;alert(1)&lt;/script&gt;"), "{}", minutes.body);
    assert!(!minutes.body.contains("<a ") && !minutes.body.contains("<img"), "{}", minutes.body);
    assert!(
        minutes.body.contains(" ici ") && minutes.body.contains("une image"),
        "{}",
        minutes.body
    );
    // A block of HTML, as a hand may write one, is text too.
    let block = MINUTES.replace("## Actions\n", "## Actions\n\n<div onclick=\"x()\">bloc</div>\n");
    let Read::Present(minutes) = read(&block) else { panic!("not read") };
    assert!(
        !minutes.body.contains("<div") && minutes.body.contains("&lt;div"),
        "{}",
        minutes.body
    );
}

#[test]
fn minutes_that_are_not_of_this_version_are_said_unreadable_and_none_are_absent() {
    let folder = tempfile::tempdir().unwrap();
    assert_eq!(minutes::minutes(folder.path()), Read::Absent);
    for wrong in [
        MINUTES.replacen("---\ncontract", "contract", 1),
        MINUTES.replace("minutes/v1", "minutes/v2"),
        MINUTES.replace("## Points clés", "Points clés"),
        format!("{MINUTES}\n## Encore\n"),
    ] {
        assert_eq!(read(&wrong), Read::Unreadable, "{wrong}");
    }
    // A sub-title is no section: three sections and a title below second level read.
    assert!(matches!(
        read(&MINUTES.replace("## Actions\n", "## Actions\n\n### Un détail\n")),
        Read::Present(_)
    ));
    // A value written `null`, quoted or not, is no value.
    let Read::Present(minutes) = read(&MINUTES.replace("model: mistral", "model: \"null\"")) else {
        panic!("not read")
    };
    assert_eq!(minutes.model, None);
}
