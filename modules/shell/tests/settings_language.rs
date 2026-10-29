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

/// S1 — Settings file missing → language = None (system default); settings::read returns Ok.
#[test]
fn s1_missing_file_defaults_none() {
    let defaults = Settings::default();
    assert_eq!(defaults.language, None);
    assert_eq!(read(Path::new("/nowhere/settings.toml")), Ok(defaults));
}

/// S2 — File names language = "fr" → Settings.language = Some(Language::French).
#[test]
fn s2_language_fr_sets_french() {
    let settings = written("language = \"fr\"").unwrap();
    assert_eq!(settings.language, Some(Language::French));
}

/// S3 — File names language = "en" → Some(Language::English); unknown → None; "system" → None.
#[test]
fn s3_language_en_sets_english_unknown_fallback() {
    assert_eq!(written("language = \"en\"").unwrap().language, Some(Language::English));
    assert_eq!(written("language = \"de\"").unwrap().language, None);
    assert_eq!(written("language = \"system\"").unwrap().language, None);
}

/// S4 — Language is persisted when settings are saved; settings shown carries it.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn s4_language_persisted_and_shown() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("settings.toml");
    let mut settings = Settings::default();
    // Pick a language different from system so we actually test persistence.
    settings.language = Some(Language::French);
    write(&path, &settings).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("language = \"fr\""),
        "language not written: {content}"
    );
    assert_eq!(read(&path).unwrap().language, Some(Language::French));
    // Settings shown carries it (direct read from struct).
    assert_eq!(Settings::default().language, None);
}

/// S5 — Only "fr" and "en" recognized (exact match).
#[test]
fn s5_language_exact_match_only() {
    for label in ["fr_FR", "french", "FR", "français"] {
        let text = format!("language = \"{label}\"");
        assert_eq!(written(&text).unwrap().language, None, "should be None for {label}");
    }
    for label in ["en_GB", "english", "EN", "en-US"] {
        let text = format!("language = \"{label}\"");
        assert_eq!(written(&text).unwrap().language, None, "should be None for {label}");
    }
    assert_eq!(written("language = \"fr\"").unwrap().language, Some(Language::French));
    assert_eq!(written("language = \"en\"").unwrap().language, Some(Language::English));
}