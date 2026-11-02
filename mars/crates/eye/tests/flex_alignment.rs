/** Flex alignment: absolutely positioned children's static positions, `safe` overflow
alignment, percentage gaps and LayoutUnit line shares, after Blink's `FlexLayoutAlgorithm`. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let sheet = "body{margin:0}".to_owned();
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The (x, y, width, height) of the first item painted for the element with id `id`. */
fn rect(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let (document, display) = layout(markup);
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    let item: &DisplayItem = display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap();
    (item.x, item.y, item.width, item.height)
}

/** An absolutely positioned child with auto insets sits where it would as the sole flex item:
centered by `justify-content` and `align-items`. */
#[test]
fn abspos_child_aligns_as_sole_item() {
    let markup = "<div style='position:relative;display:flex;justify-content:center;\
         align-items:center;width:200px;height:200px'>\
         <div id=a style='position:absolute;width:100px;height:100px'></div></div>";
    assert_eq!(rect(markup, "a"), (50.0, 50.0, 100.0, 100.0));
}

/** `align-self: safe end` keeps an item that overflows its line at the start. */
#[test]
fn safe_self_alignment_keeps_overflow_at_start() {
    let markup = |align: &str| {
        format!(
            "<div style='display:flex;height:50px;width:200px'>\
             <div id=a style='align-self:{align};width:20px;height:80px'></div></div>"
        )
    };
    assert_eq!(rect(&markup("end"), "a").1, -30.0);
    assert_eq!(rect(&markup("safe end"), "a").1, 0.0);
}

/** `justify-content: safe flex-end` packs overflowing content at the start. */
#[test]
fn safe_content_alignment_keeps_overflow_at_start() {
    let markup = |justify: &str| {
        format!(
            "<div style='display:flex;justify-content:{justify};width:50px;height:10px'>\
             <div id=a style='flex:none;width:80px'></div></div>"
        )
    };
    assert_eq!(rect(&markup("flex-end"), "a").0, -30.0);
    assert_eq!(rect(&markup("safe flex-end"), "a").0, 0.0);
}

/** A percentage column gap resolves against the container's inner width. */
#[test]
fn percent_column_gap_resolves_against_width() {
    let markup = "<div style='display:flex;column-gap:10%;width:200px'>\
         <div style='width:20px;height:5px'></div><div id=b style='width:20px;height:5px'></div></div>";
    assert_eq!(rect(markup, "b").0, 40.0);
}

/** Stretched flex lines share free space in truncated 1/64 px LayoutUnits, as Blink does. */
#[test]
fn line_shares_truncate_to_layout_units() {
    let markup = "<div style='display:flex;flex-wrap:wrap;width:10px;height:100px'>\
         <div style='width:10px'></div><div id=b style='width:10px'></div>\
         <div style='width:10px'></div></div>";
    assert_eq!(rect(markup, "b").1, 2133.0 / 64.0);
}
