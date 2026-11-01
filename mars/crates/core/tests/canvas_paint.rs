/** Paint sources: gradient stops, geometry and pattern tiling are evaluated per device pixel, in paint space. */
use mars_core::canvas::{Affine, Bitmap, Geometry, Gradient, Paint, Pattern, Repeat, Shader};

/** A linear gradient from red at x=0 to blue at x=10 along the horizontal axis. */
fn red_to_blue() -> Gradient {
    let mut gradient = Gradient::new(Geometry::Linear {
        from: [0.0, 0.0],
        to: [10.0, 0.0],
    });
    gradient.add_stop(0.0, [255, 0, 0, 255]).unwrap();
    gradient.add_stop(1.0, [0, 0, 255, 255]).unwrap();
    gradient
}

/** Paint a gradient through the identity transform and read it at a point. */
fn at(gradient: Gradient, x: f64, y: f64) -> [u8; 4] {
    Shader::new(Paint::Gradient(gradient), Affine::IDENTITY)
        .unwrap()
        .color_at(x, y)
}

/** A one-row bitmap with the given straight-alpha pixels. */
fn row(pixels: &[u8], width: u32) -> Bitmap {
    let mut bitmap = Bitmap::new(width, 1).unwrap();
    bitmap.pixels.make_mut().unwrap().copy_from_slice(pixels);
    bitmap
}

/** Linear colors interpolate across the axis and extend past both ends. */
#[test]
fn linear_gradient_interpolates_and_pads_both_ends() {
    assert_eq!(at(red_to_blue(), 5.0, 0.0), [128, 0, 128, 255]);
    assert_eq!(at(red_to_blue(), -4.0, 0.0), [255, 0, 0, 255]);
    assert_eq!(at(red_to_blue(), 40.0, 0.0), [0, 0, 255, 255]);
}

/** A zero-length axis has no parameter, so the gradient paints nothing. */
#[test]
fn degenerate_linear_axis_paints_nothing() {
    let mut gradient = Gradient::new(Geometry::Linear {
        from: [3.0, 3.0],
        to: [3.0, 3.0],
    });
    gradient.add_stop(0.0, [255, 0, 0, 255]).unwrap();
    assert_eq!(at(gradient, 3.0, 3.0), [0, 0, 0, 0]);
}

/** A gradient without stops paints nothing, and a single stop paints a solid color. */
#[test]
fn empty_and_single_stop_gradients_follow_the_stop_count() {
    let empty = Gradient::new(Geometry::Linear {
        from: [0.0, 0.0],
        to: [1.0, 0.0],
    });
    assert_eq!(at(empty, 0.5, 0.0), [0, 0, 0, 0]);
    let mut single = Gradient::new(Geometry::Linear {
        from: [0.0, 0.0],
        to: [1.0, 0.0],
    });
    single.add_stop(0.5, [9, 8, 7, 255]).unwrap();
    assert_eq!(at(single, 0.9, 0.0), [9, 8, 7, 255]);
}

/** Two stops at the same offset make a hard edge; the later stop owns that offset. */
#[test]
fn equal_offsets_form_a_hard_edge_that_takes_the_later_color() {
    let mut gradient = Gradient::new(Geometry::Linear {
        from: [0.0, 0.0],
        to: [10.0, 0.0],
    });
    gradient.add_stop(0.5, [255, 0, 0, 255]).unwrap();
    gradient.add_stop(0.5, [0, 0, 255, 255]).unwrap();
    assert_eq!(at(gradient, 5.0, 0.0), [0, 0, 255, 255]);
}

/** Offsets outside the unit interval and nonfinite offsets are refused and leave the stops unchanged. */
#[test]
fn out_of_range_stop_offsets_are_refused() {
    let mut gradient = red_to_blue();
    let before = gradient.stops.len();
    assert!(gradient.add_stop(1.5, [0; 4]).is_err());
    assert!(gradient.add_stop(-0.1, [0; 4]).is_err());
    assert!(gradient.add_stop(f64::NAN, [0; 4]).is_err());
    assert_eq!(gradient.stops.len(), before);
}

