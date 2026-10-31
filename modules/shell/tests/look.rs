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
const GLYPHS: [char; 7] = ['›', '‹', '⤢', '✓', '✔', '→', '×'];
/// The same, written otherwise.
const ESCAPES: [&str; 12] = [
    "\\203A", "\\2039", "\\2922", "\\2713", "\\2714", "\\2192", "\\D7", "&times;", "&rarr;", "&rsaquo;", "&lsaquo;", "&#",
];

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
    for escape in ESCAPES {
        assert!(!style.to_uppercase().contains(&escape.to_uppercase()), "{escape} in the style");
        assert!(!HTML.contains(escape), "{escape} in the page's frame");
    }
    assert_eq!(
        glyphs_in_page(r#"const a = `✓ ${b}`; const c = "→"; const d = `✓`; // ✓ in a comment"#).len(),
        3
    );
}

#[test]
fn nothing_is_fetched_for_an_icon() {
    let style = without_css_comments(STYLE);
    assert!(
        !style.contains("url("),
        "the page's policy refuses it, and a test of a value would not see it"
    );
    assert!(!HTML.contains("src=\"http") && !HTML.contains("href=\"http"));
    assert!(include_str!("../ui/LUCIDE-LICENSE").contains("ISC License"));
}

/// The one rule whose opacity stays: a line's pencil, no text, faint until its line is hovered or
/// focused (D62), each state then at full strength.
const FADED_ON_PURPOSE: &str = ".lines .edit";

type Rgb = [f64; 3];

fn hex(text: &str) -> Rgb {
    let digit = |at: usize| f64::from(u8::from_str_radix(&text[at..at + 2], 16).unwrap());
    [digit(1), digit(3), digit(5)]
}

/// The tokens in one block: one this test cannot read is refused, or it would say nothing of all that use it.
fn tokens(block: &str) -> BTreeMap<String, Rgb> {
    block
        .split(';')
        .filter_map(|one| one.split_once(':'))
        .filter(|(name, _)| name.trim().starts_with("--"))
        .map(|(name, value)| {
            let value = value.trim();
            assert!(value.len() == 7 && value.starts_with('#'), "{name}: {value} is no #RRGGBB");
            (name.trim().to_string(), hex(value))
        })
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

/// A value of the style in a theme: `#RRGGBB`, a token, or `color-mix` of two as the style writes it.
fn value(text: &str, theme: &BTreeMap<String, Rgb>) -> Option<Rgb> {
    let text = text.trim().trim_end_matches("!important").trim();
    if text.len() == 7 && text.starts_with('#') {
        return Some(hex(text));
    }
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
/// is built of. 4.5:1; 3:1 from 24 px. What it cannot read in a rule that names a text's colour is
/// reported too, never passed over. `inherit` and `currentColor` name none, `transparent` and `none`
/// no background. A background alone says nothing of the text inherited over it: a bar holds none.
fn too_faint(style: &str) -> Vec<String> {
    let style = without_css_comments(style);
    let mut faint = Vec::new();
    for (index, theme) in themes(&style).iter().enumerate() {
        for rule in style.split('}') {
            let Some((selector, body)) = rule.rsplit_once('{') else { continue };
            let selector = selector.trim();
            // The last one is what holds.
            let property = |name: &str| {
                body.split(';')
                    .filter_map(|one| one.split_once(':'))
                    .rfind(|(key, _)| key.trim() == name)
                    .map(|(_, found)| found.trim().to_string())
            };
            let colour = property("-webkit-text-fill-color").or_else(|| property("color"));
            let Some(colour) = colour else { continue };
            if ["inherit", "currentColor", "transparent"].contains(&colour.as_str()) {
                continue;
            }
            let Some(colour) = value(&colour, theme) else {
                faint.push(format!("{selector}: its colour {colour} cannot be read"));
                continue;
            };
            if selector != FADED_ON_PURPOSE
                && property("opacity").is_some_and(|opacity| opacity.parse::<f64>().is_ok_and(|opacity| opacity < 1.0))
            {
                faint.push(format!("{selector}: text with an opacity cannot be read"));
                continue;
            }
            let large = property("font-size")
                .and_then(|size| size.strip_suffix("px").and_then(|px| px.parse::<f64>().ok()))
                .is_some_and(|px| px >= 24.0);
            let need = if large { 3.0 } else { 4.5 };
            let ground = property("background-color")
                .or_else(|| property("background"))
                .filter(|text| !["transparent", "none", "inherit"].contains(&text.as_str()));
            let grounds: Vec<Rgb> = match ground {
                Some(own) => match value(own.split_whitespace().next().unwrap_or_default(), theme).or_else(|| value(&own, theme)) {
                    Some(own) => vec![own],
                    None => {
                        faint.push(format!("{selector}: its background {own} cannot be read"));
                        continue;
                    }
                },
                None => ["--ground", "--surface", "--raised"].map(|name| theme[name]).to_vec(),
            };
            for ground in grounds {
                let ratio = contrast(colour, ground);
                if ratio < need {
                    faint.push(format!("theme {index}, {selector}: {ratio:.2}"));
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
    let unread = ":root { --ground: #FFFFFF; --surface: #FFFFFF; --raised: #FFFFFF; --ink: #000000; }\n\
                  @media (prefers-color-scheme: dark) { :root { --ground: #000000; --surface: #000000; --raised: #000000; --ink: #FFFFFF; } }\n\
                  .a { color: rgb(1, 2, 3); }\n.b { color: var(--ink); background: linear-gradient(red, blue); }\n.c { color: var(--ink); opacity: 0.2; }\n\
                  .d { color: var(--ink); -webkit-text-fill-color: #888888; background-color: #888888; }\n.e { color: var(--ink); color: #FEFEFE; background: #FFFFFF; }";
    let refused = too_faint(unread);
    for selector in [".a", ".b", ".c", ".d", ".e"] {
        assert!(refused.iter().any(|line| line.contains(selector)), "{selector}: {refused:?}");
    }
}

#[test]
fn what_moves_moves_shortly_and_not_for_who_asked_it_not_to() {
    let style = without_css_comments(STYLE);
    for selector in [".small-rest, .small-live {", ".chip.turned {"] {
        let rule = style
            .split(selector)
            .skip(1)
            .map(|part| part.split('}').next().unwrap())
            .find(|rule| rule.contains("animation"))
            .unwrap_or_else(|| panic!("{selector}"));
        let animation = rule
            .trim()
            .strip_prefix("animation:")
            .unwrap_or_else(|| panic!("{selector}: {rule}"))
            .trim()
            .trim_end_matches(';');
        // One name, one length, one ease: nothing repeating, nothing waiting.
        let parts: Vec<&str> = animation.split_whitespace().collect();
        assert_eq!(parts.len(), 3, "{selector}: {animation}");
        let ms: u32 = parts[1]
            .strip_suffix("ms")
            .unwrap_or_else(|| panic!("{animation}"))
            .parse()
            .unwrap();
        assert!(ms <= 200, "{selector} {ms} ms");
    }
    assert!(style.contains("prefers-reduced-motion: reduce) { .small-rest, .small-live, .chip.turned { animation: none; } }"));
}
