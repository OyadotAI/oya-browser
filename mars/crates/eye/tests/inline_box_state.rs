/** Blink's InlineBoxState for empty inline boxes: one with an inline-start or inline-end margin,
border or padding is not an empty item, so its line keeps the strut and the box paints around
its content area; an undecorated one leaves its line empty. */
use mars_eye::{compute_styles, parse, render};

/** The (x, y, width, height) of the element with `id` after laying out `markup` at 800px. */
fn geometry(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
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

/** A bordered empty span makes a line as tall as the block's line-height. */
#[test]
fn decorated_empty_inline_keeps_its_line() {
    let markup =
        "<div id=d style='font:20px/30px serif'><span style='border:1px solid'></span></div>";
    assert_eq!(geometry(markup, "d").3, 30.0);
}

/** An empty span with no inline-axis margin, border or padding leaves its line empty. */
#[test]
fn undecorated_empty_inline_leaves_its_line_empty() {
    let markup =
        "<div id=d style='font:20px/30px serif'><span style='outline:1px solid'></span></div>";
    assert_eq!(geometry(markup, "d").3, 0.0);
}

/** The empty box spans its padding and borders from its margin edge, as a filled one does. */
#[test]
fn decorated_empty_inline_paints_its_padding_box() {
    let markup = "<div style='font:20px/30px serif'><span id=s style='margin-left:10px;border:2px solid;padding:0 5px'></span></div>";
    let (x, _, width, height) = geometry(markup, "s");
    assert_eq!((x, width), (10.0, 14.0));
    assert!(height > 4.0, "{height}");
}
