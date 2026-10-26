/** CSS2 border rules as tables and blocks use them: column borders and `hidden` in the collapsing
model, computed widths and `currentcolor` through inheritance, unit rounding, and tables placed
among floats or around a block inside an inline box. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at 800px with no body margin. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** Every item painted for the element with `id`, in paint order. */
fn all<'a>(document: &Document, display: &'a DisplayList, id: &str) -> Vec<&'a DisplayItem> {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .filter(|item| item.local_id == local)
        .collect()
}
/** The last item painted for `id`: a collapsed cell's edges item. */
fn last<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    all(document, display, id).last().unwrap()
}
/** The first item painted for `id`. */
fn first<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    all(document, display, id)[0]
}

/** A column's border joins the collapsed conflict, so its cells paint it (border-top-applies-to-005). */
#[test]
fn column_border_collapses_into_its_cells() {
    let (document, display) = layout(
        "<table style='border-collapse:collapse'><col style='border-top:4px solid green'>\
         <tr><td id=c style='padding:0;width:10px;height:10px'></td></tr></table>",
    );
    assert_eq!(last(&document, &display, "c").border_widths[0], 4.0);
}

/** `border-style: hidden` wins its edge over any other border and paints nothing there
(border-conflict-style-101). */
#[test]
fn hidden_border_wins_the_collapse() {
    let (document, display) = layout(
        "<table style='border-collapse:collapse'><tr style='border-style:hidden'>\
         <td id=c style='border:3px solid red;width:10px;height:10px'></td></tr></table>",
    );
    assert!(
        all(&document, &display, "c")
            .iter()
            .all(|item| item.border_widths == [0.0; 4] && item.border == 0.0)
    );
}

/** `border-width: inherit` takes the parent's computed width even when the parent's style
is `none` and paints no border (border-width-012). */
#[test]
fn border_width_inherits_the_computed_width() {
    let (document, display) = layout(
        "<div style='border-width:20px'><p id=p style='border-width:inherit;border-style:solid'>x</p></div>",
    );
    assert_eq!(first(&document, &display, "p").border, 20.0);
}

/** An inherited `currentcolor` border color is the child's own color (border-color-011). */
#[test]
fn inherited_currentcolor_is_the_childs_color() {
    let (document, display) = layout(
        "<div style='color:red;border:none'><div id=c style='border-color:inherit;\
         border-style:solid;color:green'>x</div></div>",
    );
    let item = first(&document, &display, "c");
    let color = item.border_color.clone().or(item.border_colors[0].clone());
    assert_eq!(color.as_deref(), Some("green"));
}

/** `25.4mm` is one inch, 96px, despite floating point (border-top-width-050). */
#[test]
fn millimetre_border_floors_to_whole_pixels() {
    let (document, display) =
        layout("<div id=d style='border-top:25.4mm solid;height:0;width:10px'></div>");
    assert_eq!(first(&document, &display, "d").height, 96.0);
}

/** An empty collapsed table has no padding (collapsing-border-model-011). */
#[test]
fn empty_collapsed_table_drops_padding() {
    let (document, display) = layout(
        "<table id=t style='border-collapse:collapse;box-sizing:content-box;width:100px;\
         height:100px;padding:100px'></table>",
    );
    let table = first(&document, &display, "t");
    assert_eq!((table.width, table.height), (100.0, 100.0));
}

/** A table fits beside floats over its whole height, shrinking to the band its height reaches
(table-among-floats-001). */
#[test]
fn table_fits_beside_floats_over_its_height() {
    let (document, display) = layout(
        "<div style='width:200px'><div style='width:75px;height:100px;float:left;clear:left'></div>\
         <div style='width:125px;height:100px;float:left;clear:left'></div>\
         <table id=t cellpadding=0 cellspacing=0><td><div style='float:left;width:75px;height:100px'></div>\
         <div style='float:left;width:75px;height:100px'></div></td></table></div>",
    );
    let table = first(&document, &display, "t");
    assert_eq!((table.x, table.width, table.y), (125.0, 75.0, 0.0));
}

/** A table's `clear` moves it below the floats it clears (clear-applies-to-013). */
#[test]
fn table_clears_floats() {
    let (document, display) = layout(
        "<div style='float:left;width:50px;height:40px'></div>\
         <table id=t style='clear:both'><tr><td>x</td></tr></table>",
    );
    assert_eq!(first(&document, &display, "t").y, 40.0);
}

/** A `display: table` holding only a block wraps it in an anonymous cell, so the table is as
wide as the block (floated-table-wider-than-specified). */
#[test]
fn table_with_a_block_is_as_wide_as_the_block() {
    let (document, display) = layout(
        "<div id=t style='display:table;width:50px'><div style='width:200px;height:10px'></div></div>",
    );
    assert_eq!(first(&document, &display, "t").width, 200.0);
}

/** An inline box whose first content is a block keeps its start border on a line of its own
before the block (block-in-inline-whitespace-001). */
#[test]
fn start_border_gets_a_line_before_a_block() {
    let (document, display) =
        layout("<span id=s style='border:3px solid blue'><div id=d>One</div></span>");
    let pieces = all(&document, &display, "s");
    assert!(pieces.len() >= 2);
    assert!(pieces[0].y < first(&document, &display, "d").y);
    assert_eq!(pieces[0].border_widths[3], 3.0);
}
