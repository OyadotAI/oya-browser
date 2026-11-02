/** CSS grid tracks, placement, gaps and alignment, and automatic table layout. */
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

/** Fixed and fr columns split the width after gaps. */
#[test]
fn fixed_and_fraction_columns_share_width() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:100px 1fr 2fr;column-gap:10px;width:420px'>\
         <div id=a>a</div><div id=b>b</div><div id=c>c</div></div>",
    );
    let (a, b, c) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
        find(&document, &display, "c"),
    );
    assert!(close(a.width, 100.0));
    assert!(close(b.width, 100.0));
    assert!(close(c.width, 200.0));
    assert!(close(b.x, a.x + 110.0));
    assert!(close(c.x, b.x + 110.0));
}

/** Items auto-place row by row and wrap to the next row. */
#[test]
fn items_auto_place_in_rows() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:repeat(2, 1fr);row-gap:5px'>\
         <div id=a>a</div><div id=b>b</div><div id=c>c</div></div>",
    );
    let (a, b, c) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
        find(&document, &display, "c"),
    );
    assert!(close(a.y, b.y));
    assert!(close(c.x, a.x));
    assert!(close(c.y, a.y + a.height + 5.0));
}

/** A span covers several tracks and explicit lines place an item. */
#[test]
fn spans_and_lines_place_items() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:100px 100px 100px'>\
         <div id=a style='grid-column:span 2'>a</div><div id=b style='grid-column:1 / -1'>b</div>\
         <div id=c style='grid-column-start:3;grid-row:1'>c</div></div>",
    );
    assert!(close(find(&document, &display, "a").width, 200.0));
    assert!(close(find(&document, &display, "b").width, 300.0));
    let c = find(&document, &display, "c");
    assert!(close(c.x, find(&document, &display, "a").x + 200.0));
}

/** auto-fill repeats a minmax track as many times as fit. */
#[test]
fn auto_fill_repeats_tracks() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:repeat(auto-fill, minmax(100px, 1fr));width:350px'>\
         <div id=a>a</div><div id=b>b</div><div id=c>c</div><div id=d>d</div></div>",
    );
    let d = find(&document, &display, "d");
    assert!(d.y > find(&document, &display, "a").y);
    assert!(close(find(&document, &display, "a").width, 350.0 / 3.0));
}

/** justify-items: center centers a shrink-to-fit item in its area. */
#[test]
fn justify_items_center() {
    let (document, display) = layout(
        "<div style='display:grid;grid-template-columns:200px;justify-items:center'>\
         <span id=a>ab</span></div>",
    );
    let a = find(&document, &display, "a");
    assert!(close(a.x - 8.0, 200.0 - (a.x - 8.0 + a.width)));
}

/** Table cells honor colspan, presentational widths and background colors, with no invented borders. */
#[test]
fn table_honors_spans_widths_and_colors() {
    let (document, display) = layout(
        "<table cellspacing=0 cellpadding=0 width=400><tr><td id=a width=100 bgcolor=#ff6600>a</td><td id=b>b</td></tr>\
         <tr><td id=c colspan=2>c</td></tr></table>",
    );
    let (a, b, c) = (
        find(&document, &display, "a"),
        find(&document, &display, "b"),
        find(&document, &display, "c"),
    );
    assert!(close(a.width, 100.0));
    assert!(close(b.x, a.x + 100.0));
    assert!(close(c.width, 400.0));
    assert_eq!(a.background.as_deref(), Some("#ff6600"));
    assert_eq!(a.border, 0.0);
}

/** Rows are as tall as their content, not a fixed minimum, and cells center vertically by default. */
#[test]
fn table_rows_fit_content_and_center_cells() {
    let (document, display) = layout(
        "<table cellspacing=0><tr><td id=a>a</td><td><div style='height:60px'></div></td></tr></table>",
    );
    let a = find(&document, &display, "a");
    assert!(close(a.height, 62.0));
    assert!(a.lines[0].y > a.y + 15.0);
}

/** Hostile spans clamp instead of exhausting layout. */
#[test]
fn hostile_spans_are_bounded() {
    let (_, display) = layout(
        "<table><tr><td colspan=999999 rowspan=999999>x</td></tr><tr><td>y</td></tr></table>\
         <div style='display:grid;grid-template-columns:repeat(9999, 1px)'><div style='grid-column:-99999 / 99999'>z</div></div>",
    );
    assert!(display.height.is_finite());
}
