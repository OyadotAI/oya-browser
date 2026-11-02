/** Grid behaviours ported from Blink's GridLayoutAlgorithm and css-grid-2, each pinned to the
 * geometry Chromium gives the same markup (checked in Oya at a 1100 px viewport). */
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

/** Under a min-content constraint a flexible track keeps its base size, so a nested
 * `minmax(0, 1fr)` grid contributes nothing and does not widen its parent's column. */
#[test]
fn min_content_of_minmax_zero_fr_grid_is_zero() {
    let (document, display) = layout(
        "<div style='display:grid;width:420px'><div id=item style='display:grid;\
         grid-template-columns:minmax(0,1fr)'><div style='width:900px;height:10px'></div></div></div>",
    );
    assert!(close(find(&document, &display, "item").width, 420.0));
}

/** A shrink-to-fit grid's max-content width is its sized tracks: a fixed track keeps its length
 * even when its item is narrower, and an fr track takes its item's max-content. */
#[test]
fn max_content_width_sums_sized_tracks_and_gaps() {
    let (document, display) = layout(
        "<div id=grid style='float:left;display:grid;grid-template-columns:100px 1fr;column-gap:10px'>\
         <div style='width:50px;height:10px'></div><div style='width:200px;height:10px'></div></div>",
    );
    assert!(close(find(&document, &display, "grid").width, 310.0));
}

/** Items in different rows size the same column, so the widest item per column counts rather
 * than the widest row. */
#[test]
fn max_content_width_takes_each_column_widest_item() {
    let (document, display) = layout(
        "<div id=grid style='float:left;display:grid;grid-template-columns:auto auto'>\
         <div style='width:50px;height:10px'></div><div style='width:70px;height:10px'></div>\
         <div style='width:120px;height:10px'></div></div>",
    );
    assert!(close(find(&document, &display, "grid").width, 190.0));
}

/** A spanning item only adds what its spanned tracks lack after the single-span items. */
#[test]
fn spanning_item_adds_only_missing_width() {
    let (document, display) = layout(
        "<div id=grid style='float:left;display:grid;grid-template-columns:repeat(3,auto)'>\
         <div style='grid-column:2;width:40px;height:10px'></div>\
         <div style='grid-column:1/span 3;width:30px;height:10px'></div></div>",
    );
    assert!(close(find(&document, &display, "grid").width, 40.0));
}

/** `auto` tracks take the free space only while `justify-content` is `normal` or `stretch`;
 * `start` keeps them at their content size. */
#[test]
fn justify_content_start_keeps_auto_columns_content_sized() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:auto auto;justify-content:start;width:400px'>\
         <div id=a style='width:20px;height:10px'></div><div id=b style='width:30px;height:10px'></div></div>\
         <div style='display:grid;grid-template-columns:auto auto;width:400px'>\
         <div id=c style='height:10px'></div><div id=d style='height:10px'></div></div>",
    );
    assert!(close(find(&document, &display, "b").x, 20.0));
    assert!(close(find(&document, &display, "d").x, 200.0));
}

/** `align-content` places the rows of a definite-height grid; `center` keeps the gap. */
#[test]
fn align_content_center_places_rows() {
    let (document, display) = layout(
        "<div style='display:grid;align-content:center;height:300px;row-gap:10px'>\
         <div id=a style='height:50px'></div><div id=b style='height:50px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").y, 95.0));
    assert!(close(find(&document, &display, "b").y, 155.0));
}

/** Items fixed in both axes are placed before auto-placed ones, which flow around them. */
#[test]
fn definite_items_are_placed_before_auto_items() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:repeat(3,100px)'>\
         <div id=a style='height:20px'></div><div id=b style='height:20px'></div>\
         <div id=c style='grid-row:1;grid-column:1;height:20px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").x, 100.0));
    assert!(close(find(&document, &display, "b").x, 200.0));
    assert!(close(find(&document, &display, "c").x, 0.0));
}

/** An item locked to a row takes that row's first free column before the cursor runs. */
#[test]
fn row_locked_item_is_placed_before_the_cursor_runs() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:repeat(3,100px)'>\
         <div id=a style='height:20px'></div><div id=b style='grid-row:1;height:20px'></div>\
         <div id=c style='height:20px'></div></div>",
    );
    assert!(close(find(&document, &display, "a").x, 100.0));
    assert!(close(find(&document, &display, "b").x, 0.0));
    assert!(close(find(&document, &display, "c").x, 200.0));
}

