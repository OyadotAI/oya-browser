/** CSS Text white space processing after Blink's InlineItemsBuilder and line breaker:
preserved spaces under `pre-wrap` hang, `break-spaces` wraps them, tabs stop at `tab-size`
multiples, and control characters paint visibly. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** The painted runs (text, x, width) of `#t` after laying out `markup` at 800px. */
fn runs(markup: &str) -> Vec<(String, f64, f64)> {
    let markup = format!("<body style='margin:0;font:20px monospace'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let item: &DisplayItem = display.items.iter().find(|i| i.local_id == local).unwrap();
    item.lines
        .iter()
        .map(|line| (line.text.clone(), line.x, line.width))
        .collect()
}

/** The texts of the runs. */
fn texts(markup: &str) -> Vec<String> {
    runs(markup).into_iter().map(|run| run.0).collect()
}

/** `pre-wrap` keeps every space inside a line instead of collapsing runs as `pre-line` does. */
#[test]
fn pre_wrap_keeps_runs_of_spaces() {
    assert_eq!(
        texts("<div id=t style='white-space:pre-wrap'>a   b</div>"),
        ["a   b"]
    );
}

/** Spaces before a soft wrap hang: they stay on the line and never push the next word down. */
#[test]
fn pre_wrap_spaces_hang_at_a_soft_wrap() {
    let found = runs("<div id=t style='white-space:pre-wrap;width:4ch'>XX    XX</div>");
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].0, "XX    ");
    assert_eq!(found[1].0, "XX");
}

/** Hanging spaces do not count for alignment, so a centered line centers its words; the spaces
stay in the run past the end edge, where an inline box's background still paints them. */
#[test]
fn hanging_spaces_do_not_shift_centered_lines() {
    let found =
        runs("<div id=t style='white-space:pre-wrap;width:6ch;text-align:center'>XX     XX</div>");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!((found[0].1 - found[1].1).abs() < 0.5, "{found:?}");
    assert_eq!(found[0].0, "XX     ");
}

/** `break-spaces` spaces do not hang: one that does not fit moves to the next line. */
#[test]
fn break_spaces_wraps_spaces() {
    let found = texts("<div id=t style='white-space:break-spaces;width:4ch'> XX  XX</div>");
    assert_eq!(found, [" XX ", " XX"]);
}

/** A line only breaks after a space under `break-spaces`, so a word keeps its first space. */
#[test]
fn break_spaces_breaks_only_after_a_space() {
    let found = texts("<div id=t style='white-space:break-spaces;width:4ch'>X XX X</div>");
    assert_eq!(found, ["X ", "XX X"]);
}

/** A tab advances to the next multiple of `tab-size` spaces from the line start. */
#[test]
fn tabs_stop_at_tab_size_multiples() {
    let found = runs("<div id=t style='white-space:pre;tab-size:4'>X\tY</div>");
    let space = runs("<div id=t style='white-space:pre'>X</div>")[0].2;
    let y = found.last().unwrap();
    assert_eq!(y.0, "Y");
    assert!((y.1 - 4.0 * space).abs() < 0.5, "{found:?}");
}

/** `tab-size` also takes a length. */
#[test]
fn tab_size_takes_a_length() {
    let found = runs("<div id=t style='white-space:pre;tab-size:100px'>\tY</div>");
    let y = found.last().unwrap();
    assert!((y.1 - 100.0).abs() < 0.5, "{found:?}");
}

/** Tabs hang under `pre-wrap` like spaces. */
#[test]
fn pre_wrap_tabs_hang() {
    let found = texts("<div id=t style='white-space:pre-wrap;width:2ch'>XX\t\tXX</div>");
    assert_eq!(found.first().map(String::as_str), Some("XX"));
    assert_eq!(found.last().map(String::as_str), Some("XX"));
    assert_eq!(found.iter().filter(|t| t.as_str() == "XX").count(), 2);
}

/** A control character that is not white space paints as a visible glyph. */
#[test]
fn control_characters_paint_visibly() {
    let found = runs("<div id=t>a\u{1}b</div>");
    assert_eq!(found[0].0, "a\u{fffd}b");
}

