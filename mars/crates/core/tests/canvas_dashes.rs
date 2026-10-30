/** Dash raster regressions exercise phase, transformed distance, closed seams and bounded expansion. */
use mars_core::canvas::{
    Affine, Bitmap, DashStyle, LineCap, Path, StrokeStyle, paint_path, plan_stroke,
    plan_stroke_dashed,
};

/** Render through the public planner, using integer geometry for exact alpha assertions. */
fn paint(path: &Path, segments: Vec<f64>, offset: f64, cap: LineCap, transform: Affine) -> Bitmap {
    let dash = DashStyle { segments, offset };
    let pen = StrokeStyle {
        width: 2.0,
        cap,
        ..StrokeStyle::default()
    };
    let plan = plan_stroke_dashed([16, 16], path, pen, transform, &dash).unwrap();
    let mut bitmap = Bitmap::new(16, 16).unwrap();
    paint_path(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    bitmap
}

/** Read the same backing consumed by the browser instead of internal split-run details. */
fn alpha(bitmap: &Bitmap, x: usize, y: usize) -> u8 {
    bitmap.pixels[(y * 16 + x) * 4 + 3]
}

/** A single horizontal contour makes positive and negative pattern phase visible. */
fn line() -> Path {
    let mut path = Path::default();
    path.move_to([0.0, 4.0], Affine::IDENTITY).unwrap();
    path.line_to([16.0, 4.0], Affine::IDENTITY).unwrap();
    path
}

/** Offset normalization advances positive values into the pattern and wraps negative values. */
#[test]
fn signed_offsets_select_expected_on_and_off_pixels() {
    for (offset, expected) in [
        (0.0, [255, 255, 0, 0]),
        (1.0, [255, 0, 0, 255]),
        (-1.0, [0, 255, 255, 0]),
    ] {
        let bitmap = paint(
            &line(),
            vec![2.0, 2.0],
            offset,
            LineCap::Butt,
            Affine::IDENTITY,
        );
        assert_eq!(
            [
                alpha(&bitmap, 0, 4),
                alpha(&bitmap, 1, 4),
                alpha(&bitmap, 2, 4),
                alpha(&bitmap, 3, 4)
            ],
            expected
        );
    }
}

/** Dash length is measured after inverse-mapping the constructed geometry to current user coordinates. */
#[test]
fn draw_time_transform_scales_dash_length() {
    let bitmap = paint(
        &line(),
        vec![2.0, 2.0],
        0.0,
        LineCap::Butt,
        Affine([2.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
    );
    assert_eq!(
        [
            alpha(&bitmap, 3, 4),
            alpha(&bitmap, 4, 4),
            alpha(&bitmap, 8, 4)
        ],
        [255, 0, 255]
    );
}

/** Short previous subpaths do not carry their dash phase into the next independent contour. */
#[test]
fn each_subpath_restarts_the_pattern() {
    let mut path = Path::default();
    for y in [2.0, 6.0] {
        path.move_to([0.0, y], Affine::IDENTITY).unwrap();
        path.line_to([3.0, y], Affine::IDENTITY).unwrap();
    }
    let bitmap = paint(&path, vec![2.0, 2.0], 0.0, LineCap::Butt, Affine::IDENTITY);
    assert_eq!(
        [
            alpha(&bitmap, 0, 2),
            alpha(&bitmap, 0, 6),
            alpha(&bitmap, 2, 6)
        ],
        [255, 255, 0]
    );
}

/** The first and last ink fragments form an interior miter at a closed path's origin. */
#[test]
fn closed_seam_merges_ink_runs_and_whole_contours_keep_joins() {
    let mut path = Path::default();
    path.rect([4.0, 4.0, 8.0, 8.0], Affine::IDENTITY).unwrap();
    let dashed = paint(&path, vec![5.0, 2.0], 0.0, LineCap::Butt, Affine::IDENTITY);
    assert_eq!(alpha(&dashed, 3, 3), 255);
    let continuous = paint(
        &path,
        vec![100.0, 10.0],
        0.0,
        LineCap::Butt,
        Affine::IDENTITY,
    );
    let plan = plan_stroke(
        [16, 16],
        &path,
        StrokeStyle {
            width: 2.0,
            ..StrokeStyle::default()
        },
        Affine::IDENTITY,
    )
    .unwrap();
    let mut solid = Bitmap::new(16, 16).unwrap();
    paint_path(&mut solid, &plan, [255; 4], 1.0).unwrap();
    assert_eq!(continuous.pixels, solid.pixels);
}

/** Zero-length ink entries paint cap dots while an all-zero pattern remains solid. */
#[test]
fn zero_length_dash_caps_and_all_zero_patterns_are_real_geometry() {
    let butt = paint(
        &line(),
        vec![0.0, 4.0],
        0.0,
        LineCap::Butt,
        Affine::IDENTITY,
    );
    let square = paint(
        &line(),
        vec![0.0, 4.0],
        0.0,
        LineCap::Square,
        Affine::IDENTITY,
    );
    let round = paint(
        &line(),
        vec![0.0, 4.0],
        0.0,
        LineCap::Round,
        Affine::IDENTITY,
    );
    assert_eq!(alpha(&butt, 4, 4), 0);
    assert_eq!(alpha(&square, 4, 4), 255);
    assert!(alpha(&round, 4, 4) > 0);
    assert_eq!(alpha(&square, 2, 4), 0);
    let zero = paint(
        &line(),
        vec![0.0, 0.0],
        99.0,
        LineCap::Butt,
        Affine::IDENTITY,
    );
    let empty = paint(&line(), Vec::new(), 0.0, LineCap::Butt, Affine::IDENTITY);
    assert_eq!(zero.pixels, empty.pixels);
}

/** Tiny positive periods cannot consume unbounded time, and overflowing sums cannot silently draw solid. */
#[test]
fn excessive_split_work_and_period_overflow_are_explicit_refusals() {
    let tiny = DashStyle {
        segments: vec![1.0e-8, 1.0e-8],
        offset: 0.0,
    };
    let error = plan_stroke_dashed(
        [16, 16],
        &line(),
        StrokeStyle::default(),
        Affine::IDENTITY,
        &tiny,
    )
    .unwrap_err();
    assert!(error.message.starts_with("QuotaExceededError:"));
    let huge = DashStyle {
        segments: vec![f64::MAX, f64::MAX],
        offset: 0.0,
    };
    assert!(huge.validate().is_ok());
    assert_eq!(
        plan_stroke_dashed(
            [16, 16],
            &line(),
            StrokeStyle::default(),
            Affine::IDENTITY,
            &huge
        )
        .unwrap_err()
        .code,
        "canvas_geometry"
    );
}
