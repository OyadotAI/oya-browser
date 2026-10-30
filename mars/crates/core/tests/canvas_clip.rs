/** Immutable sample clips intersect real painter coverage without modifying source paths or saved masks. */
use mars_core::canvas::{
    Affine, Bitmap, ClipMask, FillRule, Path, blit, clear_rect, fill_rect, paint_path, plan_blit,
    plan_fill, plan_rect,
};

/** Construct a clipping region through the public path planner instead of forging private mask samples. */
fn clip(rect: [f64; 4], previous: Option<&ClipMask>) -> ClipMask {
    let mut path = Path::default();
    path.rect(rect, Affine::IDENTITY).unwrap();
    plan_fill([8, 8], &path, FillRule::Nonzero)
        .unwrap()
        .with_clip(previous)
        .unwrap()
        .clip_mask()
        .unwrap()
}

/** Pixel alpha makes geometry intersection and fractional-edge preservation visible. */
fn alpha(bitmap: &Bitmap, x: usize, y: usize) -> u8 {
    bitmap.pixels[(y * 8 + x) * 4 + 3]
}

/** Intersecting a fractional region with itself retains exactly the same individual coverage samples. */
#[test]
fn repeated_fractional_clipping_is_idempotent() {
    let first = clip([0.5, 0.0, 4.0, 8.0], None);
    let second = clip([0.5, 0.0, 4.0, 8.0], Some(&first));
    assert_eq!(first, second);
    assert_eq!(first.coverage(0, 0), 0.5);
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    let plan = plan_rect([8, 8], [0.5, 0.0, 4.0, 8.0], Affine::IDENTITY)
        .unwrap()
        .with_clip(Some(&second))
        .unwrap();
    fill_rect(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    assert_eq!(alpha(&bitmap, 0, 0), 128);
}

/** New intersections share no mutable samples with an older region retained by save(). */
#[test]
fn clip_intersection_preserves_the_previous_mask() {
    let first = clip([0.0, 0.0, 4.0, 8.0], None);
    let second = clip([2.0, 0.0, 4.0, 8.0], Some(&first));
    assert_eq!(first.coverage(0, 0), 1.0);
    assert_eq!(second.coverage(0, 0), 0.0);
    assert_eq!(second.coverage(2, 0), 1.0);
    assert_eq!(second.coverage(5, 0), 0.0);
}

/** Rectangles, clearing and image copies consume the same region without altering source pixels. */
#[test]
fn rectangle_clear_and_image_painters_honor_clip_samples() {
    let clip = clip([2.0, 0.0, 2.0, 8.0], None);
    let plan = plan_rect([8, 8], [0.0, 0.0, 8.0, 8.0], Affine::IDENTITY)
        .unwrap()
        .with_clip(Some(&clip))
        .unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    fill_rect(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    assert_eq!(alpha(&bitmap, 0, 0), 0);
    assert_eq!(alpha(&bitmap, 2, 0), 255);
    clear_rect(&mut bitmap, &plan).unwrap();
    assert_eq!(alpha(&bitmap, 2, 0), 0);
    let mut source = Bitmap::new(1, 1).unwrap();
    source.pixels.make_mut().unwrap().copy_from_slice(&[255; 4]);
    let plan = plan_blit(
        [1, 1],
        [8, 8],
        [0.0, 0.0, 1.0, 1.0],
        [0.0, 0.0, 8.0, 8.0],
        Affine::IDENTITY,
        false,
    )
    .unwrap()
    .with_clip(Some(&clip))
    .unwrap();
    blit(&mut bitmap, &source, &plan, 1.0).unwrap();
    assert_eq!(alpha(&bitmap, 0, 0), 0);
    assert_eq!(alpha(&bitmap, 2, 0), 255);
}

/** Nonzero and evenodd rules produce distinct clip holes, including during subsequent path painting. */
#[test]
fn evenodd_clip_holes_are_preserved_by_path_painting() {
    let mut holes = Path::default();
    holes.rect([0.0, 0.0, 8.0, 8.0], Affine::IDENTITY).unwrap();
    holes.rect([2.0, 2.0, 4.0, 4.0], Affine::IDENTITY).unwrap();
    let mask = plan_fill([8, 8], &holes, FillRule::Evenodd)
        .unwrap()
        .clip_mask()
        .unwrap();
    let mut path = Path::default();
    path.rect([0.0, 0.0, 8.0, 8.0], Affine::IDENTITY).unwrap();
    let plan = plan_fill([8, 8], &path, FillRule::Nonzero)
        .unwrap()
        .with_clip(Some(&mask))
        .unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    paint_path(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    assert_eq!(alpha(&bitmap, 0, 0), 255);
    assert_eq!(alpha(&bitmap, 3, 3), 0);
}

/** Fully clipped painters do not normalize or overwrite transparent RGB supplied by putImageData. */
#[test]
fn empty_clip_preserves_every_destination_byte() {
    let mask = plan_fill([8, 8], &Path::default(), FillRule::Nonzero)
        .unwrap()
        .clip_mask()
        .unwrap();
    let mut bitmap = Bitmap::new(8, 8).unwrap();
    bitmap.pixels.make_mut().unwrap()[..4].copy_from_slice(&[200, 100, 50, 0]);
    let original = bitmap.clone();
    let plan = plan_rect([8, 8], [0.0, 0.0, 8.0, 8.0], Affine::IDENTITY)
        .unwrap()
        .with_clip(Some(&mask))
        .unwrap();
    fill_rect(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    clear_rect(&mut bitmap, &plan).unwrap();
    assert_eq!(bitmap, original);
}

/** A mask cannot be applied to another surface's dimensions, including after a canvas resize. */
#[test]
fn masks_validate_dimensions_and_serialized_storage() {
    let mask = clip([0.0, 0.0, 4.0, 8.0], None);
    assert!(
        plan_rect([4, 4], [0.0, 0.0, 4.0, 4.0], Affine::IDENTITY)
            .unwrap()
            .with_clip(Some(&mask))
            .is_err()
    );
    let mut wire = serde_json::to_value(mask).unwrap();
    wire["samples"] = serde_json::json!([0]);
    let malformed: ClipMask = serde_json::from_value(wire).unwrap();
    assert!(malformed.validate([8, 8]).is_err());
}
