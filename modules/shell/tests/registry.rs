//! The list of models Settings offer (D40, ADR-0010): the file in the repository is the list
//! shipped, and a list that does not read as one is refused whole — in a pull request by these
//! tests, and in Wholeheard by the same reading.

use wholeheard_shell::model;
use wholeheard_shell::registry::{self, Good};

/// A list of one model, with `entry` as its only line's fields.
fn one(entry: &str) -> String {
    format!(r#"{{"version": 1, "whisper": [{{{entry}}}]}}"#)
}

const FINGERPRINT: &str = "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f";

fn entry(address: &str, bytes: &str, sha256: &str) -> String {
    format!(r#""address": "{address}", "bytes": {bytes}, "sha256": "{sha256}", "good": "faster""#)
}

#[test]
fn the_file_in_the_repository_is_the_list_shipped_and_it_reads() {
    let shipped = registry::read(registry::SHIPPED).expect("the list in modules/shell/registry reads");
    assert_eq!(shipped, registry::shipped());
    assert!(!shipped.is_empty());
    // The default model (ADR-0003) is offered first, said recommended.
    assert_eq!(shipped[0].published, model::DEFAULT);
    assert_eq!(shipped[0].good, Some(Good::Recommended));
    assert!(shipped.iter().all(|listed| registry::HOSTS.contains(&listed.published.host)));
    // Whisper small, the faster one, as D37 offered it.
    assert_eq!(shipped.len(), 2);
    assert_eq!(model::file(&shipped[1].published), "ggml-small-q8_0.bin");
    assert_eq!(shipped[1].published.bytes, 264_464_607);
    assert_eq!(shipped[1].published.sha256, FINGERPRINT);
    assert_eq!(shipped[1].good, Some(Good::Faster));
}

#[test]
fn a_model_reads_with_its_host_from_its_address() {
    let listed = registry::read(&one(&entry(
        "https://huggingface.co/a/b/resolve/main/ggml-x.bin",
        "264464607",
        FINGERPRINT,
    )))
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].published.host, "huggingface.co");
    assert_eq!(listed[0].published.bytes, 264_464_607);
    assert_eq!(listed[0].published.sha256, FINGERPRINT);
    assert_eq!(model::file(&listed[0].published), "ggml-x.bin");
    assert_eq!(listed[0].good, Some(Good::Faster));
}

#[test]
fn what_it_is_good_for_unknown_to_this_build_shows_the_model_without_a_sentence() {
    let text = one(
        r#""address": "https://huggingface.co/m.bin", "bytes": 1, "sha256": "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f", "good": "for-a-later-build""#,
    );
    assert_eq!(registry::read(&text).unwrap()[0].good, None);
    let without = one(
        r#""address": "https://huggingface.co/m.bin", "bytes": 1, "sha256": "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f""#,
    );
    assert_eq!(registry::read(&without).unwrap()[0].good, None);
}

#[test]
fn a_field_a_later_list_adds_is_read_past() {
    let text = r#"{"version": 1, "notes": "kept by NapkinStack", "whisper": [{"address": "https://huggingface.co/m.bin", "bytes": 1, "sha256": "49c8fb02b65e6049d5fa6c04f81f53b867b5ec9540406812c643f177317f779f", "languages": ["fr", "en"]}]}"#;
    assert_eq!(registry::read(text).unwrap().len(), 1);
}

#[test]
fn a_list_that_is_not_one_is_refused_whole() {
    let good = entry("https://huggingface.co/m.bin", "1", FINGERPRINT);
    let refused = [
        ("not JSON", "{ the list".to_string()),
        ("no version", r#"{"whisper": []}"#.to_string()),
        ("a version unknown", one(&good).replace(r#""version": 1"#, r#""version": 2"#)),
        ("no models", r#"{"version": 1, "whisper": []}"#.to_string()),
        ("another host", one(&entry("https://example.com/m.bin", "1", FINGERPRINT))),
        (
            "a host's look-alike",
            one(&entry("https://huggingface.co.example.com/m.bin", "1", FINGERPRINT)),
        ),
        ("not over HTTPS", one(&entry("http://huggingface.co/m.bin", "1", FINGERPRINT))),
        ("no file", one(&entry("https://huggingface.co/", "1", FINGERPRINT))),
        (
            "a file that is the folder above",
            one(&entry("https://huggingface.co/a/..", "1", FINGERPRINT)),
        ),
        (
            "a file that is the folder itself",
            one(&entry("https://huggingface.co/a/.", "1", FINGERPRINT)),
        ),
        (
            "a query",
            one(&entry("https://huggingface.co/a/m.bin?download=true", "1", FINGERPRINT)),
        ),
        ("a fragment", one(&entry("https://huggingface.co/a/m.bin#x", "1", FINGERPRINT))),
        (
            "a query before the file",
            one(&entry("https://huggingface.co/a?x=/m.bin", "1", FINGERPRINT)),
        ),
        (
            "a file with a separator of another system",
            one(&entry("https://huggingface.co/a/m\\\\.bin", "1", FINGERPRINT)),
        ),
        ("a size of zero", one(&entry("https://huggingface.co/m.bin", "0", FINGERPRINT))),
        (
            "a size that is no number",
            one(&entry("https://huggingface.co/m.bin", "\"264 MB\"", FINGERPRINT)),
        ),
        (
            "a fingerprint too short",
            one(&entry("https://huggingface.co/m.bin", "1", &FINGERPRINT[1..])),
        ),
        (
            "a fingerprint not hexadecimal",
            one(&entry("https://huggingface.co/m.bin", "1", &FINGERPRINT.replace('4', "g"))),
        ),
        (
            "a fingerprint in capitals",
            one(&entry("https://huggingface.co/m.bin", "1", &FINGERPRINT.to_uppercase())),
        ),
        (
            "one file twice",
            format!(
                r#"{{"version": 1, "whisper": [{{{good}}}, {{{}}}]}}"#,
                entry("https://huggingface.co/other/m.bin", "2", FINGERPRINT)
            ),
        ),
    ];
    for (what, text) in refused {
        assert!(registry::read(&text).is_err(), "{what} should be refused: {text}");
    }
}

#[test]
fn a_list_larger_than_64_kb_is_refused_before_it_is_read() {
    let padding = " ".repeat(registry::LIMIT);
    let text = format!("{}{padding}", one(&entry("https://huggingface.co/m.bin", "1", FINGERPRINT)));
    assert!(registry::read(&text).is_err());
}
