/** Compositing regressions compare premultiplied results across public rect, path and image painters. */
use mars_core::canvas::{
    Affine, Bitmap, Composite, FillRule, Path, blit, clear_rect, composite, fill_rect, paint_path,
    plan_blit, plan_fill, plan_rect,
};

/** One partially transparent red pixel makes both color weights and destination alpha observable. */
fn backing() -> Bitmap {
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    bitmap
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 128]);
    bitmap
}

/** Every admitted operator has a known independent straight-alpha byte result. */
#[test]
fn all_painters_share_the_same_premultiplied_operators() {
    let source_color = [0, 0, 255, 128];
    let mut source = Bitmap::new(1, 1).unwrap();
    source
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&source_color);
    let mut path = Path::default();
    path.rect([0.0, 0.0, 1.0, 1.0], Affine::IDENTITY).unwrap();
    for (mode, expected) in [
        (Composite::SourceOver, [85, 0, 170, 192]),
        (Composite::DestinationOver, [170, 0, 85, 192]),
        (Composite::DestinationOut, [255, 0, 0, 64]),
        (Composite::SourceAtop, [127, 0, 128, 128]),
        (Composite::Xor, [128, 0, 128, 127]),
        (Composite::Copy, [0, 0, 255, 128]),
        (Composite::SourceIn, [0, 0, 255, 64]),
        (Composite::SourceOut, [0, 0, 255, 64]),
        (Composite::DestinationIn, [255, 0, 0, 64]),
        (Composite::DestinationAtop, [128, 0, 127, 128]),
        (Composite::Lighter, [128, 0, 128, 255]),
    ] {
        let mut pixel = [255, 0, 0, 128];
        composite(&mut pixel, source_color, 1.0, mode);
        assert_eq!(pixel, expected, "{mode:?}");
        let mut rectangle = backing();
        let plan = plan_rect([1, 1], [0.0, 0.0, 1.0, 1.0], Affine::IDENTITY)
            .unwrap()
            .with_composite(mode);
        fill_rect(&mut rectangle, &plan, source_color, 1.0).unwrap();
        assert_eq!(&rectangle.pixels[..], &expected, "rect {mode:?}");
        let mut polygon = backing();
        let plan = plan_fill([1, 1], &path, FillRule::Nonzero)
            .unwrap()
            .with_composite(mode);
        paint_path(&mut polygon, &plan, source_color, 1.0).unwrap();
        assert_eq!(&polygon.pixels[..], &expected, "path {mode:?}");
        let mut image = backing();
        let plan = plan_blit(
            [1, 1],
            [1, 1],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            Affine::IDENTITY,
            false,
        )
        .unwrap()
        .with_composite(mode);
        blit(&mut image, &source, &plan, 1.0).unwrap();
        assert_eq!(&image.pixels[..], &expected, "image {mode:?}");
    }
}

/** Geometry and clip samples intersect before source alpha is applied, avoiding double attenuation. */
#[test]
fn partial_clip_and_geometry_use_joint_coverage_for_destination_out() {
    let mut path = Path::default();
    path.rect([0.5, 0.0, 0.5, 1.0], Affine::IDENTITY).unwrap();
    let clip = plan_fill([1, 1], &path, FillRule::Nonzero)
        .unwrap()
        .clip_mask()
        .unwrap();
    let plan = plan_rect([1, 1], [0.5, 0.0, 0.5, 1.0], Affine::IDENTITY)
        .unwrap()
        .with_clip(Some(&clip))
        .unwrap()
        .with_composite(Composite::DestinationOut);
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    bitmap
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 255]);
    fill_rect(&mut bitmap, &plan, [0, 0, 255, 255], 1.0).unwrap();
    assert_eq!(&bitmap.pixels[..], &[255, 0, 0, 128]);
}

/** Every admitted operator preserves the destination when source alpha or effective coverage is zero. */
#[test]
fn zero_source_and_zero_coverage_are_no_ops() {
    for mode in [
        Composite::SourceOver,
        Composite::DestinationOver,
        Composite::DestinationOut,
        Composite::SourceAtop,
        Composite::Xor,
    ] {
        let mut pixel = [12, 34, 56, 0];
        composite(&mut pixel, [255, 255, 255, 0], 1.0, mode);
        assert_eq!(pixel, [12, 34, 56, 0]);
        composite(&mut pixel, [255; 4], 0.0, mode);
        assert_eq!(pixel, [12, 34, 56, 0]);
    }
}

