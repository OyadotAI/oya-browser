/** Public curve operations must produce curved ink, bounded storage and transactional path updates. */
use mars_core::canvas::{
    Affine, Bitmap, FillRule, Path, StrokeStyle, paint_path, plan_fill, plan_stroke,
};

/** Pixel alpha is enough to distinguish a real curve from its chord or control polygon. */
fn alpha(bitmap: &Bitmap, x: usize, y: usize) -> u8 {
    bitmap.pixels[(y * bitmap.width as usize + x) * 4 + 3]
}

/** Fill the open curve with its implicit closing chord, using the ordinary path rasterizer. */
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

/** A quadratic arch encloses pixels that its endpoint chord alone cannot cover. */
#[test]
fn quadratic_curve_fills_the_bowed_region() {
    let mut path = Path::default();
    path.move_to([0.0, 8.0], Affine::IDENTITY).unwrap();
    path.quadratic_curve_to([4.0, -8.0], [8.0, 8.0], Affine::IDENTITY)
        .unwrap();
    let bitmap = fill(&path);
    assert_eq!(alpha(&bitmap, 3, 4), 255);
    assert_eq!(alpha(&bitmap, 0, 0), 0);
}

/** A cubic arch is not substituted with its control polygon or a straight segment. */
#[test]
fn cubic_curve_fills_its_shape_and_preserves_empty_corners() {
    let mut path = Path::default();
    path.move_to([0.0, 8.0], Affine::IDENTITY).unwrap();
    path.bezier_curve_to([0.0, 0.0], [8.0, 0.0], [8.0, 8.0], Affine::IDENTITY)
        .unwrap();
    let bitmap = fill(&path);
    assert_eq!(alpha(&bitmap, 3, 4), 255);
    assert_eq!(alpha(&bitmap, 0, 0), 0);
}

/** Coincident endpoints can still describe a loop with real stroke extent. */
#[test]
fn collinear_backtracking_cubic_is_not_flattened_to_a_point() {
    let mut path = Path::default();
    path.move_to([4.0, 4.0], Affine::IDENTITY).unwrap();
    path.bezier_curve_to([20.0, 4.0], [-12.0, 4.0], [4.0, 4.0], Affine::IDENTITY)
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
    assert_eq!(alpha(&bitmap, 1, 3), 255);
    assert_eq!(alpha(&bitmap, 6, 3), 255);
}

/** Applying a transform at construction is equivalent to supplying already transformed controls. */
#[test]
fn curve_flattening_is_measured_after_the_construction_transform() {
    let mut transformed = Path::default();
    transformed.move_to([0.0, 8.0], Affine::IDENTITY).unwrap();
    transformed
        .bezier_curve_to(
            [0.0, 0.0],
            [4.0, 0.0],
            [4.0, 4.0],
            Affine([2.0, 0.0, 0.0, 2.0, 0.0, 0.0]),
        )
        .unwrap();
    let mut direct = Path::default();
    direct.move_to([0.0, 8.0], Affine::IDENTITY).unwrap();
    direct
        .bezier_curve_to([0.0, 0.0], [8.0, 0.0], [8.0, 8.0], Affine::IDENTITY)
        .unwrap();
    assert_eq!(fill(&transformed), fill(&direct));
}

/** Adaptive point exhaustion must not leave a prefix of the attempted curve in the path. */
#[test]
fn curve_admission_failure_preserves_the_entire_serialized_path() {
    let mut path = Path::default();
    for _ in 0..1022 {
        path.move_to([0.0, 8.0], Affine::IDENTITY).unwrap();
    }
    let before = serde_json::to_value(&path).unwrap();
    let error = path
        .quadratic_curve_to([4.0, -8.0], [8.0, 8.0], Affine::IDENTITY)
        .unwrap_err();
    assert!(error.message.starts_with("QuotaExceededError:"));
    assert_eq!(serde_json::to_value(&path).unwrap(), before);
    path.line_to([8.0, 8.0], Affine::IDENTITY).unwrap();
}

/** The same numeric geometry guard applies before any empty subpath is introduced. */
#[test]
fn invalid_transformed_curve_coordinates_leave_an_empty_path_unchanged() {
    let mut path = Path::default();
    let before = serde_json::to_value(&path).unwrap();
    assert!(
        path.bezier_curve_to(
            [1.0, 0.0],
            [2.0, 0.0],
            [3.0, 0.0],
            Affine([1e20, 0.0, 0.0, 1.0, 0.0, 0.0])
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(&path).unwrap(), before);
}
