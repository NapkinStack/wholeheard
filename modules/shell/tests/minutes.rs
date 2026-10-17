//! One meeting's minutes, read from folders made here.

use std::fs;

use summit_shell::{
    meeting::Read,
    minutes::{self, Action},
};

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

#[test]
fn the_minutes_are_read_by_the_place_of_their_sections() {
    let folder = tempfile::tempdir().unwrap();
    fs::write(folder.path().join("minutes.md"), MINUTES).unwrap();
    let Read::Present(minutes) = minutes::minutes(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(minutes.titles, ["Points forts", "Actions", "Points clés"].map(String::from));
    assert_eq!(
        minutes.highlights,
        "La date de l'événement est fixée au jeudi 3 juin, avec le 10 en seconde date."
    );
    assert_eq!(
        minutes.actions,
        [
            Action {
                what: "Contacter les trois intervenants et le comité scientifique".into(),
                who: "Alexandre".into(),
                when: "26 avril".into()
            },
            Action {
                what: "Pré-réserver les salles".into(),
                who: "non précisé".into(),
                when: "non précisé".into()
            },
        ]
    );
    assert_eq!(
        minutes.key_points,
        ["Réponse des intervenants attendue pour le 26 avril ; un plan B reste à trouver."]
    );
    assert_eq!(
        (minutes.provider.as_deref(), minutes.model.as_deref()),
        (Some("local"), Some("mistral"))
    );
    assert_eq!(minutes.made.as_deref(), Some("2026-10-13T15:12:40+02:00"));
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
        fs::write(folder.path().join("minutes.md"), &wrong).unwrap();
        assert_eq!(minutes::minutes(folder.path()), Read::Unreadable, "{wrong}");
    }
}

#[test]
fn the_minutes_are_read_as_summit_summary_writes_them_and_as_commonmark_reads_them() {
    let folder = tempfile::tempdir().unwrap();
    // The front matter as the producer writes it: quoted, escaped.
    let written = "---
contract: minutes/v1
made: 2026-10-13T15:12:40+02:00
provider: \"api.example.org\"
model: \"mod\\\"el\\nname\"
context: null
---

Faits marquants
---------------

La date est fixée,
en juin.

### Un sous-titre

---

  ## Actions ##

* Réserver — la salle — Alexandre — 26 avril
* Prévoir
  le café — Lou — demain
  - un détail, dans l'item

## Points clés

| Sujet | État |
| --- | --- |
| Budget | ouvert |
";
    fs::write(folder.path().join("minutes.md"), written).unwrap();
    let Read::Present(minutes) = minutes::minutes(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(minutes.provider.as_deref(), Some("api.example.org"));
    assert_eq!(minutes.model.as_deref(), Some("mod\"el\nname"));
    assert_eq!(minutes.titles, ["Faits marquants", "Actions", "Points clés"].map(String::from));
    // The sub-title and the rule are not the highlights' sentences.
    assert_eq!(minutes.highlights, "La date est fixée, en juin.");
    assert_eq!(
        minutes.actions,
        [
            Action {
                what: "Réserver — la salle".into(),
                who: "Alexandre".into(),
                when: "26 avril".into()
            },
            Action {
                what: "Prévoir le café".into(),
                who: "Lou".into(),
                when: "demain".into()
            },
        ]
    );
    // A key point not written as a list is kept as written.
    assert_eq!(minutes.key_points.len(), 1);
    assert!(minutes.key_points[0].contains("Budget"));

    for ordered in ["1. Faire — Lou — lundi\n", "+ Faire — Lou — lundi\n"] {
        fs::write(folder.path().join("minutes.md"), MINUTES.replace("- Contacter les trois intervenants et le comité scientifique — Alexandre — 26 avril\n- Pré-réserver les salles — non précisé — non précisé\n", ordered)).unwrap();
        let Read::Present(minutes) = minutes::minutes(folder.path()) else {
            panic!("not read: {ordered}")
        };
        assert_eq!(
            minutes.actions,
            [Action {
                what: "Faire".into(),
                who: "Lou".into(),
                when: "lundi".into()
            }],
            "{ordered}"
        );
    }
    // A value written `null`, quoted or not, is no value.
    fs::write(
        folder.path().join("minutes.md"),
        MINUTES.replace("model: mistral", "model: \"null\""),
    )
    .unwrap();
    let Read::Present(minutes) = minutes::minutes(folder.path()) else {
        panic!("not read")
    };
    assert_eq!(minutes.model, None);
}