/** The laid-out item of `#t` in `markup`, as `runs` lays it out. */
fn item(markup: &str) -> DisplayItem {
    let markup = format!("<body style='margin:0;font:20px monospace'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    display
        .items
        .into_iter()
        .find(|i| i.local_id == local)
        .unwrap()
}

/** Tab stops count from the block's content edge, so a float beside the line does not move
them (Blink's TabAlignmentWithFloats). */
#[test]
fn tab_stops_ignore_floats() {
    let space = runs("<div id=t style='white-space:pre'>X</div>")[0].2;
    let found = runs(
        "<div style='white-space:pre;tab-size:4'>\
         <span style='float:left;width:40px;height:10px'></span><span id=t>a\tb</span></div>",
    );
    let b = found.last().unwrap();
    assert_eq!(b.0, "b");
    assert!((b.1 - 8.0 * space).abs() < 0.5, "{found:?}");
}

/** `tab-size` counts spaces of the block container's font, so a smaller font inside the block
keeps the block's stops (Blink's InlineNode::FontForTab). */
#[test]
fn tab_size_counts_the_block_fonts_spaces() {
    let space = runs("<div id=t style='white-space:pre'>X</div>")[0].2;
    let found = runs(
        "<div style='white-space:pre;tab-size:4'><span id=t style='font-size:10px'>\tb</span></div>",
    );
    let b = found.last().unwrap();
    assert!((b.1 - 4.0 * space).abs() < 0.5, "{found:?}");
}

/** At the paragraph's end trailing preserved spaces hang only where they overflow, so spaces
that fit count for centering (CSS Text's conditional hang). */
#[test]
fn trailing_spaces_that_fit_count_for_alignment() {
    let found =
        runs("<div id=t style='white-space:pre-wrap;width:10ch;text-align:center'>XX  </div>");
    let space = found[0].2 / 4.0;
    assert!((found[0].1 - 3.0 * space).abs() < 0.5, "{found:?}");
}

/** Right to left, trailing preserved spaces reorder to the line's visual end, so they never
push the text off its start edge. */
#[test]
fn rtl_trailing_spaces_keep_the_text_at_the_start_edge() {
    let found = runs("<div id=t dir=rtl style='white-space:pre-wrap;width:200px'>ab </div>");
    let space = found[0].2 / 3.0;
    assert!((found[0].1 + 2.0 * space - 200.0).abs() < 0.5, "{found:?}");
}

/** White-space-only text the parser drops comes back as the box's content under preserved
white space, so `<span>   </span>` keeps its spaces (and paints their background). */
#[test]
fn preserved_white_space_only_text_is_kept() {
    let found = texts("<div style='white-space:pre'>X<span id=t>   </span>X</div>");
    assert_eq!(found, ["   "]);
}

/** A block's own leading newline under `pre` is a forced break, so its first child starts on
the second line. */
#[test]
fn a_preserved_leading_newline_breaks_the_line() {
    let span = item("<div style='white-space:pre'>\n<span id=t>X</span></div>");
    assert!(span.lines[0].y > 20.0, "{:?}", span.lines);
}

/** A no-break space is text, not white space the parser drops: `<p>&nbsp;</p>` has a line. */
#[test]
fn a_lone_no_break_space_is_text() {
    assert_eq!(texts("<p id=t>&nbsp;</p>"), ["\u{a0}"]);
}

/** Under `pre-wrap` an ideographic space hangs at the line's end with the spaces instead of
wrapping (Blink's IsBreakableSpaceOrOtherSeparator). */
#[test]
fn pre_wrap_ideographic_spaces_hang() {
    let found =
        texts("<div id=t style='white-space:pre-wrap;width:3ch'>XX\u{3000}\u{3000}XX</div>");
    assert_eq!(found, ["XX\u{3000}\u{3000}", "XX"]);
}

/** Under `break-spaces` a line may break after an ideographic space, so a long run of them
wraps instead of overflowing on one line. */
#[test]
fn break_spaces_wrap_after_ideographic_spaces() {
    let spaces = "\u{3000}".repeat(10);
    let found = runs(&format!(
        "<div id=t style='white-space:break-spaces;width:100px'>XXXX{spaces}XXXX</div>"
    ));
    assert!(found.len() > 1, "{found:?}");
    assert!(found.iter().all(|run| run.2 <= 100.5), "{found:?}");
}

/** A carriage return lays out as a space under preserved white space, as where white space
collapses. */
#[test]
fn preserved_carriage_returns_are_spaces() {
    assert_eq!(
        texts("<div style='white-space:pre'><span id=t>a&#x0D;b</span></div>"),
        ["a b"]
    );
}

/** An inline box with an inline-start margin holding only floats still opens a line (Blink's
IsInlineBoxStartEmpty), so its block is as tall as the line, and the floats keep their own
boxes. */
#[test]
fn a_box_of_only_floats_still_opens_its_line() {
    let block = item(
        "<div id=t><span style='margin-left:40px'>\
         <span style='float:left;width:50%'>float</span></span></div>",
    );
    assert!(block.height > 20.0, "{block:?}");
    let float = item(
        "<div><span style='margin-left:40px'>\
         <span id=t style='float:left;width:50%'>float</span></span></div>",
    );
    assert!((float.width - 400.0).abs() < 0.5, "{float:?}");
}

/** `ch` measures the styled document's own loaded fonts, never the ones another document left
installed on the thread, and a style that measured one is flagged so the page styles again once
its web fonts load. Pre-wrap tests sized in `ch` with Ahem passed or failed by which test ran
before them. */
#[cfg(target_os = "macos")]
#[test]
fn ch_measures_the_documents_own_fonts() {
    let bytes = std::fs::read("/System/Library/Fonts/Supplemental/Times New Roman.ttf").unwrap();
    let css = "@font-face { font-family: Site; src: url(https://fonts.example/site.ttf) } \
               div { font: 20px Site, monospace; width: 10ch }";
    let width = |document: &mars_eye::Document| {
        let display = render(document, 800.0).unwrap();
        let div = document.nodes.iter().find(|n| n.tag == "div").unwrap();
        let item = display.items.iter().find(|i| i.local_id == div.local_id);
        item.unwrap().width
    };
    let source = parse("<div>x</div>").unwrap();
    let plain = compute_styles(&source, &[css.into()]).unwrap();
    assert!(plain.formatting.font_metrics);
    let mut loaded = plain.clone();
    loaded.formatting.fonts.push(mars_eye::LoadedFont {
        face: 0,
        key: mars_eye::content_key(&bytes),
        data: base64(&bytes),
    });
    let fallback = width(&plain);
    let web = width(&compute_styles(&loaded, &[css.into()]).unwrap());
    assert!((fallback - web).abs() > 1.0, "{fallback} {web}");
    let again = width(&compute_styles(&source, &[css.into()]).unwrap());
    assert!((again - fallback).abs() < 0.01, "{again} {fallback}");
}

/** Standard base64 with padding, as the app encodes font bytes. */
#[cfg(target_os = "macos")]
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ALPHABET[((value >> (18 - 6 * i)) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}
