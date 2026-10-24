/** Line break opportunities after Blink's line breaker (UAX #14 with CSS overflow-wrap and
word-break), and LayoutUnit-rounded intrinsic text widths. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

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

/** A word wider than its line overflows whole by default instead of breaking mid-word. */
#[test]
fn a_long_word_overflows_by_default() {
    assert_eq!(
        lines("<div id=t style='width:40px'>extraordinarily</div>"),
        ["extraordinarily"]
    );
}

/** `overflow-wrap: break-word` lets an overflowing word break, inherited from an ancestor. */
#[test]
fn overflow_wrap_break_word_breaks_inside_a_word() {
    let found = lines(
        "<div style='overflow-wrap:break-word'><div id=t style='width:40px'>extraordinarily</div></div>",
    );
    assert!(found.len() > 1, "{found:?}");
}

/** `word-break: break-all` also breaks inside words, and `normal` on a child turns it off. */
#[test]
fn word_break_inherits_and_can_be_reset() {
    let broken = lines(
        "<div style='word-break:break-all'><div id=t style='width:40px'>extraordinarily</div></div>",
    );
    assert!(broken.len() > 1, "{broken:?}");
    let whole = lines(
        "<div style='word-break:break-all'><div id=t style='width:40px;word-break:normal'>extraordinarily</div></div>",
    );
    assert_eq!(whole, ["extraordinarily"]);
}

/** UAX #14 LB21: a hyphen between letters is a break opportunity. */
#[test]
fn a_hyphen_between_letters_is_a_break_opportunity() {
    let found = lines("<div id=t style='width:60px'>wellknown-wellknown</div>");
    assert_eq!(found, ["wellknown-", "wellknown"]);
}

/** LB25: a hyphen before a digit is not, so a negative number or range stays whole. */
#[test]
fn a_hyphen_before_a_digit_is_not_a_break() {
    assert_eq!(
        lines("<div id=t style='width:20px'>a-123456789</div>"),
        ["a-123456789"]
    );
}

/** A shrink-to-fit box is at least as wide as its text in LayoutUnits, so a `width: 100%` child
keeps its one word on one line (no fractional-pixel squeeze into two). */
#[test]
fn shrink_to_fit_never_squeezes_its_own_text() {
    let found = lines(
        "<div style='display:inline-block;padding:10px'><div id=t style='width:100%'>hello</div></div>",
    );
    assert_eq!(found, ["hello"]);
}
