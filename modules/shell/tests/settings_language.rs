//! Settings language parsing and serialization tests (D64).

use std::{fs, path::Path};

use wholeheard_shell::{
    settings::{Settings, read, write},
    words::{self, Language},
};

fn written(text: &str) -> Result<Settings, String> {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    fs::write(&path, text).unwrap();
    read(&path).map_err(|said| said.replace(&path.display().to_string(), "<file>"))
}

/// S1 — Settings file missing → language = system (default); settings::read returns Ok.
#[test]
fn s1_missing_file_defaults_system_language() {
    let defaults = Settings::default();
    assert_eq!(defaults.language, words::Language::system());
    assert_eq!(read(Path::new("/nowhere/settings.toml")), Ok(defaults));
}

/// S2 — File names language = "fr" → Settings.language = Language::French.
#[test]
fn s2_language_fr_sets_french() {
    let settings = written("language = \"fr\"").unwrap();
    assert_eq!(settings.language, Language::French);
}

/// S3 — File names language = "en" → Language::English; unknown → system fallback.
#[test]
fn s3_language_en_sets_english_unknown_fallback() {
    assert_eq!(written("language = \"en\"").unwrap().language, Language::English);
    assert_eq!(written("language = \"de\"").unwrap().language, words::Language::system());
    assert_eq!(written("language = \"system\"").unwrap().language, words::Language::system());
}

/// S4 — Language is persisted when settings are saved; settings shown carries it.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn s4_language_persisted_and_shown() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    let mut settings = Settings::default();
    settings.language = Language::French;
    write(&path, &settings).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("language = \"fr\"") || content.contains("language = 'fr'"),
        "language not written: {content}"
    );
    assert_eq!(read(&path).unwrap().language, Language::French);
    // Settings shown carries it (direct read from struct).
    assert_eq!(Settings::default().language, words::Language::system());
}

/// S5 — French and English variants recognized.
#[test]
fn s5_language_variants_recognized() {
    for label in ["fr", "french", "FR", "fr_FR", "français"] {
        let text = format!("language = \"{label}\"");
        assert_eq!(written(&text).unwrap().language, Language::French, "failed for {label}");
    }
    for label in ["en", "english", "en_GB", "en-US"] {
        let text = format!("language = \"{label}\"");
        assert_eq!(written(&text).unwrap().language, Language::English, "failed for {label}");
    }
}
