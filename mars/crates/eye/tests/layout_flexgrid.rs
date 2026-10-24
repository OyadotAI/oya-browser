/** Flex and grid layout ported from Blink: the css-grid-2 track sizing algorithm, automatic
minimum sizes, line placement, reversed flex axes, baseline alignment and their shorthands. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
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
/** Geometry of several ids at once. */
fn boxes<const N: usize>(markup: &str, ids: [&str; N]) -> [DisplayItem; N] {
    let (document, display) = layout(markup);
    ids.map(|id| find(&document, &display, id).clone())
}

/** css-grid-2 6.6: a scroll container's automatic minimum is zero, so an `overflow: hidden`
item's wide content does not widen its `1fr` (minmax(auto, 1fr)) column. */
#[test]
fn grid_scroll_container_item_does_not_widen_flexible_column() {
    let [a, b] = boxes(
        "<div style='display:grid;grid-template-columns:repeat(2,1fr);width:400px'>\
         <div id=a style='overflow:hidden'><div style='width:600px'>x</div></div>\
         <div id=b>b</div></div>",
        ["a", "b"],
    );
    assert!(close(a.width, 200.0), "{}", a.width);
    assert!(close(b.width, 200.0), "{}", b.width);
}

/** An item's `min-width: 0` is its minimum contribution, so a single `1fr` column stays the
grid's width (nodejs.org's page grid had widened to its code block). */
#[test]
fn grid_min_width_zero_item_keeps_flexible_column_at_grid_width() {
    let [a] = boxes(
        "<div style='display:grid;grid-template-columns:1fr;width:400px'>\
         <div id=a style='min-width:0'><div style='width:800px'>x</div></div></div>",
        ["a"],
    );
    assert!(close(a.width, 400.0), "{}", a.width);
}

/** Without that, `1fr` is `minmax(auto, 1fr)`: the content-based minimum of an item becomes
the flexible track's base size and the fr share is found over the rest (12.7.1). */
#[test]
fn grid_flexible_column_grows_to_content_based_minimum() {
    let [a, b] = boxes(
        "<div style='display:grid;grid-template-columns:1fr 1fr;width:400px'>\
         <div id=a><div style='width:300px'>x</div></div><div id=b>b</div></div>",
        ["a", "b"],
    );
    assert!(close(a.width, 300.0), "{}", a.width);
    assert!(close(b.width, 100.0), "{}", b.width);
}

/** A spanning item's extra space goes to the intrinsic tracks it spans (12.5.1 step 3). */
#[test]
fn grid_spanning_item_grows_its_auto_tracks() {
    let [a, b] = boxes(
        "<div style='display:grid;grid-template-columns:auto 100px;width:300px'>\
         <div id=a style='grid-column:1 / span 2'><div style='width:300px'>x</div></div>\
         <div id=b>b</div></div>",
        ["a", "b"],
    );
    assert!(close(a.width, 300.0), "{}", a.width);
    assert!(close(b.width, 200.0), "{}", b.width);
}

/** With an indefinite height, flexible rows take the largest fr any item needs (12.7). */
#[test]
fn grid_flexible_rows_in_indefinite_height_share_one_fraction() {
    let [grid] = boxes(
        "<div id=g style='display:grid;grid-template-rows:1fr 2fr'>\
         <div style='height:30px'></div><div style='height:30px'></div></div>",
        ["g"],
    );
    assert!(close(grid.height, 90.0), "{}", grid.height);
}

/** Negative lines count from the end of the explicit grid; with no template `-1` is line 1, so
the item is not pushed into a second implicit column, and auto margins center it. */
#[test]
fn grid_negative_line_without_template_and_auto_margins_center_item() {
    let [grid, a] = boxes(
        "<div id=g style='display:grid;width:400px'>\
         <div id=a style='grid-area:1/-1;justify-self:start;width:200px;margin:0 auto'>a</div>\
         </div>",
        ["g", "a"],
    );
    assert!(close(a.x - grid.x, 100.0), "{} {}", grid.x, a.x);
}

/** A track that is a math function mixing percentages in `min()` resolves against the grid's
width at layout, as Notion's hero grid tracks do. */
#[test]
fn grid_math_function_track_resolves_against_grid_width() {
    let [a, b] = boxes(
        "<div style='display:grid;width:200px;\
         grid-template-columns:calc((min(300px, calc(100% - 40px)) + 20px) / 2) 1fr'>\
         <div id=a>a</div><div id=b>b</div></div>",
        ["a", "b"],
    );
    assert!(close(a.width, 90.0), "{}", a.width);
    assert!(close(b.width, 110.0), "{}", b.width);
}

