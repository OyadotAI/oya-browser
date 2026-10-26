/** Containing blocks formed by split inline boxes (Blink's AddInlineContainingBlockInfo) and font
sizes past the old 72px cap, which shrank every 100px Ahem square in the positioning reftests. */
use mars_eye::{compute_styles, parse, render};

/** Lays out `markup` at 800px and returns the box of the element with id `id`. */
fn rect(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width, item.height)
}

/** The union of every box painted for `id`: a split inline paints one fragment per line. */
fn union(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let items = display.items.iter().filter(|i| i.local_id == local);
    let (mut left, mut top) = (f64::INFINITY, f64::INFINITY);
    let (mut right, mut bottom) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for item in items {
        left = left.min(item.x);
        top = top.min(item.y);
        right = right.max(item.x + item.width);
        bottom = bottom.max(item.y + item.height);
    }
    (left, top, right - left, bottom - top)
}

/** A 100px font with `line-height: 1` makes a 100px line, as Blink computes it. */
#[test]
fn font_sizes_above_seventy_two_pixels_are_kept() {
    let r = rect(
        "<body style='margin:0'><div id=a style='font:100px/1 serif'>X</div>",
        "a",
    );
    assert_eq!(r.3, 100.0);
}

/** An inline broken across lines contains an `inset: 0` box from the start of its first
fragment to the end of its last, not the union of every line. */
#[test]
fn split_inline_containing_block_runs_first_start_to_last_end() {
    let markup = "<body style='margin:0'><div style='width:100px;font:20px/1 monospace'>XX \
        <span id=s style='position:relative'>XXXX<br>XXXXXXXX<br>X\
        <div id=a style='position:absolute;inset:0'></div></span></div>";
    let span = union(markup, "s");
    let abs = rect(markup, "a");
    assert!(
        abs.0 > span.0 + 1.0,
        "starts after the leading text: {abs:?} in {span:?}"
    );
    assert!(
        abs.2 < span.2 / 2.0,
        "ends at the short last line: {abs:?} in {span:?}"
    );
    assert_eq!((abs.1, abs.3), (span.1, span.3));
}

/** An inline on one line keeps its own box as the containing block. */
#[test]
fn single_line_inline_containing_block_is_its_box() {
    let markup = "<body style='margin:0'><div style='font:20px/1 monospace'>XX \
        <span id=s style='position:relative'>XXXX\
        <div id=a style='position:absolute;inset:0'></div></span></div>";
    assert_eq!(rect(markup, "a"), rect(markup, "s"));
}
