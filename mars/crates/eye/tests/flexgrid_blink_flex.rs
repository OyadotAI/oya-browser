/** Flexbox behaviours ported from Blink's FlexLayoutAlgorithm and css-flexbox-1, each pinned to
 * the geometry Chromium gives the same markup (checked in Oya at a 1100 px viewport). */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 1100 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let sheet = "body{margin:0;font:15px sans-serif}".to_owned();
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let display = render(&document, 1100.0).unwrap();
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
/** Equal within layout rounding. */
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.5
}

/** `align-content: normal` stretches a multi-line container's lines over its free cross space,
 * and a stretched item fills its taller line. */
#[test]
fn normal_align_content_stretches_wrapped_lines() {
    let (document, display) = layout(
        "<div style='display:flex;flex-wrap:wrap;width:200px;height:200px'>\
         <div id=a style='width:150px;flex:none;min-height:50px'></div>\
         <div id=b style='width:150px;flex:none;height:50px'></div></div>",
    );
    let (a, b) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
    );
    assert!(close(a.height, 100.0));
    assert!(close(b.y, 100.0));
}

/** `align-content: center` places the lines as one block in the middle of the free space. */
#[test]
fn align_content_center_places_lines_in_the_middle() {
    let (document, display) = layout(
        "<div style='display:flex;flex-wrap:wrap;align-content:center;width:200px;height:200px'>\
         <div id=a style='width:150px;flex:none;height:50px'></div>\
         <div id=b style='width:150px;flex:none;height:50px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").y, 50.0));
    assert!(close(find(&document, &display, "b").y, 100.0));
}

/** `justify-content: center` is unsafe by default: an overflowing item spills over both edges. */
#[test]
fn centered_overflowing_item_spills_both_ways() {
    let (document, display) = layout(
        "<div style='display:flex;justify-content:center;width:200px;margin-left:100px'>\
         <div id=a style='width:300px;height:10px;flex:none'></div></div>",
    );
    assert!(close(find(&document, &display, "a").x, 50.0));
}

/** `space-around` falls back to safe `center`, which never pushes overflow past the start. */
#[test]
fn space_around_overflow_falls_back_to_safe_center() {
    let (document, display) = layout(
        "<div style='display:flex;justify-content:space-around;width:200px;margin-left:100px'>\
         <div id=a style='width:300px;height:10px;flex:none'></div></div>",
    );
    assert!(close(find(&document, &display, "a").x, 100.0));
}

/** `space-between` with one item falls back to `flex-start`, the right edge of a `row-reverse`. */
#[test]
fn space_between_single_item_packs_at_reversed_start() {
    let (document, display) = layout(
        "<div style='display:flex;flex-direction:row-reverse;justify-content:space-between;width:200px'>\
         <div id=a style='width:50px;height:10px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").x, 150.0));
}

/** An `aspect-ratio` item with a definite height takes its flex base size, and its automatic
 * minimum, from that height through the ratio. */
#[test]
fn aspect_ratio_item_bases_its_width_on_its_height() {
    let (document, display) = layout(
        "<div style='display:flex;width:300px'><div id=a style='aspect-ratio:2;height:50px'></div>\
         <div id=b style='flex:1;height:10px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").width, 100.0));
    assert!(close(find(&document, &display, "b").x, 100.0));
}

/** In a single-line container with a definite height the stretched line is the definite cross
 * size an `aspect-ratio` item sizes its width from. */
#[test]
fn aspect_ratio_item_sizes_from_the_stretched_line() {
    let (document, display) = layout(
        "<div style='display:flex;width:300px;height:60px'><div id=a style='aspect-ratio:1'></div>\
         <div style='flex:1;height:10px'></div></div>",
    );
    let a = find(&document, &display, "a");
    assert!(close(a.width, 60.0) && close(a.height, 60.0));
}

/** Stretching wins over `aspect-ratio` for the cross size of an item that also flexes. */
#[test]
fn stretch_overrides_aspect_ratio_for_the_cross_size() {
    let (document, display) = layout(
        "<div style='display:flex;width:300px;height:60px'><div id=a style='aspect-ratio:1;flex:1'></div>\
         <div style='width:100px'></div></div>",
    );
    let a = find(&document, &display, "a");
    assert!(close(a.width, 200.0) && close(a.height, 60.0));
}

/** A column item's automatic minimum is its content size, not its own `height`, so an empty
 * `height: 20px` item shrinks away when a sibling cannot. */
#[test]
fn column_item_automatic_minimum_ignores_its_own_height() {
    let (document, display) = layout(
        "<div style='display:flex;flex-direction:column;height:100px;width:300px'>\
         <div id=a style='flex:1'><div style='height:300px'></div></div>\
         <div id=b style='height:20px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").height, 300.0));
    assert!(close(find(&document, &display, "b").height, 0.0));
}

/** A wrapping column with a definite height starts a new line beside the first, and
 * `align-content: normal` shares the width between the lines. */
#[test]
fn wrapping_column_places_lines_side_by_side() {
    let (document, display) = layout(
        "<div style='display:flex;flex-direction:column;flex-wrap:wrap;height:100px;width:300px'>\
         <div id=a style='height:60px;width:50px'></div><div id=b style='height:60px;width:50px'></div></div>",
    );
    let (a, b) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
    );
    assert!(close(b.y, a.y));
    assert!(close(b.x, 150.0));
}

/** A packed wrapping column's line is as wide as its widest item. */
#[test]
fn packed_column_line_is_its_widest_item() {
    let (document, display) = layout(
        "<div style='display:flex;flex-direction:column;flex-wrap:wrap;height:100px;width:300px;\
         align-content:flex-start;align-items:flex-start'><div id=a style='height:60px;width:50px'></div>\
         <div id=b style='height:60px;width:80px'></div><div id=c style='height:30px;width:20px'></div></div>",
    );
    let (b, c) = (
        find(&document, &display, "b"),
        find(&document, &display, "c"),
    );
    assert!(close(b.x, 50.0));
    assert!(close(c.x, 50.0) && close(c.y, b.y + 60.0));
}