/** `grid-auto-flow: dense` back-fills a hole an earlier spanning item left. */
#[test]
fn dense_flow_back_fills_holes() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:repeat(3,100px);grid-auto-flow:dense'>\
         <div id=a style='grid-column:span 2;height:20px'></div>\
         <div id=b style='grid-column:span 2;height:20px'></div><div id=c style='height:20px'></div></div>",
    );
    let (a, c) = (
        find(&document, &display, "a"),
        find(&document, &display, "c"),
    );
    assert!(close(c.y, a.y));
    assert!(close(c.x, 200.0));
}

/** `grid-auto-flow: column` fills each column's rows before the next, and implicit columns take
 * `grid-auto-columns`. */
#[test]
fn column_flow_fills_columns_with_auto_column_sizes() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-rows:20px 20px;grid-auto-flow:column;grid-auto-columns:50px'>\
         <div id=a></div><div id=b></div><div id=c></div></div>",
    );
    let (a, b, c) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
        find(&document, &display, "c"),
    );
    assert!(close(b.y, a.y + 20.0) && close(b.x, a.x));
    assert!(close(c.x, 50.0) && close(c.y, a.y));
    assert!(close(c.width, 50.0));
}

/** Implicit rows cycle through the `grid-auto-rows` list. */
#[test]
fn implicit_rows_cycle_auto_row_sizes() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:100px;grid-auto-rows:30px 10px'>\
         <div id=a></div><div id=b></div><div id=c></div></div>",
    );
    assert!(close(find(&document, &display, "a").height, 30.0));
    assert!(close(find(&document, &display, "b").height, 10.0));
    assert!(close(find(&document, &display, "c").height, 30.0));
}

/** A box of `width` px that never wraps, as an item's content. */
fn block(width: u32) -> String {
    format!("<span style='display:inline-block;width:{width}px'></span>")
}

/** `fit-content(L)` sizes to max-content up to `L`, and unlike `auto` does not stretch. */
#[test]
fn fit_content_track_grows_to_content_and_does_not_stretch() {
    let (document, display) = layout(&format!(
        "<div style='display:grid;grid-template-columns:fit-content(300px) auto;width:600px'>\
         <div id=a>{}</div><div id=b style='height:20px'></div></div>",
        block(150)
    ));
    assert!(close(find(&document, &display, "a").width, 150.0));
    assert!(close(find(&document, &display, "b").width, 450.0));
}

/** `max-content` tracks keep their content size; only `auto` tracks take leftover space. */
#[test]
fn max_content_tracks_do_not_stretch() {
    let (document, display) = layout(&format!(
        "<div style='display:grid;grid-template-columns:max-content max-content;width:600px'>\
         <div id=a>{}</div><div id=b>{}</div></div>",
        block(150),
        block(50)
    ));
    assert!(close(find(&document, &display, "a").width, 150.0));
    assert!(close(find(&document, &display, "b").x, 150.0));
    assert!(close(find(&document, &display, "b").width, 50.0));
}

/** A `min-content` track stops at its items' min-content and leaves the rest to `fr`. */
#[test]
fn min_content_track_stops_at_min_content() {
    let (document, display) = layout(&format!(
        "<div style='display:grid;grid-template-columns:min-content 1fr;width:600px'>\
         <div id=a>{} {}</div><div id=b style='height:20px'></div></div>",
        block(80),
        block(80)
    ));
    assert!(close(find(&document, &display, "a").width, 80.0));
    assert!(close(find(&document, &display, "b").width, 520.0));
}

/** `auto` rows of an auto-height grid stretch to its `min-height`. */
#[test]
fn auto_rows_stretch_to_min_height() {
    let (document, display) = layout(
        "<div style='display:grid;min-height:300px'><div id=a>x</div><div id=b>y</div></div>",
    );
    assert!(close(find(&document, &display, "a").height, 150.0));
    assert!(close(find(&document, &display, "b").y, 150.0));
}