/** A grid item's percentage height resolves against its grid area once tracks are sized. */
#[test]
fn grid_item_percentage_height_fills_its_area() {
    let [a] = boxes(
        "<div style='display:grid;grid-template-rows:120px'>\
         <figure id=a style='margin:0;height:100%;align-self:start'></figure></div>",
        ["a"],
    );
    assert!(close(a.height, 120.0), "{}", a.height);
}

/** `place-items` sets both alignments of every item. */
#[test]
fn grid_place_items_centers_items() {
    let [grid, a] = boxes(
        "<div id=g style='display:grid;grid-template-columns:200px;grid-template-rows:100px;\
         place-items:center'><div id=a style='width:50px;height:20px'></div></div>",
        ["g", "a"],
    );
    assert!(close(a.x - grid.x, 75.0), "{}", a.x - grid.x);
    assert!(close(a.y - grid.y, 40.0), "{}", a.y - grid.y);
}

/** Flexbox 4.5: a scroll container's automatic minimum size is zero, so it shrinks below its
content's min-content width. */
#[test]
fn flex_scroll_container_item_shrinks_below_content() {
    let [a] = boxes(
        "<div style='display:flex;width:200px'>\
         <div id=a style='overflow:hidden'><div style='width:300px'>x</div></div>\
         <div style='width:100px;flex-shrink:0'>b</div></div>",
        ["a"],
    );
    assert!(close(a.width, 100.0), "{}", a.width);
}

/** `row-reverse` runs from the main-end edge: the first item is rightmost and packs right. */
#[test]
fn flex_row_reverse_places_first_item_at_the_right() {
    let [box_, a, b] = boxes(
        "<div id=f style='display:flex;flex-direction:row-reverse;width:300px'>\
         <div id=a style='width:50px'>a</div><div id=b style='width:50px'>b</div></div>",
        ["f", "a", "b"],
    );
    assert!(close(a.x - box_.x, 250.0), "{}", a.x - box_.x);
    assert!(close(b.x - box_.x, 200.0), "{}", b.x - box_.x);
}

/** `column-reverse` stacks the first item last. */
#[test]
fn flex_column_reverse_stacks_first_item_last() {
    let [a, b] = boxes(
        "<div style='display:flex;flex-direction:column-reverse'>\
         <div id=a style='height:10px'></div><div id=b style='height:20px'></div></div>",
        ["a", "b"],
    );
    assert!(close(a.y - b.y, 20.0), "{} {}", a.y, b.y);
}

/** `wrap-reverse` stacks lines from the cross-end edge: the first line is the lowest. */
#[test]
fn flex_wrap_reverse_puts_first_line_last() {
    let [a, c] = boxes(
        "<div style='display:flex;flex-wrap:wrap-reverse;width:100px'>\
         <div id=a style='width:60px;height:10px'></div><div style='width:60px;height:10px'></div>\
         <div id=c style='width:60px;height:10px'></div></div>",
        ["a", "c"],
    );
    assert!(close(a.y - c.y, 20.0), "{} {}", a.y, c.y);
}

/** `align-items: baseline` lines up first baselines: an item whose text sits 20px lower inside
it moves up by 20px against its neighbour (Flexbox 8.3). */
#[test]
fn flex_baseline_alignment_lines_up_first_baselines() {
    let [a, b] = boxes(
        "<div style='display:flex;align-items:baseline'>\
         <div id=a style='padding-top:20px'>a</div><div id=b>b</div></div>",
        ["a", "b"],
    );
    assert!(close(b.y - a.y, 20.0), "{} {}", a.y, b.y);
}

/** `flex-flow` sets the direction and wrapping it names. */
#[test]
fn flex_flow_shorthand_sets_direction() {
    let [a, b] = boxes(
        "<div style='display:flex;flex-flow:column nowrap'>\
         <div id=a style='height:10px'></div><div id=b style='height:10px'></div></div>",
        ["a", "b"],
    );
    assert!(close(b.y - a.y, 10.0), "{} {}", a.y, b.y);
    assert!(close(a.x, b.x));
}

/** `flex-basis: content` is a content-sized basis, so `flex: 1 1 content` is kept and grows. */
#[test]
fn flex_basis_content_keyword_keeps_the_declaration() {
    let [a] = boxes(
        "<div style='display:flex;width:400px'>\
         <div id=a style='flex:1 1 content'><div style='width:120px'>x</div></div></div>",
        ["a"],
    );
    assert!(close(a.width, 400.0), "{}", a.width);
}
