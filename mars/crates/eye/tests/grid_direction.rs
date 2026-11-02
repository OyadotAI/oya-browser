/** Grid rules of round two pinned against Chromium's results: right-to-left grids and their
abspos static positions, `left`/`right`/`self-*` and `safe` self-alignment, grid-lanes stretch
across openings, explicit stretch of ratio-free replaced items, and the right-to-left
over-constrained block margin. */
use mars_eye::{compute_styles, parse, render};

/** Laid-out box of one element: (x, y, width, height). */
type Rect = (f64, f64, f64, f64);

/** The first box of the element with `id`, laid out at 800px with no body margin. */
fn rect(body: &str, id: &str) -> Rect {
    let markup = format!("<body style='margin:0;font:16px/1 monospace'>{body}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == id).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == node.local_id)
        .unwrap();
    (item.x, item.y, item.width, item.height)
}

/** A grid of 30px with an abspos child in the 20px middle column of `2px 20px 2px`, the child
 * styled `style`. */
fn abspos(style: &str) -> String {
    format!(
        "<div style='display:grid;direction:rtl;position:relative;width:24px;height:10px;\
         grid:10px/2px 20px 2px'><i id=a style='position:absolute;grid-column:2/3;\
         width:6px;height:4px;{style}'></i></div>"
    )
}

/** Under `direction: rtl` the first column is the rightmost. */
#[test]
fn rtl_columns_run_right_to_left() {
    let body = "<div style='display:grid;direction:rtl;width:300px;grid-template-columns:100px 50px'>\
                <i id=a style='height:5px'></i><i id=b style='height:5px'></i></div>";
    assert_eq!(rect(body, "a").0, 200.0);
    assert_eq!(rect(body, "b").0, 150.0);
}

/** An rtl grid's abspos child starts at the right edge of its area. */
#[test]
fn an_rtl_abspos_static_position_is_the_area_right_edge() {
    assert_eq!(rect(&abspos(""), "a").0, 16.0);
}

/** `left` and `right` are physical, so `left` is the end of an rtl grid. */
#[test]
fn justify_self_left_and_right_are_physical() {
    assert_eq!(rect(&abspos("justify-self:left"), "a").0, 2.0);
    assert_eq!(rect(&abspos("justify-self:right"), "a").0, 16.0);
}

/** `left` is no `align-self` value, so the declaration is dropped. */
#[test]
fn align_self_left_is_invalid() {
    let body = "<div style='display:grid;grid:20px/20px;position:relative'>\
                <i id=a style='display:block;height:4px;align-self:end;align-self:left'></i></div>";
    assert_eq!(rect(body, "a").1, 16.0);
}

/** A `safe` alignment of an overflowing abspos box keeps it at the start. */
#[test]
fn safe_alignment_keeps_an_overflowing_box_at_the_start() {
    let grid = |align: &str| {
        format!(
            "<div style='display:grid;position:relative;width:20px;height:20px;grid:10px/10px'>\
             <i id=a style='position:absolute;grid-area:1/1/2/2;width:4px;height:16px;\
             align-self:{align}'></i></div>"
        )
    };
    assert_eq!(rect(&grid("safe end"), "a").1, 0.0);
    assert_eq!(rect(&grid("end"), "a").1, -6.0);
}

/** An authored `stretch` item fills the opening a spanner below leaves in its lane. */
#[test]
fn grid_lanes_stretch_fills_the_opening_above_a_spanner() {
    let body = "<div style='display:grid-lanes;grid-template-columns:50px 50px;align-items:stretch'>\
                <i id=a style='display:block'>x</i><i style='display:block;height:60px'></i>\
                <i style='display:block;height:10px;grid-column:span 2'></i></div>";
    assert_eq!(rect(body, "a").3, 60.0);
}

/** An `<svg>` with no `viewBox` has no aspect ratio, so an authored `stretch` fills the area. */
#[test]
fn an_svg_without_a_ratio_stretches_when_asked() {
    let body = "<div style='display:grid;width:350px;height:250px'>\
                <svg id=a style='justify-self:stretch;align-self:stretch'></svg></div>";
    assert_eq!(rect(body, "a").2, 350.0);
    assert_eq!(rect(body, "a").3, 250.0);
}

/** In an rtl parent an over-constrained block keeps its right margin (CSS 2.1 10.3.3). */
#[test]
fn an_rtl_overconstrained_block_aligns_right() {
    let body = "<div style='direction:rtl;width:300px'>\
                <div id=a style='width:100px;margin:0 10px 0 20px;height:5px'></div></div>";
    assert_eq!(rect(body, "a").0, 190.0);
}
