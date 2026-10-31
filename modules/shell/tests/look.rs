//! D69: the page draws its icons, and each theme's text reads on what it is written on, from the files.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{StringLiteral, TemplateElement};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;

const PAGE: &str = include_str!("../ui/window.js");
const STYLE: &str = include_str!("../ui/window.css");
const HTML: &str = include_str!("../ui/index.html");
/// The glyphs a font would draw as an icon (UX review §8), and the back arrow beside them.
const GLYPHS: [char; 6] = ['›', '‹', '⤢', '✓', '→', '×'];

#[derive(Default)]
struct Texts(Vec<String>);

impl<'a> Visit<'a> for Texts {
    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.0.push(literal.value.as_str().unwrap_or_default().to_string());
    }

    fn visit_template_element(&mut self, element: &TemplateElement<'a>) {
        self.0.push(element.value.raw.as_str().to_string());
    }
}

fn without_css_comments(mut css: &str) -> String {
    let mut kept = String::new();
    while let Some((before, rest)) = css.split_once("/*") {
        kept.push_str(before);
        css = rest.split_once("*/").map_or("", |(_, after)| after);
    }
    kept + css
}

fn glyphs_in_page(page: &str) -> Vec<String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, page, SourceType::mjs()).parse();
    assert!(parsed.diagnostics.is_empty(), "the page does not parse");
    let mut texts = Texts::default();
    texts.visit_program(&parsed.program);
    texts.0.into_iter().filter(|text| text.contains(GLYPHS)).collect()
}

