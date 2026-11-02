/** Radial gradients keep their shape, size and center, so painters place them as CSS does. */
use mars_eye::{BackgroundLayer, Document, compute_styles, parse};

/** The first background layer of `#a` under `background`. */
fn layer(background: &str) -> BackgroundLayer {
    let document: Document = compute_styles(
        &parse("<div id=a>a</div>").unwrap(),
        &[format!("div{{background:{background}}}")],
    )
    .unwrap();
    document.nodes[0].style.backgrounds[0].clone()
}
/** (cx, cy, rx, ry) of a radial layer in a 200 x 100 box. */
fn geometry(background: &str) -> [f64; 4] {
    match layer(background) {
        BackgroundLayer::Radial { shape, .. } => shape.geometry(200.0, 100.0),
        other => panic!("not radial: {other:?}"),
    }
}

#[test]
fn the_default_is_a_centered_farthest_corner_ellipse() {
    let [cx, cy, rx, ry] = geometry("radial-gradient(#fff,#000)");
    assert_eq!((cx, cy), (100.0, 50.0));
    assert!((rx - 100.0 * 2f64.sqrt()).abs() < 1e-9 && (ry - 50.0 * 2f64.sqrt()).abs() < 1e-9);
}

#[test]
fn circle_at_a_position_reaches_the_farthest_corner() {
    let [cx, cy, rx, ry] = geometry("radial-gradient(circle at 50% 0,#fff,#000)");
    assert_eq!((cx, cy), (100.0, 0.0));
    assert!((rx - 100f64.hypot(100.0)).abs() < 1e-9 && rx == ry);
}

#[test]
fn explicit_ellipse_radii_and_keyword_positions() {
    assert_eq!(
        geometry("radial-gradient(ellipse 60% 40% at top,#fff,#000)"),
        [100.0, 0.0, 120.0, 40.0]
    );
    assert_eq!(
        geometry("radial-gradient(60% 70% at 50% 45%,#fff,#0000 70%)"),
        [100.0, 45.0, 120.0, 70.0]
    );
}

#[test]
fn closest_side_touches_the_nearest_sides() {
    assert_eq!(
        geometry("radial-gradient(closest-side,#fff,#0000 75%)"),
        [100.0, 50.0, 100.0, 50.0]
    );
    assert_eq!(
        geometry("radial-gradient(circle closest-side at 20px 30px,#fff,#000)"),
        [20.0, 30.0, 20.0, 20.0]
    );
}

#[test]
fn a_circle_length_sets_one_radius() {
    assert_eq!(
        geometry("radial-gradient(circle 10px,#fff,#000)"),
        [100.0, 50.0, 10.0, 10.0]
    );
    assert_eq!(
        geometry("radial-gradient(10px at left,#fff,#000)"),
        [0.0, 50.0, 10.0, 10.0]
    );
}

#[test]
fn configuration_is_not_counted_as_a_stop() {
    let BackgroundLayer::Radial { stops, .. } = layer("radial-gradient(circle at 50% 0,red,blue)")
    else {
        panic!()
    };
    assert_eq!(stops.len(), 2);
}
