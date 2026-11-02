/** Polygon path tests exercise visible pixels through Core's public planning and painting contract. */
use mars_core::canvas::{
    Affine, Bitmap, FillRule, LineCap, LineJoin, Path, StrokeStyle, paint_path, plan_fill,
    plan_stroke,
};

/** Inspect canonical straight-alpha bytes without relying on outline implementation details. */
fn pixel(bitmap: &Bitmap, x: usize, y: usize) -> [u8; 4] {
    let at = (y * bitmap.width as usize + x) * 4;
    bitmap.pixels[at..at + 4].try_into().unwrap()
}

/** Both fill rules must operate across all contours in a single compositing operation. */
#[test]
fn nested_contours_obey_evenodd_and_nonzero_fill_rules() {
    let mut path = Path::default();
    path.rect([0.0, 0.0, 8.0, 8.0], Affine::IDENTITY).unwrap();
    path.rect([2.0, 2.0, 4.0, 4.0], Affine::IDENTITY).unwrap();
    for (rule, center) in [(FillRule::Nonzero, [255; 4]), (FillRule::Evenodd, [0; 4])] {
        let mut bitmap = Bitmap::new(8, 8).unwrap();
        let plan = plan_fill([8, 8], &path, rule).unwrap();
        paint_path(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
        assert_eq!(pixel(&bitmap, 0, 0), [255; 4]);
        assert_eq!(pixel(&bitmap, 3, 3), center);
    }
}

/** Opposite winding creates a nonzero hole without changing either contour's stored vertices. */
#[test]
fn reversed_inner_contour_cuts_a_nonzero_hole() {
    let mut path = Path::default();
    path.rect([0.0, 0.0, 8.0, 8.0], Affine::IDENTITY).unwrap();
    path.rect([6.0, 2.0, -4.0, 4.0], Affine::IDENTITY).unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    paint_path(
        &mut bitmap,
        &plan_fill([8, 8], &path, FillRule::Nonzero).unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    assert_eq!(pixel(&bitmap, 3, 3), [0; 4]);
    assert_eq!(pixel(&bitmap, 0, 0), [255; 4]);
}

/** Open triangles fill by implicit closure and retain partially covered diagonal pixels. */
#[test]
fn open_triangle_fill_covers_edges_without_filling_its_bounding_box() {
    let mut path = Path::default();
    path.move_to([0.0, 0.0], Affine::IDENTITY).unwrap();
    path.line_to([8.0, 0.0], Affine::IDENTITY).unwrap();
    path.line_to([0.0, 8.0], Affine::IDENTITY).unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    paint_path(
        &mut bitmap,
        &plan_fill([8, 8], &path, FillRule::Nonzero).unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    assert_eq!(pixel(&bitmap, 0, 0), [255; 4]);
    assert_eq!(pixel(&bitmap, 7, 7), [0; 4]);
    assert!((1..255).contains(&pixel(&bitmap, 3, 4)[3]));
}

/** Overlapping segment and join ink is blended once, preserving the requested global alpha. */
#[test]
fn stroke_miters_join_without_repeated_alpha_and_honor_miter_limit() {
    let mut path = Path::default();
    path.move_to([2.0, 6.0], Affine::IDENTITY).unwrap();
    path.line_to([2.0, 2.0], Affine::IDENTITY).unwrap();
    path.line_to([6.0, 2.0], Affine::IDENTITY).unwrap();
    for (join, limit, corner) in [
        (LineJoin::Miter, 10.0, 128),
        (LineJoin::Miter, 1.0, 0),
        (LineJoin::Bevel, 10.0, 0),
    ] {
        let style = StrokeStyle {
            width: 4.0,
            join,
            miter_limit: limit,
            ..Default::default()
        };
        let mut bitmap = Bitmap::new(8, 8).unwrap();
        paint_path(
            &mut bitmap,
            &plan_stroke([8, 8], &path, style, Affine::IDENTITY).unwrap(),
            [255; 4],
            0.5,
        )
        .unwrap();
        assert_eq!(pixel(&bitmap, 0, 0)[3], corner);
        assert_eq!(pixel(&bitmap, 2, 2)[3], 128);
    }
}

/** Stroke width follows the draw-time transform while path positions remain at construction time. */
#[test]
fn anisotropic_stroke_transform_changes_pen_without_moving_existing_path() {
    let mut path = Path::default();
    path.move_to([2.0, 4.0], Affine::IDENTITY).unwrap();
    path.line_to([6.0, 4.0], Affine::IDENTITY).unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    let pen = StrokeStyle {
        width: 2.0,
        ..Default::default()
    };
    let transform = Affine([1.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
    paint_path(
        &mut bitmap,
        &plan_stroke([8, 8], &path, pen, transform).unwrap(),
        [255; 4],
        1.0,
    )
    .unwrap();
    assert_eq!(pixel(&bitmap, 3, 2), [255; 4]);
    assert_eq!(pixel(&bitmap, 3, 5), [255; 4]);
    assert_eq!(pixel(&bitmap, 3, 6), [0; 4]);
}

/** Round and square caps extend open endpoints while butt caps stop at the endpoint plane. */
#[test]
fn open_stroke_caps_have_distinct_coverage() {
    let mut path = Path::default();
    path.move_to([2.0, 4.0], Affine::IDENTITY).unwrap();
    path.line_to([6.0, 4.0], Affine::IDENTITY).unwrap();
    let mut alphas = Vec::new();
    for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
        let mut bitmap = Bitmap::new(8, 8).unwrap();
        let pen = StrokeStyle {
            width: 4.0,
            cap,
            ..Default::default()
        };
        paint_path(
            &mut bitmap,
            &plan_stroke([8, 8], &path, pen, Affine::IDENTITY).unwrap(),
            [255; 4],
            1.0,
        )
        .unwrap();
        alphas.push(pixel(&bitmap, 0, 2)[3]);
    }
    assert_eq!(alphas[0], 0);
    assert!((1..255).contains(&alphas[1]));
    assert_eq!(alphas[2], 255);
}

/** Planning reports edge cost as well as pixel coverage, so pathological contours cannot bypass work admission. */
#[test]
fn complex_full_canvas_paths_exceed_the_script_work_allowance() {
    let mut path = Path::default();
    for _ in 0..200 {
        path.rect([0.0, 0.0, 512.0, 512.0], Affine::IDENTITY)
            .unwrap();
    }
    assert!(
        plan_fill([512, 512], &path, FillRule::Nonzero)
            .unwrap()
            .work()
            > mars_core::canvas::WORK
    );
}
