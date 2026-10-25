/** Table ports from Blink's layout/table/, each pinned by the CSS2 or css-tables reftest it fixes. */
use mars_eye::{StyleSource, compute_styles, parse, render};

/** (x, y, width, height) of `#id` after laying out `markup` (with its own `<style>`) at 800px. */
fn place(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let markup = format!("<body style='margin:0;font:16px serif'>{markup}");
    let parsed = parse(&markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width, item.height)
}

/** Baseline-aligned cells share the row's baseline: the one with the most padding above its text
 * sets it and the others move down to it (CSS2 table-vertical-align-baseline-001). */
#[test]
fn baseline_cells_share_the_row_baseline() {
    let markup = "<style>td{vertical-align:baseline}</style><table><tr>\
        <td style='padding-top:40px'><div id=a>data</div></td>\
        <td style='padding-top:20px'><div id=b>data</div></td>\
        <td style='padding-top:0'><div id=c>data</div></td></tr></table>";
    let (a, b, c) = (
        place(markup, "a").1,
        place(markup, "b").1,
        place(markup, "c").1,
    );
    assert!((a - b).abs() < 0.01 && (a - c).abs() < 0.01, "{a} {b} {c}");
    assert!(a >= 40.0, "{a}");
}
/** Table cells directly in a block are wrapped in one anonymous table, so they sit side by side
 * in one row instead of stacking (CSS2 table-anonymous-objects-061). */
#[test]
fn cells_in_a_block_share_an_anonymous_table() {
    let markup = "<div id=d style='padding:1px'><span id=a style='display:table-cell'>one</span>\
        <span id=b style='display:table-cell'>two</span>\
        <span style='display:table-row'><span id=c style='display:table-cell'>three</span></span></div>";
    let (a, b, c) = (place(markup, "a"), place(markup, "b"), place(markup, "c"));
    assert!(
        (a.1 - b.1).abs() < 0.01 && b.0 > a.0 + a.2 - 0.01,
        "{a:?} {b:?}"
    );
    assert!(
        (c.0 - a.0).abs() < 0.01 && c.1 >= a.1 + a.3 - 0.01,
        "{a:?} {c:?}"
    );
    assert!((a.0 - 1.0).abs() < 0.01, "{a:?}");
    assert!(
        (place(markup, "d").2 - 800.0).abs() < 0.01,
        "the block keeps its own width"
    );
}
/** A caption sits outside the table box's border and padding: the grid starts below a top
 * caption past the table's border, and a bottom caption follows the border (CSS 2.1 17.4). */
#[test]
fn captions_sit_outside_the_table_border() {
    let markup = "<div id=t style='display:table;border:5px solid;padding:3px'>\
        <div id=c style='display:table-caption'>Filler</div>\
        <div style='display:table-row'><div id=d style='display:table-cell;height:96px;width:192px'></div></div>\
        <div id=b style='display:table-caption;caption-side:bottom'>Below</div></div>";
    let (t, c, d, b) = (
        place(markup, "t"),
        place(markup, "c"),
        place(markup, "d"),
        place(markup, "b"),
    );
    assert!(
        (c.0 - t.0).abs() < 0.01 && (c.1 - t.1).abs() < 0.01,
        "{t:?} {c:?}"
    );
    assert!((d.1 - (c.1 + c.3 + 8.0)).abs() < 0.01, "{c:?} {d:?}");
    assert!((b.1 - (d.1 + d.3 + 8.0)).abs() < 0.01, "{d:?} {b:?}");
    assert!(
        (t.1 + t.3 - (b.1 + b.3)).abs() < 0.01,
        "the wrapper holds both captions"
    );
}
/** An authored table height counts its captions (Blink subtracts them from the grid's share). */
#[test]
fn table_height_includes_captions() {
    let markup = "<table id=t style='height:200px;border-spacing:0'><caption id=c style='height:50px'>c</caption>\
        <tr id=x><td style='padding:0'>x</td></tr></table>";
    let (t, c, x) = (place(markup, "t"), place(markup, "c"), place(markup, "x"));
    assert!(
        (c.3 - 50.0).abs() < 0.01 && (t.3 - 200.0).abs() < 0.01,
        "{t:?} {c:?}"
    );
    assert!(
        (x.1 - 50.0).abs() < 0.01,
        "the grid starts below the caption: {x:?}"
    );
}
/** A block wrapping a cell and a caption keeps both inside its own padding, the caption first
 * (CSS2 caption-position-001). */
#[test]
fn anonymous_table_captions_stay_in_the_block() {
    let markup = "<div id=w style='border:3px solid;padding:16px;width:98px'>\
        <p id=c style='display:table-cell;margin:0'>cell</p>\
        <p id=t style='display:table-caption;margin:0'>ABOVE</p></div>";
    let (w, c, t) = (place(markup, "w"), place(markup, "c"), place(markup, "t"));
    assert!(
        (t.0 - 19.0).abs() < 0.01 && (t.1 - 19.0).abs() < 0.01,
        "{t:?}"
    );
    assert!((c.1 - (t.1 + t.3)).abs() < 0.01, "{c:?} {t:?}");
    assert!((w.1 + w.3 - (c.1 + c.3 + 19.0)).abs() < 0.01, "{w:?} {c:?}");
}