/** Radial circles interpolate by the radius of the circle that contains the point. */
#[test]
fn radial_gradient_follows_the_containing_circle() {
    let mut gradient = Gradient::new(Geometry::Radial {
        start: [0.0, 0.0, 0.0],
        end: [0.0, 0.0, 10.0],
    });
    gradient.add_stop(0.0, [255, 0, 0, 255]).unwrap();
    gradient.add_stop(1.0, [0, 0, 255, 255]).unwrap();
    assert_eq!(at(gradient.clone(), 5.0, 0.0), [128, 0, 128, 255]);
    assert_eq!(at(gradient, 0.0, 30.0), [0, 0, 255, 255]);
}

/** A negative radius is refused by validation, the same check serialized state passes through. */
#[test]
fn negative_radius_fails_validation() {
    let mut gradient = Gradient::new(Geometry::Radial {
        start: [0.0, 0.0, -1.0],
        end: [0.0, 0.0, 10.0],
    });
    gradient.add_stop(0.0, [0; 4]).unwrap();
    assert!(gradient.validate().is_err());
}

/** Conic angles start at twelve o'clock and run clockwise; the seam closes the turn without a jump. */
#[test]
fn conic_gradient_runs_clockwise_from_twelve_and_wraps() {
    let mut gradient = Gradient::new(Geometry::Conic {
        center: [0.0, 0.0],
        start: 0.0,
    });
    gradient.add_stop(0.0, [255, 0, 0, 255]).unwrap();
    gradient.add_stop(0.5, [0, 0, 255, 255]).unwrap();
    assert_eq!(at(gradient.clone(), 0.0, -1.0), [255, 0, 0, 255]);
    assert_eq!(at(gradient.clone(), 1.0, 0.0), [128, 0, 128, 255]);
    assert_eq!(at(gradient, 0.0, 1.0), [0, 0, 255, 255]);
}

/** A pattern tiles in both directions and reads the tile at each wrapped coordinate. */
#[test]
fn repeated_pattern_tiles_both_axes() {
    let pattern = Pattern::new(row(&[1, 2, 3, 255, 4, 5, 6, 255], 2), Repeat::Both);
    let shader = Shader::new(Paint::Pattern(pattern), Affine::IDENTITY).unwrap();
    assert_eq!(shader.color_at(2.5, 0.5), [1, 2, 3, 255]);
    assert_eq!(shader.color_at(3.5, 7.5), [4, 5, 6, 255]);
}

/** A non-repeating axis is transparent outside the bitmap, while the other axis still tiles. */
#[test]
fn no_repeat_is_transparent_outside_and_repeat_x_tiles_only_horizontally() {
    let pixels = [1, 2, 3, 255, 4, 5, 6, 255];
    let once = Shader::new(
        Paint::Pattern(Pattern::new(row(&pixels, 2), Repeat::Neither)),
        Affine::IDENTITY,
    )
    .unwrap();
    assert_eq!(once.color_at(2.5, 0.5), [0, 0, 0, 0]);
    let across = Shader::new(
        Paint::Pattern(Pattern::new(row(&pixels, 2), Repeat::X)),
        Affine::IDENTITY,
    )
    .unwrap();
    assert_eq!(across.color_at(2.5, 0.5), [1, 2, 3, 255]);
    assert_eq!(across.color_at(0.5, 1.5), [0, 0, 0, 0]);
}

/** The pattern transform places tiles in user space, so a translation moves where the bitmap starts. */
#[test]
fn pattern_transform_moves_the_tile() {
    let mut pattern = Pattern::new(row(&[9, 9, 9, 255], 1), Repeat::Neither);
    pattern.transform = Affine([1.0, 0.0, 0.0, 1.0, 10.0, 0.0]);
    let shader = Shader::new(Paint::Pattern(pattern), Affine::IDENTITY).unwrap();
    assert_eq!(shader.color_at(10.5, 0.5), [9, 9, 9, 255]);
    assert_eq!(shader.color_at(0.5, 0.5), [0, 0, 0, 0]);
}

/** A degenerate drawing transform resolves to no shader, so nothing is painted. */
#[test]
fn singular_transform_resolves_to_no_shader() {
    let singular = Affine([0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    assert!(Shader::new(Paint::Solid([1, 2, 3, 4]), singular).is_none());
}
