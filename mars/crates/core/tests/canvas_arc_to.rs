/** Tangent arc geometry is observable through the same checked path fill and stroke used by V9. */
use mars_core::canvas::{Affine, Bitmap, Path, StrokeStyle, paint_path, plan_stroke};

/** Stroke with a unit pen so rounded corners differ visibly from sharp line intersections. */
fn stroke(path: &Path) -> Bitmap {
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    paint_path(
        &mut bitmap,
        &plan_stroke([8, 8], path, StrokeStyle::default(), Affine::IDENTITY).unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    bitmap
}

/** Inspect exact public pixels rather than private tangent centers or subdivision points. */
fn alpha(bitmap: &Bitmap, x: usize, y: usize) -> u8 {
    bitmap.pixels[(y * 8 + x) * 4 + 3]
}

/** A right-angle turn ends at the outgoing tangent instead of reaching the control corner. */
#[test]
fn tangent_arc_rounds_the_corner_and_stops_at_its_tangent() {
    let mut path = Path::default();
    path.move_to([0.0, 1.0], Affine::IDENTITY).unwrap();
    path.arc_to([7.0, 1.0], [7.0, 7.0], 4.0, Affine::IDENTITY)
        .unwrap();
    let bitmap = stroke(&path);
    assert!(alpha(&bitmap, 1, 1) > 0);
    assert!(alpha(&bitmap, 6, 3) > 0);
    assert_eq!(alpha(&bitmap, 7, 1), 0);
    assert_eq!(alpha(&bitmap, 6, 6), 0);
}

/** Collinear rays and zero radii produce their required straight connection. */
#[test]
fn tangent_arc_degeneracies_are_straight_lines() {
    for (first, second, radius) in [
        ([6.0, 4.0], [7.0, 4.0], 2.0),
        ([6.0, 4.0], [6.0, 7.0], 0.0),
        ([6.0, 4.0], [6.0, 4.0], 2.0),
    ] {
        let mut path = Path::default();
        path.move_to([0.0, 4.0], Affine::IDENTITY).unwrap();
        path.arc_to(first, second, radius, Affine::IDENTITY)
            .unwrap();
        assert!(alpha(&stroke(&path), 3, 3) > 0);
    }
}

/** Previous path geometry is inverse-mapped into the current user coordinates before tangent construction. */
#[test]
fn current_transform_scales_the_pen_geometry_without_moving_the_existing_point() {
    let mut transformed = Path::default();
    transformed.move_to([0.0, 2.0], Affine::IDENTITY).unwrap();
    transformed
        .arc_to(
            [3.0, 1.0],
            [3.0, 4.0],
            1.0,
            Affine([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]),
        )
        .unwrap();
    let mut direct = Path::default();
    direct.move_to([0.0, 2.0], Affine::IDENTITY).unwrap();
    direct
        .arc_to([6.0, 2.0], [6.0, 8.0], 2.0, Affine::IDENTITY)
        .unwrap();
    assert_eq!(stroke(&transformed), stroke(&direct));
}

/** Admission failure leaves neither the tangent connection nor a partial arc behind. */
#[test]
fn tangent_arc_quota_failure_preserves_the_entire_path() {
    let mut path = Path::default();
    for _ in 0..1022 {
        path.move_to([0.0, 1.0], Affine::IDENTITY).unwrap();
    }
    let before = serde_json::to_value(&path).unwrap();
    assert!(
        path.arc_to([7.0, 1.0], [7.0, 7.0], 4.0, Affine::IDENTITY)
            .is_err()
    );
    assert_eq!(serde_json::to_value(&path).unwrap(), before);
}
