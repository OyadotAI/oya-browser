/** `border-collapse: collapse` after Blink's TableBorders: each grid edge takes the widest
border on it, cells use half of their edges and the table half of its outer ones. Expected
values are Chromium's for 16px sans-serif. */
use mars_eye::{compute_styles, parse, render};

/** (x, y, width, height) of `#id` after laying out `markup` at 1100px with no body margin. */
fn rect(markup: &str, id: &str) -> [f64; 4] {
    let markup = format!("<body style='margin:0;font:16px sans-serif'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 1100.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    [item.x, item.y, item.width, item.height]
}

/** Every coordinate within half a pixel of Chromium. */
fn near(actual: [f64; 4], expected: [f64; 4]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() < 0.5)
}

/** Cells with 1px and 3px borders: the table's outer half borders inset the first cell, and
 * the percentage column resolves against the width left inside them. */
#[test]
fn cells_share_half_of_each_collapsed_edge() {
    let markup = "<table style='width:400px;border-collapse:collapse' cellpadding=4><tr>\
                  <td id=a style='width:30%;border:1px solid'>a</td><td id=b style='border:1px solid'>b</td>\
                  <td id=c style='border:3px solid'>c</td></tr></table>";
    assert!(
        near(rect(markup, "a"), [0.5, 1.5, 119.4, 29.5]),
        "{:?}",
        rect(markup, "a")
    );
    assert!(
        near(rect(markup, "c"), [258.82, 1.5, 139.68, 29.5]),
        "{:?}",
        rect(markup, "c")
    );
}

/** The widest of the table's, a cell's and its neighbour's borders wins each edge. */
#[test]
fn the_widest_border_wins() {
    let markup = "<table id=t style='border-collapse:collapse;border:4px solid' cellpadding=2><tr>\
                  <td id=a style='border:2px solid'>a</td><td id=b style='border:6px solid'>b</td></tr>\
                  <tr><td id=c style='border:1px solid'>c</td><td id=d>d</td></tr></table>";
    assert!(
        near(rect(markup, "t"), [0.0, 0.0, 41.8, 61.0]),
        "{:?}",
        rect(markup, "t")
    );
    assert!(
        near(rect(markup, "d"), [19.9, 31.5, 18.9, 27.5]),
        "{:?}",
        rect(markup, "d")
    );
}
