/** Rules of grid round five pinned against Chromium's results: a vertical grid item's
percentage height is auto while the tracks are sized, and row grid-lanes place a fill-reverse
stack by `justify-content`. */
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

/** A vertical item's `max-height: 100%` does not size its auto row from the viewport. */
#[test]
fn a_vertical_items_percentage_height_is_auto_for_track_sizing() {
    let body = "<div id=g style='display:inline-grid;grid:auto/auto;place-items:start'>\
                <div style='writing-mode:vertical-rl;max-height:100%'>\
                <div style='width:30px;height:30px'></div></div></div>";
    assert_eq!(rect(body, "g").3, 30.0);
}

/** `justify-content: end` puts a row fill-reverse stack against the end edge. */
#[test]
fn row_fill_reverse_stack_follows_justify_content() {
    let lanes = |justify: &str| {
        format!(
            "<div style='display:grid-lanes;grid-lanes-direction:row fill-reverse;\
             grid-template-rows:20px;width:200px;justify-content:{justify}'>\
             <i id=a style='display:block;width:50px'></i></div>"
        )
    };
    assert_eq!(rect(&lanes("start"), "a").0, 0.0);
    assert_eq!(rect(&lanes("end"), "a").0, 150.0);
    assert_eq!(rect(&lanes("normal"), "a").0, 150.0);
}
