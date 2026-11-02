/** The automatic minimum size of grid items (css-grid-1 6.6) and the intrinsic size it feeds,
pinned against Blink's special spanning criteria: an item spanning more than one flexible track,
or none with an `auto` minimum, has a zero minimum contribution, so its min-content width never
opens fractional tracks. A percentage-width grid in an auto track takes the area it is given. */
use mars_eye::{compute_styles, parse, render};

/** Border box (x, y, width, height) of the first element with `id`, laid out at 1100px. */
fn rect(body: &str, id: &str) -> (f64, f64, f64, f64) {
    let markup = format!("<body style='margin:0;font:16px/1 monospace'>{body}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 1100.0).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == id).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == node.local_id)
        .unwrap();
    (item.x, item.y, item.width, item.height)
}

/** Track fractions are not exact in binary, so widths are compared within a sub-pixel tolerance. */
const TOLERANCE: f64 = 1e-6;

/** Asserts that a laid-out width is the expected one, within the tolerance. */
fn assert_width(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < TOLERANCE,
        "{actual} != {expected}"
    );
}

/** An item spanning 18 of 24 `1fr` tracks keeps its 3000px child out of the track sizes: its
automatic minimum is zero, so the tracks are the 1100px grid's own fractions, 821px for the span. */
#[test]
fn item_spanning_many_fr_tracks_has_no_content_minimum() {
    let body = "<div style='display:grid;width:1100px;grid-template-columns:repeat(24,1fr);column-gap:16px'>\
                <i id=x style='display:block;grid-column:1/span 18'>\
                <i style='display:block;width:3000px;height:10px'></i></i></div>";
    assert_width(rect(body, "x").2, 821.0);
}

/** A single `1fr` track still has the content-based minimum: its track grows to the 300px child. */
#[test]
fn item_spanning_one_fr_track_keeps_its_content_minimum() {
    let body = "<div style='display:grid;width:200px;grid-template-columns:1fr'>\
                <i id=x style='display:block'><i style='display:block;width:300px;height:10px'></i></i></div>";
    assert_width(rect(body, "x").2, 300.0);
}

/** An explicit `min-width` is the minimum contribution; the content-based one does not replace it. */
#[test]
fn explicit_min_width_is_the_minimum_contribution() {
    let body = "<div style='display:grid;width:1100px;grid-template-columns:repeat(24,1fr);column-gap:16px'>\
                <i id=x style='display:block;grid-column:1/span 18;min-width:900px'>\
                <i style='display:block;width:3000px;height:10px'></i></i></div>";
    assert_width(rect(body, "x").2, 900.0);
}

/** A percentage-width grid in an auto track takes the definite width of its grid area, and its
flexible columns then size the span from that width rather than from the wide child's min-content. */
#[test]
fn percentage_width_grid_in_auto_track_takes_its_area() {
    let body = "<div style='display:grid;width:1100px;justify-items:center'>\
                <div id=g style='display:grid;width:100%;grid-template-columns:repeat(24,1fr);column-gap:16px'>\
                <i id=x style='display:block;grid-column:1/span 18'>\
                <i style='display:block;width:3000px;height:10px'></i></i></div></div>";
    assert_width(rect(body, "g").2, 1100.0);
    assert_width(rect(body, "x").2, 821.0);
}

/** css-grid-1 6.6 for rows: an item spanning several flexible rows has a zero automatic minimum,
so its 300px content does not open the `1fr` rows of a 200px grid past their share (Blink's
special spanning criteria, as for columns). */
#[test]
fn item_spanning_flexible_rows_does_not_open_them_to_its_content() {
    let (_, y, _, _) = rect(
        "<div style='display:grid;grid-template-rows:1fr 1fr;height:200px'>\
         <div style='grid-row:1 / span 2'><div style='height:300px'>x</div></div>\
         <div id=c style='grid-column:2;grid-row:2'>c</div></div>",
        "c",
    );
    assert_eq!(y, 100.0);
}
