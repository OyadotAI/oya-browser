/** Line breaking rules of CSS Text in Eye: UAX #14 classes at element boundaries and before
 * space separators, `break-spaces`, CJK `line-break` and language rules, soft hyphens with
 * `hyphens` and `hyphenate-character`, look-ahead across elements, `text-autospace`, CJK
 * justification and the intrinsic sizes these change. */
use mars_eye::{DisplayItem, PaintLine, compute_styles, parse, render};

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;

/** The base sheet: no margins, a 16px monospace face. */
const BASE: &str = "body{margin:0;font:16px monospace}";

/** Every display item for `markup` styled by `sheet`. */
fn items(markup: &str, sheet: &str) -> Vec<DisplayItem> {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    render(&document, VIEWPORT).unwrap().items
}
/** Every painted run with text, top to bottom then left to right. */
fn runs(markup: &str, sheet: &str) -> Vec<PaintLine> {
    let mut lines: Vec<PaintLine> = items(markup, sheet)
        .into_iter()
        .flat_map(|i| i.lines)
        .filter(|l| !l.text.is_empty())
        .collect();
    lines.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    lines
}
/** The text of each line: the runs at each distinct top, joined. */
fn lines(markup: &str, sheet: &str) -> Vec<String> {
    let mut out: Vec<(f64, String)> = Vec::new();
    for run in runs(markup, sheet) {
        match out.last_mut().filter(|(y, _)| *y == run.y) {
            Some((_, text)) => text.push_str(&run.text),
            None => out.push((run.y, run.text)),
        }
    }
    out.into_iter().map(|(_, text)| text).collect()
}
/** The `#t` element's display item. */
fn target(markup: &str, sheet: &str) -> DisplayItem {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let display = render(&document, VIEWPORT).unwrap();
    display
        .items
        .into_iter()
        .find(|i| i.local_id == local)
        .unwrap()
}

/** UAX #14 LB21: a line never breaks before an ideographic space, which hangs instead. */
#[test]
fn no_break_before_an_ideographic_space() {
    let found = lines("<div style='width:1ch'>X\u{3000}</div>", BASE);
    assert_eq!(found, ["X\u{3000}"]);
}

/** Under `break-spaces` with `word-break: break-all` a preserved space never starts a line. */
#[test]
fn break_spaces_keep_a_space_off_the_line_start() {
    let sheet = format!("{BASE}div{{width:4ch;white-space:break-spaces;word-break:break-all}}");
    let found = lines("<div>X XX X</div>", &sheet);
    assert!(found.iter().all(|line| !line.starts_with(' ')), "{found:?}");
}

/** A preserved newline inside an inline child ends a line of the container's max-content. */
#[test]
fn preserved_newlines_end_max_content_lines() {
    let markup = "<div id=t style='float:left'><span style='white-space:pre-wrap'>AB\n</span><span> CD</span></div>";
    let one = target(markup, BASE).width;
    let two = target("<div id=t style='float:left'>AB</div>", BASE).width;
    assert!((one - two).abs() < 0.5, "{one} {two}");
}

/** Letters across an element boundary with no space between stay on one line. */
#[test]
fn letters_across_elements_do_not_break() {
    let found = lines("<div style='width:2ch'>abc<b>def</b></div>", BASE);
    assert_eq!(found, ["abcdef"]);
}

/** In Japanese, `line-break: normal` lets a line break before the wave dash; elsewhere not. */
#[test]
fn cjk_hyphens_break_only_in_cjk_languages() {
    let ja = lines("<div lang=ja style='width:2em'>中中\u{301c}</div>", BASE);
    let en = lines("<div style='width:2em'>中中\u{301c}</div>", BASE);
    assert_eq!(ja.len(), 2, "{ja:?}");
    assert_eq!(en.len(), 2, "{en:?}");
    assert!(ja[1].starts_with('\u{301c}'), "{ja:?}");
    assert!(!en[1].starts_with('\u{301c}'), "{en:?}");
}

/** `line-break: loose` lets a line break before an iteration mark. */
#[test]
fn loose_breaks_before_nonstarters() {
    let sheet = format!("{BASE}div{{width:2em}}");
    let loose = lines("<div style='line-break:loose'>中中\u{3005}</div>", &sheet);
    assert!(loose[1].starts_with('\u{3005}'), "{loose:?}");
    let bang = lines("<div style='line-break:loose'>中中\u{ff01}</div>", &sheet);
    assert!(bang[1].starts_with('\u{ff01}'), "{bang:?}");
    let normal = lines("<div>中中\u{3005}</div>", &sheet);
    assert!(!normal[1].starts_with('\u{3005}'), "{normal:?}");
}

/** A soft hyphen is a break opportunity that shows a hyphen when the line breaks there. */
#[test]
fn soft_hyphens_break_and_show_a_hyphen() {
    let found = lines("<div style='width:6ch'>high\u{ad}way</div>", BASE);
    assert_eq!(found, ["high\u{ad}\u{2010}", "way"]);
}

