/** Flex layout steps ported from Blink's `FlexLayoutAlgorithm`, each pinned to Chromium's geometry. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let sheet = "body{margin:0;font:15px sans-serif}".to_owned();
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The first item painted for the element with `id`. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** The left edge of the element with `id`. */
fn left(markup: &str, id: &str) -> f64 {
    let (document, display) = layout(markup);
    find(&document, &display, id).x
}

/** `flex-basis` takes any length unit, as Blink's `ConvertLengthOrAuto`: `flex: 0 0 4em` at a
15px font gives a 60px basis instead of being dropped for the item's width. */
#[test]
fn flex_basis_resolves_font_relative_lengths() {
    let (document, display) = layout(
        "<div style='display:flex;width:400px'>\
         <div id=a style='width:100px;flex:0 0 4em;height:10px'></div>\
         <div id=b style='width:100px;flex-basis:2em;flex-shrink:0;height:10px'></div></div>",
    );
    assert_eq!(find(&document, &display, "a").width, 60.0);
    assert_eq!(find(&document, &display, "b").width, 30.0);
}

/** `justify-content: start` is the container's own start edge (Blink's `kStart`), so a
`row-reverse` row packs at the left while `flex-start` packs at the right. */
#[test]
fn justify_start_ignores_row_reverse() {
    let row = |justify: &str| {
        format!(
            "<div style='display:flex;flex-direction:row-reverse;justify-content:{justify};\
             width:400px'><div id=a style='width:100px;height:10px'></div></div>"
        )
    };
    assert_eq!(left(&row("start"), "a"), 0.0);
    assert_eq!(left(&row("flex-start"), "a"), 300.0);
    assert_eq!(left(&row("end"), "a"), 300.0);
}

/** A right-to-left row's main-start is its right edge: the first item sits at the right, and
`left` resolves to the logical end, the physical left (Blink's `ResolvedJustifyContent`). */
#[test]
fn rtl_row_starts_at_the_right_edge() {
    let row = |justify: &str| {
        format!(
            "<div style='display:flex;direction:rtl;justify-content:{justify};width:400px'>\
             <div id=a style='width:100px;height:10px'></div>\
             <div id=b style='width:50px;height:10px'></div></div>"
        )
    };
    assert_eq!(left(&row("normal"), "a"), 300.0);
    assert_eq!(left(&row("normal"), "b"), 250.0);
    assert_eq!(left(&row("left"), "b"), 0.0);
    assert_eq!(left(&row("right"), "a"), 300.0);
}

/** In a horizontal-writing column `left` and `right` fall back to `start`, the top. */
#[test]
fn column_left_and_right_fall_back_to_start() {
    let (document, display) = layout(
        "<div style='display:flex;flex-direction:column;justify-content:right;height:200px'>\
         <div id=a style='height:50px'></div></div>",
    );
    assert_eq!(find(&document, &display, "a").y, 0.0);
}
