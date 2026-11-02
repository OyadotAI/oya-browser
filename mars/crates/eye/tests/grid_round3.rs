/** Rules of grid round three pinned against Chromium's results: implicit area line names,
grid-lanes `align-content` with fill-reverse (and `safe`), right-to-left row lanes, the
right-to-left `text-indent` edge and right-to-left column flex overflow. */
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

/** A named area gives its edges the names `name-start` and `name-end`. */
#[test]
fn area_edges_carry_implicit_line_names() {
    let body = "<div style='display:grid;grid-template-columns:repeat(4,50px);\
                grid-template-areas:\". a a .\"'><i id=x style='grid-column:a-end;height:5px'></i></div>";
    assert_eq!(rect(body, "x").0, 150.0);
}

/** `align-content` places a fill-reverse stack inside a taller container. */
#[test]
fn fill_reverse_stack_follows_align_content() {
    let lanes = |align: &str| {
        format!(
            "<div style='display:grid-lanes;grid-lanes-direction:column fill-reverse;\
             grid-template-columns:80px;height:100px;align-content:{align}'>\
             <i id=a style='display:block;height:20px'></i></div>"
        )
    };
    assert_eq!(rect(&lanes("start"), "a").1, 0.0);
    assert_eq!(rect(&lanes("end"), "a").1, 80.0);
    assert_eq!(rect(&lanes("normal"), "a").1, 80.0);
}

/** A `safe` fill-reverse stack that overflows keeps its end edge, its start, in place. */
#[test]
fn a_safe_overflowing_fill_reverse_stack_ends_at_the_bottom() {
    let body = "<div style='display:grid-lanes;grid-lanes-direction:column fill-reverse;\
                grid-template-columns:80px;height:30px;align-content:safe start'>\
                <i id=a style='display:block;height:40px'></i></div>";
    assert_eq!(rect(body, "a").1, -10.0);
}

/** Right-to-left row lanes stack from the right edge, alignment mirrored with them. */
#[test]
fn rtl_row_lanes_stack_from_the_right() {
    let body = "<div style='display:grid-lanes;direction:rtl;grid-template-rows:20px;width:200px'>\
                <i id=a style='display:block;width:30px'></i><i id=b style='display:block;width:40px'></i></div>";
    assert_eq!(rect(body, "a").0, 170.0);
    assert_eq!(rect(body, "b").0, 130.0);
}

/** A right-to-left line takes its `text-indent` off its right edge, negative ones outwards. */
#[test]
fn rtl_text_indent_comes_off_the_right_edge() {
    let markup = "<body style='margin:0;font:16px/1 monospace'><div id=d \
                  style='direction:rtl;width:300px;text-indent:-20px'>x</div>";
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let right = display
        .items
        .iter()
        .flat_map(|item| item.lines.iter())
        .find(|line| line.text == "x")
        .map(|line| line.x + line.width)
        .unwrap();
    assert_eq!(right, 320.0);
}

/** A right-to-left column flex item wider than its line overflows to the left. */
#[test]
fn rtl_column_flex_items_overflow_left() {
    let body = "<div style='display:flex;flex-direction:column;direction:rtl;width:100px'>\
                <i id=a style='display:block;width:150px;height:5px'></i></div>";
    assert_eq!(rect(body, "a").0, -50.0);
}
