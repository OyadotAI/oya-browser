/** Rules of grid round seven pinned against Chromium's results: a vertical item joins its row's
baseline group with its content box's top plus its font's ascent, horizontal items in a column
line up their synthesized border-box end edges under `justify-self: baseline`, and a subgrid's
`repeat(auto-fill, ...)` line names repeat to fill its span. */
use mars_eye::{compute_styles, parse, render};

/** Laid-out box of one element: (x, y, width, height). */
type Rect = (f64, f64, f64, f64);

/** The first box of the element with `id`, laid out at 800px with no body margin. */
fn rect(body: &str, id: &str) -> Rect {
    let markup = format!("<body style='margin:0;font:20px/1 monospace'>{body}");
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

/** A vertical item with 30px of top padding sits higher than a plain horizontal item by the
 * difference of their content-box ascents. */
#[test]
fn a_vertical_item_joins_the_row_baseline_with_its_content_top() {
    let body = "<div style='display:grid;grid-template-columns:100px 100px;align-items:baseline'>\
                <div id=h>x</div>\
                <div id=v style='writing-mode:vertical-lr;padding-top:30px;height:60px'>x</div></div>";
    assert_eq!(rect(body, "h").1, 30.0);
    assert_eq!(rect(body, "v").1, 0.0);
}

/** `justify-items: baseline` lines horizontal items of one column up on their end edges. */
#[test]
fn column_baseline_aligns_border_box_end_edges() {
    let body = "<div style='display:grid;grid-template-columns:100px;justify-items:baseline'>\
                <div id=a style='width:40px;height:5px'></div>\
                <div id=b style='width:70px;height:5px'></div></div>";
    let (a, b) = (rect(body, "a"), rect(body, "b"));
    assert_eq!(a.0 + a.2, b.0 + b.2);
    assert_eq!(b.0, 0.0);
}

/** An auto-fill group of names repeats as often as the subgrid's lines allow, so `x -1`
 * reaches the last repetition's line. */
#[test]
fn subgrid_auto_fill_names_fill_the_span() {
    let body = "<div style='display:grid;grid-auto-columns:20px'>\
                <div style='display:grid;grid-column:1/span 4;\
                grid-template-columns:subgrid [] repeat(auto-fill,[x])'>\
                <i id=a style='grid-column:x -2/x -1;height:5px'></i></div></div>";
    assert_eq!(rect(body, "a").0, 60.0);
}
