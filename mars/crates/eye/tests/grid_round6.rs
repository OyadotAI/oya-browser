/** Rules of grid round six pinned against Chromium's results: fill-reverse start alignment
moves an item into the opening above it, an auto-repeat fills a minimum width transferred
through `aspect-ratio`, and an auto-placed grid-lanes subgrid gives every lane its larger edge. */
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

/** Under fill-reverse, `align-items: start` moves an item up into the gap a spanner left. */
#[test]
fn fill_reverse_start_alignment_takes_the_opening_above() {
    let body = "<div style='display:grid-lanes;grid-lanes-direction:column fill-reverse;\
                grid-template-columns:50px 50px;height:200px;align-items:start'>\
                <i id=a style='display:block;height:20px'></i>\
                <i style='display:block;height:60px'></i>\
                <i style='display:block;height:10px;grid-column:span 2'></i></div>";
    // Item a sits at the bottom with 40px open above it under the spanner.
    assert_eq!(rect(body, "a").1, 140.0);
}

/** An inline grid's auto-repeat count reaches the width its `min-height` gives through its
 * aspect ratio. */
#[test]
fn auto_repeat_fills_a_minimum_transferred_through_the_aspect_ratio() {
    let body = "<div id=g style='display:inline-grid;aspect-ratio:1/1;min-height:60px;\
                grid-template-columns:repeat(auto-fill,50px)'></div>";
    assert_eq!(rect(body, "g").2, 100.0);
}

/** An auto-placed subgrid spanning two rows gives each row it could land on its larger edge. */
#[test]
fn auto_placed_subgrid_gives_each_lane_its_larger_edge() {
    let body = "<div id=g style='display:inline grid-lanes;grid-template-rows:repeat(4,auto)'>\
                <i style='display:block;width:40px;height:10px'></i>\
                <i style='display:block;width:40px;height:10px'></i>\
                <i style='display:block;width:40px;height:10px'></i>\
                <i style='display:block;width:40px;height:10px'></i>\
                <div style='display:grid;grid:subgrid/auto;grid-row:span 2;\
                padding-top:50px;padding-bottom:40px'></div></div>";
    assert_eq!(rect(body, "g").3, 200.0);
}
