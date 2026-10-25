/** Blink's line box alignment for `vertical-align: top` and `bottom` (MetricsForTopAndBottomAlign):
a top- or bottom-aligned box taller than the aligned subtree keeps that subtree on the edge it
names and extends the line on the other side. */
use mars_eye::{compute_styles, parse, render};

/** The (x, y, width, height) of every item of the element with `id`, plus its text runs' y. */
fn geometry(markup: &str, id: &str) -> ((f64, f64, f64, f64), Vec<f64>) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    let runs = item.lines.iter().map(|line| line.y).collect();
    ((item.x, item.y, item.width, item.height), runs)
}

/** A tall `bottom` box keeps the aligned subtree on the line's bottom edge. */
#[test]
fn tall_bottom_box_pushes_the_subtree_down() {
    let m = "<div id=d style='font:30px/1 serif'><b id=a>x</b><span style='font-size:60px;vertical-align:bottom'>X</span></div>";
    assert_eq!(geometry(m, "d").0.3, 60.0);
    assert_eq!(geometry(m, "a").1, vec![30.0]);
}

/** A tall `top` box keeps the aligned subtree on the line's top edge. */
#[test]
fn tall_top_box_extends_the_line_below() {
    let m = "<div id=d style='font:30px/1 serif'><b id=a>x</b><span style='font-size:60px;vertical-align:top'>X</span></div>";
    assert_eq!(geometry(m, "d").0.3, 60.0);
    assert_eq!(geometry(m, "a").1, vec![0.0]);
}

/** `text-bottom` puts the box's bottom (its leading included) at the parent's font descent, so
the box rises by its half-leading and the line grows above by as much, as CSS2
vertical-align-117a: the text beside it drops by the half-leading. */
#[test]
fn text_bottom_meets_the_parent_font_descent() {
    let m = "<div id=d style='font:40px/130px Ahem'>T<span id=s style='vertical-align:text-bottom'>M</span>T</div>";
    let ((_, _, _, height), block) = geometry(m, "d");
    let (_, span) = geometry(m, "s");
    assert_eq!(span, vec![0.0]);
    assert!(block[0] > 30.0, "{block:?}");
    assert_eq!(height, 130.0 + block[0]);
}

/** `text-top` puts the box's top (its leading included) at the parent's font ascent, so the box
drops by its half-leading and the line grows below. */
#[test]
fn text_top_meets_the_parent_font_ascent() {
    let m = "<div id=d style='font:40px/130px Ahem'>T<span id=s style='vertical-align:text-top'>M</span>T</div>";
    let ((_, _, _, height), block) = geometry(m, "d");
    let (_, span) = geometry(m, "s");
    assert_eq!(block, vec![0.0]);
    assert!(span[0] > 30.0, "{span:?}");
    assert_eq!(height, 130.0 + span[0]);
}
