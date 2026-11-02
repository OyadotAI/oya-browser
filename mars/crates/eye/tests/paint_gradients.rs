/** CSS gradient painting ported from Blink and CSS Images 3: `to <corner>` angles from the box,
 * length stop positions, color stop fixup, repeating gradients and clipping to the line. */
use mars_eye::{DisplayBackground, GradientStop, compute_styles, parse, render};

/** The first background layer of a 200x100 box with `background-image: <image>`. */
fn layer(image: &str) -> DisplayBackground {
    let markup =
        format!("<div id=b style='width:200px;height:100px;background-image:{image}'></div>");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .find_map(|item| item.backgrounds.first().cloned())
        .expect("a gradient layer")
}
/** The stops of a gradient layer as (position rounded to 1e-6, color). */
fn stops(layer: &DisplayBackground) -> Vec<(f64, String)> {
    let stops: &Vec<GradientStop> = match layer {
        DisplayBackground::Linear { stops, .. } | DisplayBackground::Radial { stops, .. } => stops,
        other => panic!("not a gradient: {other:?}"),
    };
    stops
        .iter()
        .map(|stop| ((stop.position * 1e6).round() / 1e6, stop.color.clone()))
        .collect()
}

/** `to top right` on a 2:1 box points perpendicular to the top-left/bottom-right diagonal,
 * not at 45 degrees: the line runs two units up for every unit right. */
#[test]
fn corner_direction_follows_the_box_diagonal() {
    let DisplayBackground::Linear { x0, y0, x1, y1, .. } =
        layer("linear-gradient(to top right, red, blue)")
    else {
        panic!("linear expected")
    };
    let (dx, dy) = (x1 - x0, y1 - y0);
    assert!(dx > 0.0 && dy < 0.0);
    assert!((dy / dx + 2.0).abs() < 1e-9, "slope {}", dy / dx);
}

/** Length positions resolve against the gradient line: 10px of a 100px line is 10%. */
#[test]
fn length_stops_resolve_against_the_line() {
    let layer = layer("linear-gradient(red 0, red 10px, blue 10px)");
    assert_eq!(
        stops(&layer),
        vec![
            (0.0, "#ff0000".into()),
            (0.1, "#ff0000".into()),
            (0.1, "#0000ff".into())
        ]
    );
}

/** `calc()` positions add their percentage and length parts. */
#[test]
fn calc_stops_mix_percent_and_length() {
    let layer = layer("linear-gradient(red calc(100% - 20px), blue)");
    assert_eq!(stops(&layer)[1].0, 0.8);
}

/** A repeating gradient tiles its stop span along the whole line. */
#[test]
fn repeating_gradient_tiles_its_pattern() {
    let layer = layer("repeating-linear-gradient(red 0, blue 25%)");
    let positions: Vec<f64> = stops(&layer).iter().map(|stop| stop.0).collect();
    for edge in [0.25, 0.5, 0.75] {
        assert_eq!(
            positions.iter().filter(|p| **p == edge).count(),
            2,
            "a hard edge at {edge} in {positions:?}"
        );
    }
    let colors = stops(&layer);
    assert_eq!(colors.first().unwrap().1, "#ff0000");
    assert!(colors.contains(&(1.0, "#0000ff".into())), "{colors:?}");
}

/** Stops outside the line are kept and the ramp is clipped with interpolated end colors. */
#[test]
fn out_of_range_stops_clip_with_interpolated_colors() {
    let layer = layer("linear-gradient(red -100%, blue 200%)");
    assert_eq!(
        stops(&layer),
        vec![(0.0, "#aa0055".into()), (1.0, "#5500aa".into())]
    );
}

/** A stop before an earlier one moves up to it, making a hard edge (color stop fixup). */
#[test]
fn decreasing_positions_clamp_to_the_largest_before() {
    let layer = layer("linear-gradient(red 60%, blue 20%)");
    assert_eq!(
        stops(&layer),
        vec![
            (0.0, "#ff0000".into()),
            (0.6, "#ff0000".into()),
            (0.6, "#0000ff".into())
        ]
    );
}

/** The `background-position` of a 200x100 box with an image layer, as `[x_px, x_percent,
 * y_px, y_percent]`. */
fn position(value: &str) -> [f64; 4] {
    let markup = format!(
        "<div style='width:200px;height:100px;background-image:url(a.png);background-position:{value}'></div>"
    );
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .find_map(|item| match item.backgrounds.first() {
            Some(DisplayBackground::Image { position, .. }) => Some(*position),
            _ => None,
        })
        .expect("an image layer")
}

/** Four-value positions measure from the named edges: `right 10px bottom 20%`. */
#[test]
fn four_value_position_offsets_from_edges() {
    assert_eq!(position("right 10px bottom 20%"), [-10.0, 100.0, 0.0, 80.0]);
}

/** Three-value positions take one edge offset: `left 15px top`. */
#[test]
fn three_value_position_takes_one_edge_offset() {
    assert_eq!(position("left 15px top"), [15.0, 0.0, 0.0, 0.0]);
}

/** Two values are x then y, or keywords in either order; one value centers the other axis. */
#[test]
fn one_and_two_value_positions() {
    assert_eq!(position("top left"), [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(position("left 10px"), [0.0, 0.0, 10.0, 0.0]);
    assert_eq!(position("bottom"), [0.0, 50.0, 0.0, 100.0]);
}
