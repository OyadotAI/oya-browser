/** Rounded-rectangle geometry covers corner normalization, reflected winding and atomic path admission. */
use mars_core::canvas::{
    Affine, Bitmap, FillRule, Path, StrokeStyle, paint_path, plan_fill, plan_stroke,
};

/** Inspect canonical coverage without exposing private corner or arc representations. */
fn alpha(bitmap: &Bitmap, x: usize, y: usize) -> u8 {
    bitmap.pixels[(y * 8 + x) * 4 + 3]
}

/** Use the public fill surface so all corner arcs and implicit subpaths participate. */
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

/** Rounded corners remove ink from the square corners while preserving the center. */
#[test]
fn rounded_rectangle_draws_real_corner_arcs() {
    let mut path = Path::default();
    path.round_rect([0.0, 0.0, 8.0, 8.0], &[[3.0, 3.0]], Affine::IDENTITY)
        .unwrap();
    let bitmap = fill(&path);
    assert_eq!(alpha(&bitmap, 0, 0), 0);
    assert_eq!(alpha(&bitmap, 3, 3), 255);
    assert!(alpha(&bitmap, 1, 0) > 0);
}

/** Overlapping radii scale together, including values whose direct sum would overflow. */
#[test]
fn huge_radii_scale_without_overflowing_or_becoming_square() {
    let mut expected = Path::default();
    expected
        .round_rect([0.0, 0.0, 8.0, 8.0], &[[4.0, 4.0]], Affine::IDENTITY)
        .unwrap();
    let mut large = Path::default();
    large
        .round_rect([0.0, 0.0, 8.0, 8.0], &[[1e308, 1e308]], Affine::IDENTITY)
        .unwrap();
    assert_eq!(fill(&large), fill(&expected));
}

/** Two and three supplied corners expand with the same diagonal rules as their four-value forms. */
#[test]
fn corner_shorthand_expands_to_the_same_geometry() {
    for (short, full) in [
        (
            vec![[1.0, 2.0], [3.0, 1.0]],
            vec![[1.0, 2.0], [3.0, 1.0], [1.0, 2.0], [3.0, 1.0]],
        ),
        (
            vec![[1.0, 1.0], [2.0, 2.0], [3.0, 1.0]],
            vec![[1.0, 1.0], [2.0, 2.0], [3.0, 1.0], [2.0, 2.0]],
        ),
    ] {
        let mut actual = Path::default();
        actual
            .round_rect([0.0, 0.0, 8.0, 8.0], &short, Affine::IDENTITY)
            .unwrap();
        let mut expected = Path::default();
        expected
            .round_rect([0.0, 0.0, 8.0, 8.0], &full, Affine::IDENTITY)
            .unwrap();
        assert_eq!(fill(&actual), fill(&expected));
    }
}

/** A negative width mirrors corner placement and reverses winding instead of taking its absolute value silently. */
#[test]
fn negative_width_reflects_corner_geometry_and_winding() {
    let mut path = Path::default();
    path.round_rect([0.0, 0.0, 8.0, 8.0], &[[3.0, 3.0]], Affine::IDENTITY)
        .unwrap();
    path.round_rect([8.0, 0.0, -8.0, 8.0], &[[3.0, 3.0]], Affine::IDENTITY)
        .unwrap();
    assert_eq!(alpha(&fill(&path), 3, 3), 0);
}

/** A zero corner axis retains its boundary lines instead of drawing a diagonal across the rectangle. */
#[test]
fn zero_corner_axis_keeps_a_square_outline() {
    let mut path = Path::default();
    path.round_rect([0.0, 0.0, 8.0, 8.0], &[[0.0, 3.0]], Affine::IDENTITY)
        .unwrap();
    assert!(
        fill(&path)
            .pixels
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 255)
    );
}

/** The specified final subpath begins at x,y rather than the first arc's tangent point. */
#[test]
fn line_after_round_rect_starts_at_the_rectangle_origin() {
    let mut path = Path::default();
    path.round_rect([2.0, 2.0, 4.0, 4.0], &[[2.0, 2.0]], Affine::IDENTITY)
        .unwrap();
    path.line_to([2.0, 0.0], Affine::IDENTITY).unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    paint_path(
        &mut bitmap,
        &plan_stroke(
            [8, 8],
            &path,
            StrokeStyle {
                width: 2.0,
                ..Default::default()
            },
            Affine::IDENTITY,
        )
        .unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    assert_eq!(alpha(&bitmap, 1, 0), 255);
    assert_eq!(alpha(&bitmap, 3, 0), 0);
}

/** Quota failure cannot retain some corners or close the previous current subpath. */
#[test]
fn rounded_rectangle_admission_is_atomic() {
    let mut path = Path::default();
    for _ in 0..1020 {
        path.move_to([1.0, 1.0], Affine::IDENTITY).unwrap();
    }
    let before = serde_json::to_value(&path).unwrap();
    assert!(
        path.round_rect([0.0, 0.0, 8.0, 8.0], &[[3.0, 3.0]], Affine::IDENTITY)
            .is_err()
    );
    assert_eq!(serde_json::to_value(&path).unwrap(), before);
}
