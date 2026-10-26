/** CSSOM View offsets: `offsetTop`/`offsetLeft` are measured from the offset parent (the
nearest positioned ancestor, else the body), and `offsetParent` names it. */
mod common;

/** A box of `node` at (x, y) sized 50 by 50, in document coordinates. */
fn node_box(node: usize, x: f64, y: f64) -> mars_v9::NodeBox {
    mars_v9::NodeBox {
        node,
        x,
        y,
        width: 50.0,
        height: 50.0,
    }
}

/** Inside a positioned ancestor, offsets count from it and `offsetParent` is it; its own
scroll offset is added back, so the result does not move as it scrolls. */
#[test]
fn offsets_count_from_the_positioned_ancestor() {
    let mut environment = common::environment();
    environment.positions.insert(0, "relative".into());
    environment.geometry = vec![node_box(0, 10.0, 100.0), node_box(1, 30.0, 140.0)];
    environment.element_scroll.insert(0, [0.0, 20.0]);
    let execution = common::load_with(
        "const first = document.getElementById('first');
         const r = [first.offsetLeft, first.offsetTop, first.offsetParent.id].join('|');",
        &environment,
    );
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "20|60|main");
}

/** Without a positioned ancestor, offsets are from the document and the parent is the body. */
#[test]
fn offsets_without_a_positioned_ancestor_are_from_the_body() {
    let mut environment = common::environment();
    environment.geometry = vec![node_box(0, 10.0, 100.0), node_box(1, 30.0, 140.0)];
    let execution = common::load_with(
        "const first = document.getElementById('first');
         const r = [first.offsetLeft, first.offsetTop, first.offsetParent === document.body].join('|');",
        &environment,
    );
    assert_eq!(common::failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "30|140|true");
}
