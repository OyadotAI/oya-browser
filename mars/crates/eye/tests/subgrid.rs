/** Subgrid regressions (css-grid-2 section 9): inherited tracks, gap-difference margins, edge
 * margins and subgrid items sizing the parent's tracks. */
use mars_eye::{DisplayList, Document, compute_styles, parse, render};

/** A styled document with no body margin, laid out at 800px. */
fn laid_out(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &["body{margin:0}".to_owned()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The `(x, y, width)` of the element with `id`. */
fn spot(document: &Document, display: &DisplayList, id: &str) -> (f64, f64, f64) {
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width)
}

/** A subgrid's items sit in the parent's columns, not in tracks of their own. */
#[test]
fn subgrid_items_take_the_parent_columns() {
    let (document, display) = laid_out(
        "<div style='display:grid;grid-template-columns:100px 50px 70px'>\
         <div style='display:grid;grid-column:1/4;grid-template-columns:subgrid'>\
         <i id=a style='display:block;height:5px'></i><i id=b style='display:block;height:5px'></i>\
         <i id=c style='display:block;height:5px'></i></div></div>",
    );
    assert_eq!(spot(&document, &display, "a"), (0.0, 0.0, 100.0));
    assert_eq!(spot(&document, &display, "b"), (100.0, 0.0, 50.0));
    assert_eq!(spot(&document, &display, "c"), (150.0, 0.0, 70.0));
}

/** A subgrid's own smaller gap leaves the line positions where the parent has them: half the
 * difference goes to each item next to a gutter. */
#[test]
fn a_smaller_subgrid_gap_keeps_the_parent_lines() {
    let (document, display) = laid_out(
        "<div style='display:grid;grid-template-columns:100px 100px;column-gap:20px'>\
         <div style='display:grid;grid-column:1/3;grid-template-columns:subgrid;column-gap:0'>\
         <i id=a style='display:block;height:5px'></i><i id=b style='display:block;height:5px'></i>\
         </div></div>",
    );
    assert_eq!(spot(&document, &display, "a"), (0.0, 0.0, 110.0));
    assert_eq!(spot(&document, &display, "b"), (110.0, 0.0, 110.0));
}

/** An item in a subgrid sizes the parent's `auto` column it lands in, with the subgrid's
 * padding added on the first track. */
#[test]
fn subgrid_items_size_the_parent_tracks() {
    let (document, display) = laid_out(
        "<div style='display:inline-grid;grid-template-columns:auto auto'>\
         <div style='display:grid;grid-column:1/3;grid-template-columns:subgrid;padding-left:10px'>\
         <i id=a style='display:block;width:40px;height:5px'></i>\
         <i id=b style='display:block;width:60px;height:5px'></i></div>\
         <i id=c style='display:block;height:5px'></i><i id=d style='display:block;height:5px'></i>\
         </div>",
    );
    let (c, d) = (
        spot(&document, &display, "c"),
        spot(&document, &display, "d"),
    );
    assert_eq!((c.0, c.2), (0.0, 50.0));
    assert_eq!((d.0, d.2), (50.0, 60.0));
}

/** A subgrid with no items still gives its padding to the first and last tracks. */
#[test]
fn an_empty_subgrid_keeps_its_edges() {
    let (document, display) = laid_out(
        "<div style='display:inline-grid;grid-template-columns:auto auto'>\
         <div id=s style='display:grid;grid-column:1/3;grid-template-columns:subgrid;\
         padding:0 7px 0 13px;height:5px'></div></div>",
    );
    assert_eq!(spot(&document, &display, "s").2, 20.0);
}

/** Auto-placed, a subgrid spans one less track than its line-name lists. */
#[test]
fn line_name_lists_set_an_auto_subgrid_span() {
    let (document, display) = laid_out(
        "<div style='display:grid;grid-template-columns:10px 20px 30px 40px'>\
         <div id=s style='display:grid;grid-template-columns:subgrid [a] [b] [c];height:5px'></div>\
         <i id=n style='display:block;height:5px'></i></div>",
    );
    assert_eq!(spot(&document, &display, "s").2, 30.0);
    assert_eq!(spot(&document, &display, "n").0, 30.0);
}

/** A column subgrid of a grid-lanes container takes the lanes it spans as its columns. */
#[test]
fn a_grid_lanes_column_subgrid_takes_the_lanes() {
    let (document, display) = laid_out(
        "<div style='display:grid-lanes;grid-template-columns:100px 50px 70px'>\
         <div style='display:grid;grid-column:1/4;grid-template-columns:subgrid'>\
         <i id=a style='display:block;height:5px'></i><i id=b style='display:block;height:5px'></i>\
         <i id=c style='display:block;height:5px'></i></div></div>",
    );
    assert_eq!(spot(&document, &display, "a"), (0.0, 0.0, 100.0));
    assert_eq!(spot(&document, &display, "b"), (100.0, 0.0, 50.0));
    assert_eq!(spot(&document, &display, "c"), (150.0, 0.0, 70.0));
}

/** A subgrid in rows only still sizes each parent row from the item in it, rather than
 * spreading its whole height over the rows it spans. */
#[test]
fn a_row_only_subgrid_sizes_each_parent_row() {
    let (document, display) = laid_out(
        "<div style='display:grid;grid-template-rows:auto auto;grid-template-columns:100px'>\
         <div style='display:grid;grid-row:1/3;grid-template-rows:subgrid'>\
         <i id=a style='display:block;height:30px'></i><i id=b style='display:block;height:10px'></i>\
         </div></div>",
    );
    assert_eq!(spot(&document, &display, "b").1, 30.0);
}

/** `name n` past the subgrid's lines lands on an implicit line, which counts as carrying the
 * name, and the area wholly past the subgrid is clamped into its last track. */
#[test]
fn an_implicit_named_line_past_a_subgrid_clamps_into_its_last_track() {
    let (document, display) = laid_out(
        "<div style='display:grid;grid-template-columns:repeat(4, 10px) [a] 10px [a]'>\
         <div style='display:grid;grid-column:1/5;grid-template-columns:subgrid'>\
         <i id=x style='display:block;height:5px;grid-column:span a / a 3'></i></div></div>",
    );
    assert_eq!(spot(&document, &display, "x"), (30.0, 0.0, 10.0));
}

/** Line names inside `repeat()` repeat with their tracks, an auto-repeat group's at layout, and
 * the names after an auto-repeat group move past every repetition. */
#[test]
fn line_names_repeat_with_their_tracks() {
    let (document, display) = laid_out(
        "<div style='display:grid;width:200px;grid-template-columns:repeat(3, [b] 20px)'>\
         <i id=fixed style='display:block;height:5px;grid-column:b 2'></i></div>\
         <div style='display:grid;width:200px;grid-template-columns:repeat(auto-fill, [c] 50px)'>\
         <i id=auto style='display:block;height:5px;grid-column:c 3'></i></div>\
         <div style='display:grid;width:220px;grid-template-columns:repeat(auto-fill, 50px) [d] 20px'>\
         <i id=after style='display:block;height:5px;grid-column:d'></i></div>",
    );
    assert_eq!(spot(&document, &display, "fixed").0, 20.0);
    assert_eq!(spot(&document, &display, "auto").0, 100.0);
    assert_eq!(spot(&document, &display, "after").0, 200.0);
}

/** An auto-placed subgrid of grid lanes inherits no line names (css-grid-3), so a name it
 * cannot find is an implicit line past it, clamped into its last track. */
#[test]
fn an_auto_placed_grid_lanes_subgrid_inherits_no_line_names() {
    let (document, display) = laid_out(
        "<div style='display:grid-lanes;grid-template-columns:[a] 50px 50px'>\
         <div style='display:grid;grid-column:span 2;grid-template-columns:subgrid'>\
         <i id=x style='display:block;height:5px;grid-column:a'></i></div></div>",
    );
    assert_eq!(spot(&document, &display, "x"), (50.0, 0.0, 50.0));
}
