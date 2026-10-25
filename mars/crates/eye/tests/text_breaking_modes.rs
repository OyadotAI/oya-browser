/** `word-break: break-all | keep-all`, `line-break: anywhere` and visible control characters in
generated content, after Blink's LineBreaker and LazyLineBreakIterator. */
use mars_eye::{DisplayItem, StyleSource, compute_styles, parse, render};

/** The painted lines of `#t` after laying out `markup` at 800px. */
fn lines(markup: &str) -> Vec<String> {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let item: &DisplayItem = display.items.iter().find(|i| i.local_id == local).unwrap();
    item.lines.iter().map(|line| line.text.clone()).collect()
}

/** The painted text of every item, joined, after laying out `markup` with its own `<style>`. */
fn painted(markup: &str) -> String {
    let parsed = parse(markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .flat_map(|item| item.lines.iter().map(|line| line.text.clone()))
        .collect()
}

/** `break-all` fills the line: the word after a space starts on the first line and breaks at
the remaining width instead of moving down whole. */
#[test]
fn break_all_fills_the_remaining_width() {
    let found = lines(
        "<div id=t style='font:20px/1 monospace;width:5ch;word-break:break-all'>XX XXXXXX</div>",
    );
    assert_eq!(found, ["XX XX", "XXXX"]);
}

/** `overflow-wrap: break-word` is an emergency break: the word moves to the next line first. */
#[test]
fn break_word_moves_the_word_down_first() {
    let found = lines(
        "<div id=t style='font:20px/1 monospace;width:5ch;overflow-wrap:break-word'>XX XXXXXX</div>",
    );
    assert_eq!(found, ["XX", "XXXXX", "X"]);
}

/** `line-break: anywhere` fills the line like `break-all`. */
#[test]
fn line_break_anywhere_fills_the_remaining_width() {
    let found = lines(
        "<div id=t style='font:20px/1 monospace;width:5ch;line-break:anywhere'>XX XXXXXX</div>",
    );
    assert_eq!(found, ["XX XX", "XXXX"]);
}

/** Under `pre-wrap`, `break-all` fills the line after a preserved space too. */
#[test]
fn break_all_fills_after_a_preserved_space() {
    let found = lines(
        "<div id=t style='font:20px/1 monospace;width:5ch;white-space:pre-wrap;word-break:break-all'> XXXXX</div>",
    );
    assert_eq!(found, [" XXXX", "X"]);
}

/** `keep-all` keeps ideographs together, so a CJK run wraps at its spaces like words do. */
#[test]
fn keep_all_keeps_ideographs_together() {
    let found = lines(
        "<div id=t style='font:20px/1 monospace;width:3.5em;word-break:keep-all'>文文 文文</div>",
    );
    assert_eq!(found, ["文文", "文文"]);
}

/** A keep-all run wider than a whole line relaxes to `normal`, as CSS Text 3 allows. */
#[test]
fn keep_all_relaxes_when_a_run_cannot_fit() {
    let found =
        lines("<div id=t style='font:20px/1 monospace;width:0;word-break:keep-all'>文文文</div>");
    assert_eq!(found, ["文", "文", "文"]);
}

/** Control characters in `content` stay in the generated text, which paints them visibly. */
#[test]
fn generated_control_characters_paint_visibly() {
    let text = painted(r#"<style>div::after{content:"\0001"}</style><div></div>"#);
    assert!(text.chars().any(|c| c != ' '), "{text:?}");
}

/** `text-transform: full-width` paints ASCII and the space as their fullwidth forms. */
#[test]
fn full_width_maps_ascii_to_fullwidth_forms() {
    let text = painted("<div style='text-transform:full-width'>a1 B</div>");
    assert_eq!(text, "\u{ff41}\u{ff11}\u{3000}\u{ff22}");
}

/** `text-transform: full-size-kana` paints small kana as full-size, after the case mapping. */
#[test]
fn full_size_kana_maps_small_kana_and_combines_with_case() {
    let text =
        painted("<div style='text-transform:uppercase full-size-kana'>\u{3041}\u{30f5}x</div>");
    assert_eq!(text, "\u{3042}\u{30ab}X");
}

/** A keyword given twice is invalid, so the inherited transform stays. */
#[test]
fn a_repeated_transform_keyword_is_invalid() {
    let text = painted("<div style='text-transform:full-width full-width'>a</div>");
    assert_eq!(text, "a");
}
