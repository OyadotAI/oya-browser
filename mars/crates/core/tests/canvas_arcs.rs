/** Arc contracts exercise real ellipse ink, directed sweeps, transformed geometry and atomic limits. */
use mars_core::canvas::{
    Affine, Bitmap, FillRule, Path, StrokeStyle, paint_path, plan_fill, plan_stroke,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/** Pixel coverage distinguishes arcs from their endpoint chords and bounding rectangles. */
fn alpha(bitmap: &Bitmap, x: usize, y: usize) -> u8 {
    bitmap.pixels[(y * bitmap.width as usize + x) * 4 + 3]
}

/** The normal fill operation closes an open arc with its endpoint chord. */
fn fill(path: &Path) -> Bitmap {
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    paint_path(
        &mut bitmap,
        &plan_fill([8, 8], path, FillRule::Nonzero).unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    bitmap
}

/** Opposite sweep directions select opposite semicircles for the same endpoint angles. */
#[test]
fn arc_sweep_direction_changes_the_filled_half_circle() {
    for (counterclockwise, upper, lower) in [(false, 0, 255), (true, 255, 0)] {
        let mut path = Path::default();
        path.arc(
            [4.0, 4.0],
            3.0,
            [0.0, PI],
            counterclockwise,
            Affine::IDENTITY,
        )
        .unwrap();
        let bitmap = fill(&path);
        assert_eq!(alpha(&bitmap, 4, 2), upper);
        assert_eq!(alpha(&bitmap, 4, 5), lower);
    }
}

/** Nonzero whole turns remain complete while an exactly equal start and end produces no area. */
#[test]
fn whole_turns_are_capped_at_one_circle_and_equal_angles_are_empty() {
    for (end, counterclockwise, expected) in [
        (TAU, false, 255),
        (TAU, true, 255),
        (-TAU, false, 255),
        (-TAU, true, 255),
        (TAU * 3.0, false, 255),
        (0.0, false, 0),
    ] {
        let mut path = Path::default();
        path.arc(
            [4.0, 4.0],
            3.0,
            [0.0, end],
            counterclockwise,
            Affine::IDENTITY,
        )
        .unwrap();
        let bitmap = fill(&path);
        assert_eq!(alpha(&bitmap, 4, 4), expected);
        assert_eq!(alpha(&bitmap, 0, 0), 0);
    }
}

/** Angles just below and above a whole turn wrap in the requested direction without accidental extra turns. */
#[test]
fn reversed_sweeps_wrap_on_both_sides_of_a_whole_turn() {
    for (end, expected) in [(TAU - 0.0001, 0), (TAU + 0.0001, 255)] {
        let mut path = Path::default();
        path.arc([4.0, 4.0], 3.0, [0.0, end], true, Affine::IDENTITY)
            .unwrap();
        assert_eq!(alpha(&fill(&path), 4, 4), expected);
    }
}

/** Ellipse rotation is applied to its unequal radii rather than to a circular substitute. */
#[test]
fn rotated_ellipse_has_real_unequal_axes() {
    let mut horizontal = Path::default();
    horizontal
        .ellipse(
            [4.0, 4.0],
            [3.0, 1.0],
            0.0,
            [0.0, TAU],
            false,
            Affine::IDENTITY,
        )
        .unwrap();
    let mut vertical = Path::default();
    vertical
        .ellipse(
            [4.0, 4.0],
            [3.0, 1.0],
            FRAC_PI_2,
            [0.0, TAU],
            false,
            Affine::IDENTITY,
        )
        .unwrap();
    assert!(alpha(&fill(&horizontal), 6, 3) > 0);
    assert_eq!(alpha(&fill(&vertical), 6, 3), 0);
    assert!(alpha(&fill(&vertical), 3, 6) > 0);
}

/** Construction-time scaling yields the same ellipse as already transformed center and radii. */
#[test]
fn transformed_circle_matches_ellipse_geometry() {
    let mut transformed = Path::default();
    transformed
        .arc(
            [2.0, 4.0],
            1.0,
            [0.0, TAU],
            false,
            Affine([2.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
        )
        .unwrap();
    let mut ellipse = Path::default();
    ellipse
        .ellipse(
            [4.0, 4.0],
            [2.0, 1.0],
            0.0,
            [0.0, TAU],
            false,
            Affine::IDENTITY,
        )
        .unwrap();
    assert_eq!(fill(&transformed), fill(&ellipse));
}

/** Zero-radius circular arcs still connect the current path to their center. */
#[test]
fn degenerate_arc_connects_to_the_center() {
    let mut path = Path::default();
    path.move_to([0.0, 4.0], Affine::IDENTITY).unwrap();
    path.arc([8.0, 4.0], 0.0, [0.0, PI], false, Affine::IDENTITY)
        .unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    let pen = StrokeStyle {
        width: 2.0,
        ..Default::default()
    };
    paint_path(
        &mut bitmap,
        &plan_stroke([8, 8], &path, pen, Affine::IDENTITY).unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    assert_eq!(alpha(&bitmap, 3, 3), 255);
}

/** A whole rejected arc leaves neither its connecting line nor a partial arc in the path. */
#[test]
fn failed_arc_point_admission_is_atomic() {
    let mut path = Path::default();
    for _ in 0..1022 {
        path.move_to([0.0, 0.0], Affine::IDENTITY).unwrap();
    }
    let before = serde_json::to_value(&path).unwrap();
    assert!(
        path.arc([4.0, 4.0], 3.0, [0.0, TAU], false, Affine::IDENTITY)
            .unwrap_err()
            .message
            .starts_with("QuotaExceededError:")
    );
    assert_eq!(serde_json::to_value(&path).unwrap(), before);
}
