/** Grid-lanes (css-grid-3 masonry) behaviours ported from Blink's GridLanesLayoutAlgorithm,
 * each pinned to the geometry the WPT references give the same markup. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let sheet = "body{margin:0;font:16px sans-serif}".to_owned();
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
/** Equal within layout rounding. */
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.5
}

/** Auto-placed items go to the shortest column, and a later item stacks under the one above it;
 * the container is as tall as its tallest column. */
#[test]
fn column_lanes_stack_items_into_the_shortest_column() {
    let (document, display) = layout(
        "<div id=grid style='display:grid-lanes;grid-template-columns:100px 100px;width:200px;\
         flow-tolerance:0'><div id=a style='height:50px'></div><div id=b style='height:20px'></div>\
         <div id=c style='height:10px'></div></div>",
    );
    let c = find(&document, &display, "c");
    assert!(close(c.x, 100.0) && close(c.y, 20.0));
    assert!(close(find(&document, &display, "grid").height, 50.0));
}

/** Within the flow tolerance a column counts as tied with the shortest, so the item keeps going
 * forward from the auto-placement cursor instead of jumping back. */
#[test]
fn flow_tolerance_ties_columns_and_the_cursor_moves_on() {
    let (document, display) = layout(
        "<div style='display:grid-lanes;grid-template-columns:100px 100px 100px;width:300px;\
         flow-tolerance:10px'><div style='height:12px'></div><div style='height:10px'></div>\
         <div style='height:15px'></div><div id=d style='height:5px'></div></div>",
    );
    // Running positions 12, 10, 15 after three items; the cursor wraps to lane 0 (12 <= 20).
    assert!(close(find(&document, &display, "d").x, 0.0));
}

/** An intrinsic `repeat(auto-fill, auto)` repeats as often as the size its items give the track
 * fits, rather than once. */
#[test]
fn intrinsic_auto_fill_repeats_by_item_size() {
    let (document, display) = layout(
        "<div style='display:grid-lanes;grid-template-columns:repeat(auto-fill,auto);width:350px;\
         flow-tolerance:0'><div style='width:100px;height:10px'></div>\
         <div style='width:100px;height:10px'></div><div id=c style='width:100px;height:10px'></div>\
         </div>",
    );
    // Three lanes fit; their `auto` tracks then stretch to share the 350px.
    let c = find(&document, &display, "c");
    assert!(close(c.x, 700.0 / 3.0) && close(c.y, 0.0));
}

/** With only `grid-template-rows`, the lanes are rows and items stack left to right. */
#[test]
fn row_lanes_stack_items_horizontally() {
    let (document, display) = layout(
        "<div style='display:grid-lanes;grid-template-rows:40px 40px;flow-tolerance:0'>\
         <div style='width:60px'></div><div style='width:30px'></div>\
         <div id=c style='width:10px'></div></div>",
    );
    let c = find(&document, &display, "c");
    assert!(close(c.x, 30.0) && close(c.y, 40.0) && close(c.height, 40.0));
}