/** `hyphenate-character` replaces the hyphen; `hyphens: none` keeps the word whole. */
#[test]
fn hyphenate_character_and_hyphens_none() {
    let custom = lines(
        "<div style='width:6ch;hyphenate-character:\"=\"'>high\u{ad}way</div>",
        BASE,
    );
    assert_eq!(custom[0], "high\u{ad}=");
    let none = lines(
        "<div style='width:6ch;hyphens:none'>high\u{ad}way</div>",
        BASE,
    );
    assert_eq!(none, ["high\u{ad}way"]);
}

/** A word joined to the next element's letters breaks at its own last opportunity so they fit. */
#[test]
fn look_ahead_breaks_before_joined_letters() {
    let found = lines(
        "<div style='width:6ch'><span>high\u{ad}wa</span>y</div>",
        BASE,
    );
    assert_eq!(found, ["high\u{ad}\u{2010}", "way"]);
}

/** `overflow-wrap: anywhere` counts its breaks in min-content: the widest letter. */
#[test]
fn overflow_wrap_anywhere_min_content_is_a_letter() {
    let item = target(
        "<div id=t style='width:min-content;overflow-wrap:anywhere'>XXXX</div>",
        BASE,
    );
    let letter = target("<div id=t style='width:min-content'>X</div>", BASE);
    assert!((item.width - letter.width).abs() < 0.5, "{}", item.width);
}

/** `word-break: break-word` is `overflow-wrap: anywhere`, so its breaks count in min-content, and
an inheriting child takes the same min-content as the letter. */
#[test]
fn word_break_break_word_min_content_is_a_letter() {
    let item = target(
        "<div style='word-break:break-word'><div id=t style='width:min-content'>XXXX</div></div>",
        BASE,
    );
    let letter = target("<div id=t style='width:min-content'>X</div>", BASE);
    assert!((item.width - letter.width).abs() < 0.5, "{}", item.width);
}

/** A child's own `overflow-wrap: normal` overrides an inherited `word-break: break-word`: the child
has no letter breaks, so its min-content is the whole word. */
#[test]
fn word_break_break_word_is_overridden_by_child_overflow_wrap() {
    let item = target(
        "<div style='word-break:break-word'><div id=t style='width:min-content;overflow-wrap:normal'>XXXX</div></div>",
        BASE,
    );
    let word = target("<div id=t style='width:min-content'>XXXX</div>", BASE);
    let letter = target("<div id=t style='width:min-content'>X</div>", BASE);
    assert!((item.width - word.width).abs() < 0.5, "{}", item.width);
    assert!(item.width > letter.width + 1.0, "{}", item.width);
}

/** `word-break: break-all` counts every letter in min-content, as it did before `break-word` was
split out of it. */
#[test]
fn word_break_break_all_min_content_is_a_letter() {
    let item = target(
        "<div id=t style='width:min-content;word-break:break-all'>XXXX</div>",
        BASE,
    );
    let letter = target("<div id=t style='width:min-content'>X</div>", BASE);
    assert!((item.width - letter.width).abs() < 0.5, "{}", item.width);
}

/** `text-justify: auto` spreads CJK text between its ideographs. */
#[test]
fn auto_justification_spreads_between_ideographs() {
    let found = runs(
        "<div style='width:10em;text-align-last:justify'>東京</div>",
        BASE,
    );
    let end = found.iter().map(|r| r.x + r.width).fold(0.0, f64::max);
    assert!((end - 160.0).abs() < 0.5, "{found:?}");
}

/** `text-autospace` puts an eighth of an em between an ideograph and a Latin letter. */
#[test]
fn autospace_widens_ideograph_alpha_boundaries() {
    let spaced = target("<span id=t style='text-autospace:normal'>国X</span>", BASE);
    let plain = target(
        "<span id=t style='text-autospace:no-autospace'>国X</span>",
        BASE,
    );
    assert!(
        (spaced.width - plain.width - 2.0).abs() < 0.01,
        "{}",
        spaced.width
    );
}

/** `word-break: break-all` still never starts a line with a closing mark (UAX #14 LB13). */
#[test]
fn break_all_keeps_closing_punctuation_off_the_line_start() {
    let found = lines(
        "<div style='width:8ch;word-break:break-all'>XX XXX...</div>",
        BASE,
    );
    assert!(found.iter().all(|line| !line.starts_with('.')), "{found:?}");
}

/** A directional mark is an invisible control a line never breaks beside. */
#[test]
fn no_break_beside_a_directional_mark() {
    let found = lines(
        "<div style='width:1ch'>X\u{200e}<b>\u{200f}X</b></div>",
        BASE,
    );
    assert_eq!(found.len(), 1, "{found:?}");
}
