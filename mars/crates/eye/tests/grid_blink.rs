/** Ports of Blink's grid layout algorithm, each pinned by the behavior it fixed. */
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
    (a - b).abs() < 0.01
}

/** A statically positioned abspos child of a grid that is not its containing block is aligned
 * by `align-items` and `justify-items` in the grid's content box (PlaceOutOfFlowItems). */
#[test]
fn static_abspos_child_aligns_in_the_content_box() {
    let (document, display) = layout(
        "<body style='margin:0'><div id=g style='display:grid;width:100px;height:100px;\
         border:1px solid;padding:5px;align-items:center;justify-items:end'>\
         <div id=a style='position:absolute;width:50px;height:20px'></div></div></body>",
    );
    let (grid, item) = (
        find(&document, &display, "g"),
        find(&document, &display, "a"),
    );
    assert!(close(item.x, grid.x + 6.0 + 50.0), "x {}", item.x);
    assert!(close(item.y, grid.y + 6.0 + 40.0), "y {}", item.y);
}

/** When the grid is the containing block, the static position rectangle is the child's grid
 * area, and its self alignment places it there; `auto` lines are the padding edges. */
#[test]
fn static_abspos_child_aligns_in_its_grid_area() {
    let (document, display) = layout(
        "<body style='margin:0'><div id=g style='display:grid;position:relative;\
         grid:10px 40px / 20px 60px;width:100px;height:80px;padding:4px'>\
         <div id=a style='position:absolute;grid-area:2/2/3/3;width:10px;height:10px;\
         justify-self:center;align-self:end'></div>\
         <div id=b style='position:absolute;grid-column:2;width:10px;height:10px;\
         justify-self:end'></div></div></body>",
    );
    let grid = find(&document, &display, "g");
    let a = find(&document, &display, "a");
    assert!(close(a.x, grid.x + 4.0 + 20.0 + 25.0), "a.x {}", a.x);
    assert!(close(a.y, grid.y + 4.0 + 10.0 + 30.0), "a.y {}", a.y);
    // `grid-column: 2` runs from line 2 to the end padding edge.
    let b = find(&document, &display, "b");
    assert!(close(b.x, grid.x + 108.0 - 10.0), "b.x {}", b.x);
}

/** An auto-fill row group repeats as often as it fits a definite height, as columns do in a
 * definite width (css-grid-2 7.2.3.2); the items past it go to implicit rows. */
#[test]
fn auto_fill_rows_repeat_into_a_definite_height() {
    let (document, display) = layout(
        "<body style='margin:0'><div id=g style='display:grid;height:100px;\
         grid-template-rows:repeat(auto-fill, 20px)'>\
         <div></div><div></div><div id=c></div></div></body>",
    );
    let grid = find(&document, &display, "g");
    let c = find(&document, &display, "c");
    assert!(close(c.y, grid.y + 40.0), "c.y {}", c.y);
    assert!(close(c.height, 20.0), "c.height {}", c.height);
}

/** Empty `auto-fit` repetitions collapse with their gutters, so content distribution sees only
 * the occupied tracks (css-grid-2 7.2.3.2). */
#[test]
fn empty_auto_fit_tracks_collapse() {
    let (document, display) = layout(
        "<body style='margin:0'><div id=g style='display:grid;width:200px;height:100px;\
         grid-template-columns:repeat(auto-fit, 50px);grid-template-rows:repeat(auto-fit, 20px);\
         column-gap:10px;justify-content:center;align-content:end'>\
         <div id=a style='grid-row:2'></div></div></body>",
    );
    let grid = find(&document, &display, "g");
    let a = find(&document, &display, "a");
    assert!(close(a.x, grid.x + 75.0), "a.x {}", a.x);
    assert!(close(a.width, 50.0), "a.width {}", a.width);
    assert!(close(a.y, grid.y + 80.0), "a.y {}", a.y);
}