/** Clear remains independent of compositing, and unsupported advanced blend operators are not admitted. */
#[test]
fn clear_bypasses_mode_and_unsupported_names_are_unaccepted() {
    let mut bitmap = backing();
    let plan = plan_rect([1, 1], [0.0, 0.0, 1.0, 1.0], Affine::IDENTITY)
        .unwrap()
        .with_composite(Composite::DestinationOver);
    clear_rect(&mut bitmap, &plan).unwrap();
    assert_eq!(&bitmap.pixels[..], &[0, 0, 0, 0]);
    for name in ["multiply", "screen", "SOURCE-OVER", "invalid"] {
        assert!(Composite::named(name).is_none());
    }
    assert!(serde_json::from_str::<Composite>("\"multiply\"").is_err());
}

/** Transparent source affects the entire clip even when there are no source edges or visible source bounds. */
#[test]
fn empty_and_offcanvas_sources_composite_across_the_clip() {
    let mut region = Path::default();
    region.rect([0.0, 0.0, 1.0, 1.0], Affine::IDENTITY).unwrap();
    let clip = plan_fill([2, 1], &region, FillRule::Nonzero)
        .unwrap()
        .clip_mask()
        .unwrap();
    for mode in [
        Composite::Copy,
        Composite::SourceIn,
        Composite::SourceOut,
        Composite::DestinationIn,
        Composite::DestinationAtop,
    ] {
        for image in [false, true] {
            let mut bitmap = Bitmap::new(2, 1).unwrap();
            bitmap
                .pixels
                .make_mut()
                .unwrap()
                .copy_from_slice(&[255, 0, 0, 255, 255, 0, 0, 255]);
            if image {
                let source = backing();
                let plan = plan_blit(
                    [1, 1],
                    [2, 1],
                    [0.0, 0.0, 1.0, 1.0],
                    [10.0, 10.0, 1.0, 1.0],
                    Affine::IDENTITY,
                    false,
                )
                .unwrap()
                .with_clip(Some(&clip))
                .unwrap()
                .with_composite(mode);
                assert!(!plan.is_empty());
                assert!(plan.work() >= 2 * mars_core::canvas::COMPOSITE_WORK);
                blit(&mut bitmap, &source, &plan, 1.0).unwrap();
            } else {
                let plan = plan_fill([2, 1], &Path::default(), FillRule::Nonzero)
                    .unwrap()
                    .with_clip(Some(&clip))
                    .unwrap()
                    .with_composite(mode);
                assert!(!plan.is_empty());
                paint_path(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
            }
            assert_eq!(
                &bitmap.pixels[..],
                &[0, 0, 0, 0, 255, 0, 0, 255],
                "{mode:?} image={image}"
            );
        }
    }
}

/** Saturating additive composition happens before clip interpolation, rather than clipping source alpha first. */
#[test]
fn lighter_clamps_before_fractional_clip_coverage() {
    let mut region = Path::default();
    region.rect([0.5, 0.0, 0.5, 1.0], Affine::IDENTITY).unwrap();
    let clip = plan_fill([1, 1], &region, FillRule::Nonzero)
        .unwrap()
        .clip_mask()
        .unwrap();
    let plan = plan_rect([1, 1], [0.0, 0.0, 1.0, 1.0], Affine::IDENTITY)
        .unwrap()
        .with_clip(Some(&clip))
        .unwrap()
        .with_composite(Composite::Lighter);
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    bitmap
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 204]);
    fill_rect(&mut bitmap, &plan, [255, 0, 0, 204], 1.0).unwrap();
    assert_eq!(&bitmap.pixels[..], &[255, 0, 0, 230]);
    let plan = plan_rect([1, 1], [10.0, 10.0, 1.0, 1.0], Affine::IDENTITY)
        .unwrap()
        .with_clip(Some(&clip))
        .unwrap()
        .with_composite(Composite::Copy);
    fill_rect(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    assert_eq!(&bitmap.pixels[..], &[255, 0, 0, 115]);
}

/** No source crop is an aborted image operation; a valid transparent bitmap remains a compositing source. */
#[test]
fn empty_source_crop_and_transparent_source_are_distinct() {
    let source = Bitmap::new(1, 1).unwrap();
    let mut bitmap = backing();
    let plan = plan_blit(
        [1, 1],
        [1, 1],
        [5.0, 5.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        Affine::IDENTITY,
        false,
    )
    .unwrap()
    .with_composite(Composite::Copy);
    assert!(plan.is_empty());
    blit(&mut bitmap, &source, &plan, 1.0).unwrap();
    assert_eq!(&bitmap.pixels[..], &[255, 0, 0, 128]);
    let plan = plan_blit(
        [1, 1],
        [1, 1],
        [0.0, 0.0, 1.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
        Affine::IDENTITY,
        false,
    )
    .unwrap()
    .with_composite(Composite::Copy);
    blit(&mut bitmap, &source, &plan, 1.0).unwrap();
    assert_eq!(&bitmap.pixels[..], &[0; 4]);
}
