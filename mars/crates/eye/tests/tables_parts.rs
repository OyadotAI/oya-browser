/** Table boxes and parts: the caption outside the grid box, column and row backgrounds under
their cells, rtl column order, two-value border spacing, fixed-layout percentages, block-size
limits, inline-table, and floated or positioned parts leaving the grid. */
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
/** The first item painted for the element with `id`. */
fn first<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    all(document, display, id)[0]
}

/** A bottom caption sits outside the table box, so the table's background paints only the grid
(caption-side-applies-to-006). */
#[test]
fn table_background_stays_off_its_caption() {
    let (document, display) = layout(
        "<div id=t style='display:table;background:blue;caption-side:bottom'>\
         <div style='display:table-caption;height:20px'></div>\
         <div style='display:table-row'><div style='display:table-cell;height:50px;width:50px'></div></div></div>",
    );
    let painted: Vec<_> = all(&document, &display, "t")
        .into_iter()
        .filter(|item| item.background.is_some())
        .collect();
    assert_eq!(painted.len(), 1);
    assert_eq!(painted[0].height, 50.0);
}

/** A fixed table resolves a content-box cell's percentage width plus its padding
(fixed-table-layout-025: 50% of 100px and 25px padding each side is the whole table). */
#[test]
fn fixed_table_percentage_adds_cell_padding() {
    let (document, display) = layout(
        "<table style='table-layout:fixed;width:100px;border-spacing:0'><tr>\
         <td></td><td id=m style='width:50%;padding:0 25px'></td><td></td></tr></table>",
    );
    assert_eq!(first(&document, &display, "m").width, 100.0);
}

/** A column's background paints under each of its cells, clipped to the cell, and nowhere in
the border spacing (table-backgrounds-bs-column-001). */
#[test]
fn column_background_paints_under_each_cell() {
    let (document, display) = layout(
        "<table style='border-spacing:10px'><col id=c style='background:aqua'>\
         <tr><td style='width:20px;height:20px'></td></tr>\
         <tr><td style='width:20px;height:20px'></td></tr></table>",
    );
    let pieces = all(&document, &display, "c");
    assert_eq!(pieces.len(), 2);
    let clips: Vec<_> = pieces.iter().map(|item| item.clip.unwrap()).collect();
    assert_eq!((clips[0].y, clips[0].height), (10.0, 22.0));
    assert_eq!((clips[1].y, clips[1].height), (42.0, 22.0));
}

/** A row's own box stays for its extent but paints nothing; its background paints under its
cells instead. */
#[test]
fn row_box_is_bare_and_its_background_is_per_cell() {
    let (document, display) = layout(
        "<table style='border-spacing:4px'><tr id=r style='background:pink'>\
         <td style='width:10px'>a</td><td style='width:10px'>b</td></tr></table>",
    );
    let items = all(&document, &display, "r");
    assert!(items[0].bare && items[0].background.is_none());
    assert_eq!(items.iter().filter(|item| item.clip.is_some()).count(), 2);
}

/** An rtl table puts its first column at the right (col-paint-htb-rtl). */
#[test]
fn rtl_table_mirrors_its_columns() {
    let (document, display) = layout(
        "<table style='direction:rtl;border-spacing:0'><tr>\
         <td id=a style='width:100px'></td><td id=b style='width:50px'></td></tr></table>",
    );
    assert!(first(&document, &display, "a").x > first(&document, &display, "b").x);
}

/** `border-spacing` with two lengths spaces columns by the first and rows by the second. */
#[test]
fn two_value_border_spacing() {
    let (document, display) = layout(
        "<table style='border-spacing:2px 10px'><tr><td id=a style='width:10px;height:10px'></td>\
         <td id=b style='width:10px;height:10px'></td></tr>\
         <tr><td id=c style='width:10px;height:10px'></td><td></td></tr></table>",
    );
    let (a, b, c) = (
        first(&document, &display, "a"),
        first(&document, &display, "b"),
        first(&document, &display, "c"),
    );
    assert_eq!(b.x - (a.x + a.width), 2.0);
    assert_eq!(c.y - (a.y + a.height), 10.0);
}