#[test]
fn no_glyph_stands_for_an_icon_in_the_page() {
    assert_eq!(glyphs_in_page(PAGE), Vec::<String>::new());
    let style = without_css_comments(STYLE);
    assert!(!style.contains(GLYPHS), "a glyph in the style");
    assert!(!HTML.contains(GLYPHS), "a glyph in the page's frame");
    assert_eq!(glyphs_in_page(r#"const a = `✓ ${b}`; const c = "→"; // ✓ in a comment"#).len(), 2);
}

#[test]
fn nothing_is_fetched_for_an_icon() {
    for file in [PAGE, STYLE, HTML] {
        for line in file.lines().filter(|line| line.contains("<svg") || line.contains("url(")) {
            assert!(
                !line.contains("url(\"http") && !line.contains("href=\"http") && !line.contains("src=\"http"),
                "{line}"
            );
        }
    }
    assert!(include_str!("../ui/LUCIDE-LICENSE").contains("ISC License"));
}

type Rgb = [f64; 3];

fn hex(text: &str) -> Rgb {
    let digit = |at: usize| f64::from(u8::from_str_radix(&text[at..at + 2], 16).unwrap());
    [digit(1), digit(3), digit(5)]
}

/// The tokens in one block.
fn tokens(block: &str) -> BTreeMap<String, Rgb> {
    block
        .split(';')
        .filter_map(|one| one.split_once(':'))
        .filter(|(name, value)| name.trim().starts_with("--") && value.trim().starts_with('#') && value.trim().len() == 7)
        .map(|(name, value)| (name.trim().to_string(), hex(value.trim())))
        .collect()
}

/// Each theme's tokens, read from the file.
fn themes(style: &str) -> [BTreeMap<String, Rgb>; 2] {
    let light = style.split_once(":root {").unwrap().1.split_once('}').unwrap().0;
    let dark = style
        .split_once("prefers-color-scheme: dark")
        .unwrap()
        .1
        .split_once(":root {")
        .unwrap()
        .1
        .split_once('}')
        .unwrap()
        .0;
    [tokens(light), tokens(dark)]
}

/// A value of the style in a theme: a token, or `color-mix` of two as the style writes it.
fn value(text: &str, theme: &BTreeMap<String, Rgb>) -> Option<Rgb> {
    let text = text.trim().trim_end_matches("!important").trim();
    if let Some(name) = text.strip_prefix("var(").and_then(|rest| rest.strip_suffix(')')) {
        return theme.get(name).copied();
    }
    let mix = text.strip_prefix("color-mix(in srgb,")?.strip_suffix(')')?;
    let (first, second) = mix.split_once(',')?;
    let (first, share) = first.trim().rsplit_once(' ')?;
    let share = share.strip_suffix('%')?.parse::<f64>().ok()? / 100.0;
    let (a, b) = (value(first, theme)?, value(second, theme)?);
    Some([0, 1, 2].map(|at| a[at] * share + b[at] * (1.0 - share)))
}

fn luminance(colour: Rgb) -> f64 {
    let [r, g, b] = colour.map(|channel| {
        let c = channel / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Each rule's text on its background; a rule naming no background is read on the three the window
/// is built of. 4.5:1, whatever its size: no text here is large enough to count on less, except a
/// rule of `font-size` 24 px or more, 3:1. An icon's colour or a `transparent` background is no pair.
fn too_faint(style: &str) -> Vec<String> {
    let style = without_css_comments(style);
    let mut faint = Vec::new();
    for (index, theme) in themes(&style).iter().enumerate() {
        for rule in style.split('}') {
            let Some((selector, body)) = rule.rsplit_once('{') else { continue };
            let property = |name: &str| {
                body.split(';')
                    .filter_map(|one| one.split_once(':'))
                    .find(|(key, _)| key.trim() == name)
                    .map(|(_, found)| found.trim().to_string())
            };
            let Some(colour) = property("color").and_then(|text| value(&text, theme)) else {
                continue;
            };
            let large = property("font-size")
                .and_then(|size| size.strip_suffix("px").and_then(|px| px.parse::<f64>().ok()))
                .is_some_and(|px| px >= 24.0);
            let need = if large { 3.0 } else { 4.5 };
            let grounds: Vec<Rgb> = match property("background").and_then(|text| value(&text, theme)) {
                Some(own) => vec![own],
                None => ["--ground", "--surface", "--raised"].map(|name| theme[name]).to_vec(),
            };
            for ground in grounds {
                let ratio = contrast(colour, ground);
                if ratio < need {
                    faint.push(format!("theme {index}, {}: {ratio:.2}", selector.trim()));
                }
            }
        }
    }
    faint
}

#[test]
fn each_themes_text_reaches_aa_on_what_it_is_written_on() {
    assert_eq!(too_faint(STYLE), Vec::<String>::new());
}

#[test]
fn a_pair_below_aa_is_refused() {
    let style = ":root { --ground: #FFFFFF; --surface: #FFFFFF; --raised: #FFFFFF; --ink: #000000; --pale: #BBBBBB; }\n\
                 @media (prefers-color-scheme: dark) { :root { --ground: #000000; --surface: #000000; --raised: #000000; --ink: #FFFFFF; --pale: #444444; } }\n\
                 .good { color: var(--ink); }\n.bad { color: var(--pale); }\n.big { color: var(--pale); font-size: 30px; }";
    let faint = too_faint(style);
    assert!(faint.iter().any(|line| line.contains(".bad")), "{faint:?}");
    assert!(!faint.iter().any(|line| line.contains(".good")), "{faint:?}");
    assert!((contrast([0.0; 3], [255.0; 3]) - 21.0).abs() < 0.01);
}

#[test]
fn what_moves_moves_shortly_and_not_for_who_asked_it_not_to() {
    let style = without_css_comments(STYLE);
    for (selector, longest) in [(".small-rest, .small-live {", 200), (".chip.turned {", 200)] {
        let rule = style
            .split(selector)
            .skip(1)
            .map(|part| part.split('}').next().unwrap())
            .find(|rule| rule.contains("animation"))
            .unwrap_or_else(|| panic!("{selector}"));
        let ms: u32 = rule.split("ms").next().unwrap().rsplit(' ').next().unwrap().parse().unwrap();
        assert!(ms <= longest, "{selector} {ms} ms");
    }
    let reduced = style
        .split("prefers-reduced-motion")
        .filter(|part| part.contains(".chip.turned"))
        .count();
    assert!(reduced >= 1 && style.contains(".small-rest, .small-live, .chip.turned { animation: none; }"));
}
