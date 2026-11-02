/** Block margin collapsing and clearance, after Blink's block_layout_algorithm.cc margin strut
and BFC block offset resolution (CSS 2.1 8.3.1 and 9.5.2). */
use mars_eye::{compute_styles, parse, render};

/** Lays out `markup` at 800px with a zero body margin and returns (y, height) for `id`. */
fn place(markup: &str, id: &str) -> (f64, f64) {
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
    (item.y, item.height)
}
/** Equal within layout rounding. */
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/** A first child's top margin collapses through its parent, so both border edges sit at the
collapsed offset and the parent is not taller by its own margin. */
#[test]
fn parent_and_first_child_share_the_collapsed_top_offset() {
    let markup =
        "<div id=a style='margin-top:20px'><div id=b style='margin-top:30px'>x</div></div>";
    let (a, b) = (place(markup, "a"), place(markup, "b"));
    assert!(close(a.0, 30.0) && close(b.0, 30.0), "{a:?} {b:?}");
    assert!(close(a.1, b.1), "{a:?} {b:?}");
}

/** Nested parents all move to the offset their deepest first child resolves. */
#[test]
fn nested_parents_resolve_at_the_deepest_first_child() {
    let markup = "<div id=a><div id=b style='margin-top:10px'><div id=c style='margin-top:25px'>x</div></div></div>";
    for id in ["a", "b", "c"] {
        assert!(
            close(place(markup, id).0, 25.0),
            "{id} {:?}",
            place(markup, id)
        );
    }
}

/** Top padding separates the margins, so the parent stays put. */
#[test]
fn padding_stops_the_parent_child_collapse() {
    let markup =
        "<div id=p style='padding-top:1px'><div id=c style='margin-top:30px'>x</div></div>";
    assert!(close(place(markup, "p").0, 0.0));
    assert!(close(place(markup, "c").0, 31.0));
}

/** Text as first content resolves the strut in place: the parent does not move. */
#[test]
fn a_line_box_resolves_the_parent_in_place() {
    let markup =
        "<div id=p style='margin-top:10px'>text<div id=c style='margin-top:30px'>x</div></div>";
    assert!(close(place(markup, "p").0, 10.0));
}

/** CSS 2.1 9.5.2: clearance puts the border edge at the float's bottom, absorbing a smaller
top margin rather than adding it below the float. */
#[test]
fn clearance_absorbs_the_top_margin() {
    let markup = "<div id=p><div style='float:left;height:50px;width:50px'></div><div id=c style='clear:left;margin-top:10px'>x</div></div>";
    assert!(
        close(place(markup, "c").0, 50.0),
        "{:?}",
        place(markup, "c")
    );
    assert!(close(place(markup, "p").0, 0.0), "{:?}", place(markup, "p"));
}

/** A child clearing a float adjoining its parent's open strut gets clearance whatever its margin
(Blink's HasClearancePastAdjoiningFloats; WPT adjoining-float-before-clearance): it sits at the
float's bottom, and its parent stays with the float instead of moving down with that margin. */
#[test]
fn cleared_child_resolves_its_parent_where_it_stands() {
    let markup = "<div id=p><div style='float:left;height:50px;width:50px'></div><div id=c style='clear:left;margin-top:80px'>x</div></div>";
    assert!(close(place(markup, "c").0, 50.0));
    assert!(close(place(markup, "p").0, 0.0));
}

/** Sibling margins collapse to the larger, and a negative one is added to the largest positive. */
#[test]
fn sibling_margins_collapse_with_negatives() {
    let markup = "<div style='margin-bottom:-10px;height:50px'></div><div id=b style='margin-top:30px'>x</div>";
    assert!(close(place(markup, "b").0, 70.0));
}