/** A table's block size is held by max-height and floored by min-height. */
#[test]
fn table_height_respects_min_and_max() {
    let (document, display) = layout(
        "<div id=t style='display:table;height:300px;max-height:96px'>\
         <div style='display:table-row'><div style='display:table-cell'>a</div></div></div>\
         <div id=u style='display:table;min-height:96px'>\
         <div style='display:table-row'><div style='display:table-cell'>a</div></div></div>",
    );
    assert_eq!(first(&document, &display, "t").height, 96.0);
    assert_eq!(first(&document, &display, "u").height, 96.0);
}

/** `display: inline-table` is a table on its line, so its rows lay out as its grid. */
#[test]
fn inline_table_lays_out_its_rows() {
    let (document, display) = layout(
        "<div id=t style='display:inline-table'><div style='display:table-row'>\
         <div id=a style='display:table-cell'>a</div><div id=b style='display:table-cell'>b</div></div></div>",
    );
    let (a, b) = (
        first(&document, &display, "a"),
        first(&document, &display, "b"),
    );
    assert_eq!(a.y, b.y);
    assert!(b.x > a.x);
}

/** A floated table part is blockified, so it floats instead of joining the grid
(float-applies-to-001). */
#[test]
fn floated_row_group_is_a_block_float() {
    let (document, display) = layout(
        "<div style='display:table;width:100%'><div id=g style='display:table-row-group;float:right'>\
         <div style='display:table-row'><div style='display:table-cell;width:48px;height:48px'></div></div>\
         </div></div>",
    );
    let group = first(&document, &display, "g");
    assert_eq!(group.x + group.width, 800.0);
}

/** An absolutely positioned caption leaves the table and is placed at the grid's start
(left-applies-to-015). */
#[test]
fn absolute_caption_is_placed() {
    let (document, display) = layout(
        "<div style='display:table'><div id=c style='display:table-caption;position:absolute;left:0;\
         width:96px;height:96px;background:green'></div>\
         <div style='display:table-row'><div style='display:table-cell'></div></div></div>",
    );
    let caption = first(&document, &display, "c");
    assert_eq!(
        (caption.x, caption.width, caption.height),
        (0.0, 96.0, 96.0)
    );
}

/** A `<table>` sizes its border box (the UA sheet's `box-sizing: border-box`); a `display:
table` element keeps `content-box`, so its border adds to its width and height
(height-width-table-001, min-height-table). */
#[test]
fn table_element_sizes_its_border_box() {
    let (document, display) = layout(
        "<table id=t style='width:100px;height:100px;border:10px solid;border-spacing:0'>\
         <tr><td></td></tr></table>\
         <div id=d style='display:table;width:50px;height:50px;border:25px solid'></div>",
    );
    let (table, div) = (
        first(&document, &display, "t"),
        first(&document, &display, "d"),
    );
    assert_eq!((table.width, table.height), (100.0, 100.0));
    assert_eq!((div.width, div.height), (100.0, 100.0));
}

/** An empty `display: table` element is still a table, as wide as its edges
(empty-table-height). */
#[test]
fn empty_table_shrinks_to_its_edges() {
    let (document, display) = layout("<div id=t style='display:table;padding:10px'></div>");
    assert_eq!(first(&document, &display, "t").width, 20.0);
}

/** A table is a new formatting context, so it sits beside an earlier float when its width fits
next to it, and below the float when it does not (floats-wrap-bfc-001-left-table). */
#[test]
fn table_sits_beside_floats() {
    let (document, display) = layout(
        "<div style='width:300px'><div style='float:left;width:100px;height:50px'></div>\
         <table id=a style='width:150px'><tr><td></td></tr></table>\
         <table id=b style='width:250px'><tr><td></td></tr></table></div>",
    );
    let (a, b) = (
        first(&document, &display, "a"),
        first(&document, &display, "b"),
    );
    assert_eq!((a.x, a.y), (100.0, 0.0));
    assert_eq!((b.x, b.y), (0.0, 50.0));
}
