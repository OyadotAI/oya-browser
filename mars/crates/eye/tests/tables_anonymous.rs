/** CSS 2.1 17.2.1 anonymous table objects, inline box fragments around blocks and lines, and
`display: contents` boxes, as they show in the display list. */
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

/** Rows between other blocks each get their own anonymous table, so their cells sit side by
side instead of stacking (table-anonymous-objects-083). */
#[test]
fn row_among_blocks_gets_an_anonymous_table() {
    let (document, display) = layout(
        "<div><span style='display:table-row'><span id=a style='display:table-cell'>a</span>\
         <span id=b style='display:table-cell'>b</span></span><div>block</div></div>",
    );
    let (a, b) = (
        first(&document, &display, "a"),
        first(&document, &display, "b"),
    );
    assert_eq!(a.y, b.y);
    assert!(b.x > a.x);
}

/** A run of non-cells inside a row is one anonymous cell, not one cell each. */
#[test]
fn non_cells_in_a_row_share_one_cell() {
    let (document, display) = layout(
        "<div style='display:table-row'><span id=a>one</span><span id=b>two</span>\
         <span id=c style='display:table-cell'>c</span></div>",
    );
    let (a, b) = (
        first(&document, &display, "a"),
        first(&document, &display, "b"),
    );
    assert_eq!(a.y, b.y);
    assert!((b.x - (a.x + a.width)).abs() < 0.5);
}

/** Anonymous boxes paint nothing and never reach the display list. */
#[test]
fn anonymous_boxes_leave_no_items() {
    let markup = "<div><span style='display:table-cell'>a</span></div>";
    let (document, display) = layout(markup);
    let known: Vec<usize> = document.nodes.iter().map(|node| node.local_id).collect();
    assert!(
        display
            .items
            .iter()
            .all(|item| known.contains(&item.local_id))
    );
}

/** An inline box split by a block paints one fragment before the block and one after it, the
start border only on the first and the end border only on the last. */
#[test]
fn inline_box_around_a_block_splits() {
    let (document, display) =
        layout("<div><span id=s style='border:3px solid blue'>one<div>block</div>two</span></div>");
    let pieces = all(&document, &display, "s");
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0].border_widths, [3.0, 0.0, 3.0, 3.0]);
    assert_eq!(pieces[1].border_widths, [3.0, 3.0, 3.0, 0.0]);
    assert!(pieces[1].y > pieces[0].y + pieces[0].height);
}

/** An inline box that wraps paints one fragment per line, each with its own runs. */
#[test]
fn wrapped_inline_box_paints_per_line() {
    let (document, display) = layout(
        "<div style='width:100px;font:16px/1 monospace'><span id=s style='background:pink'>\
         aaaa bbbb cccc dddd</span></div>",
    );
    let pieces = all(&document, &display, "s");
    assert_eq!(pieces.len(), 2);
    assert!(pieces.iter().all(|piece| piece.lines.len() == 1));
}

/** A `display: contents` element generates no box, so its own border never paints
(display-contents-inline-001). */
#[test]
fn contents_element_paints_no_border() {
    let (document, display) =
        layout("<span>P<div id=c style='display:contents;border:10px solid red'>A</div>SS</span>");
    assert!(
        all(&document, &display, "c")
            .iter()
            .all(|item| item.border == 0.0 && item.border_widths == [0.0; 4])
    );
}
