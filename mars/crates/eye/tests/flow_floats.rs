/** Float placement after Blink's PositionFloat and exclusion space (CSS 2.1 9.5.1), new
formatting contexts beside floats, and inline static positions. Expected values are Chromium's
for 16px sans-serif. */
use mars_eye::{compute_styles, parse, render};

/** (x, y) of `#id` after laying out `markup` at 1100px with no body margin. */
fn place(markup: &str, id: &str) -> (f64, f64) {
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
    (item.x, item.y)
}

/** Within half a pixel of Chromium. */
fn near((x, y): (f64, f64), (ex, ey): (f64, f64)) -> bool {
    (x - ex).abs() < 0.5 && (y - ey).abs() < 0.5
}

/** A left float met mid-line goes beside that line and pushes its content over. */
#[test]
fn a_mid_line_float_that_fits_stays_on_the_line() {
    let markup = "<div><span id=a style='display:inline-block;max-width:150px'>shrink to fit \
                  wraps after a while here</span><div id=b style='float:left'>float text \
                  <b>bold</b></div></div>";
    assert!(
        near(place(markup, "b"), (0.0, 0.0)),
        "{:?}",
        place(markup, "b")
    );
    assert!(
        near(place(markup, "a"), (98.69, 0.0)),
        "{:?}",
        place(markup, "a")
    );
}

/** A float's top is never above an earlier float's (rule 5). */
#[test]
fn a_float_never_rises_above_an_earlier_one() {
    let markup = "<div style='width:300px'><div style='float:left;width:200px;height:20px'></div>\
                  <div style='float:left;width:200px;height:20px'></div>\
                  <div id=c style='float:right;width:50px;height:50px'></div></div>";
    assert!(
        near(place(markup, "c"), (250.0, 20.0)),
        "{:?}",
        place(markup, "c")
    );
}

/** A formatting-context root too wide for the room beside a float moves below it. */
#[test]
fn a_wide_formatting_context_moves_below_floats() {
    let markup = "<div style='width:300px'><div style='float:left;width:100px;height:100px'></div>\
                  <div id=b style='overflow:hidden'>bfc beside float</div>\
                  <div id=c style='display:flow-root;width:250px'>too wide bfc</div></div>";
    assert!(
        near(place(markup, "b"), (100.0, 0.0)),
        "{:?}",
        place(markup, "b")
    );
    assert!(
        near(place(markup, "c"), (0.0, 100.0)),
        "{:?}",
        place(markup, "c")
    );
}

/** A float before a block whose margin collapses through the parent is placed after the
 * margins resolve, so it sits below them. */
#[test]
fn floats_wait_for_the_resolved_block_offset() {
    let markup = "<div><div id=a style='float:left;margin:10px;padding:5px'>pad</div>\
                  <div style='margin-top:30px'>block after</div></div>";
    assert!(
        near(place(markup, "a"), (10.0, 40.0)),
        "{:?}",
        place(markup, "a")
    );
}

/** An inline absolutely positioned box's static position is after a preceding space. */
#[test]
fn inline_static_position_follows_the_space() {
    let markup = "<div style='position:relative'>start <span id=a style='position:absolute'>abs\
                  </span> end</div>";
    assert!(
        near(place(markup, "a"), (35.56, 0.0)),
        "{:?}",
        place(markup, "a")
    );
}
