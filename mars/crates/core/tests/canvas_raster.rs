/** Pixel contracts cover real transform geometry, alpha, cropping and transactional snapshots. */
use mars_core::canvas::{Affine, Bitmap, blit, clear_rect, fill_rect, plan_blit, plan_rect};

/** Inspect canonical pixels without depending on private raster helpers. */
fn pixel(bitmap: &Bitmap, x: usize, y: usize) -> [u8; 4] {
    let at = (y * bitmap.width as usize + x) * 4;
    bitmap.pixels[at..at + 4].try_into().unwrap()
}

/** Draw through the same checked plan that V9 charges before committing its pixels. */
fn fill(bitmap: &mut Bitmap, rect: [f64; 4], transform: Affine, color: [u8; 4], alpha: f64) {
    let plan = plan_rect([bitmap.width, bitmap.height], rect, transform).unwrap();
    fill_rect(bitmap, &plan, color, alpha).unwrap();
}

#[test]
fn transforms_postmultiply_and_inverse_maps_back() {
    let scale = Affine([2.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
    let translated = scale.compose(Affine([1.0, 0.0, 0.0, 1.0, 4.0, 5.0]));
    assert_eq!(translated.map([1.0, 1.0]), [10.0, 18.0]);
    assert_eq!(translated.inverse().unwrap().map([10.0, 18.0]), [1.0, 1.0]);
}

#[test]
fn very_large_finite_scale_has_a_representable_inverse() {
    let matrix = Affine([1.0e200, 0.0, 0.0, 1.0e200, 0.0, 0.0]);
    let inverse = matrix.inverse().unwrap();
    assert_eq!(inverse.map([1.0e200, 1.0e200]), [1.0, 1.0]);
    let anisotropic = Affine([1.0e200, 0.0, 0.0, 1.0e-200, 0.0, 0.0]);
    assert_eq!(
        anisotropic.inverse().unwrap().map([1.0e200, 1.0e-200]),
        [1.0, 1.0]
    );
}

#[test]
fn copy_on_write_allocation_refusal_preserves_both_snapshots() {
    const CHILD_FLAG: &str = "MARS_CANVAS_COW_FIXTURE";
    const PROCESS_BYTES: usize = 96 * 1024 * 1024;
    if std::env::var_os(CHILD_FLAG).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "copy_on_write_allocation_refusal_preserves_both_snapshots",
            ])
            .env(CHILD_FLAG, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        return;
    }
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    let snapshot = bitmap.clone();
    let reservation = mars_core::canvas::Allocation::new(PROCESS_BYTES - 4).unwrap();
    let plan = plan_rect([1, 1], [0.0, 0.0, 1.0, 1.0], Affine::IDENTITY).unwrap();
    assert!(fill_rect(&mut bitmap, &plan, [255; 4], 1.0).is_err());
    assert_eq!(bitmap, snapshot);
    drop(reservation);
    fill_rect(&mut bitmap, &plan, [255; 4], 1.0).unwrap();
    assert_eq!(pixel(&snapshot, 0, 0), [0; 4]);
    assert_eq!(pixel(&bitmap, 0, 0), [255; 4]);
}

#[test]
fn fractional_rectangles_keep_exact_coverage_under_translation() {
    let mut bitmap = Bitmap::new(3, 2).unwrap();
    fill(
        &mut bitmap,
        [0.0, 0.0, 1.0, 1.0],
        Affine([1.0, 0.0, 0.0, 1.0, 0.5, 0.5]),
        [255, 0, 0, 255],
        1.0,
    );
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        assert_eq!(pixel(&bitmap, x, y), [255, 0, 0, 64]);
    }
    assert_eq!(pixel(&bitmap, 2, 0), [0; 4]);
}

#[test]
fn rotated_rectangles_clip_as_polygons_instead_of_bounding_boxes() {
    let mut bitmap = Bitmap::new(4, 4).unwrap();
    fill(
        &mut bitmap,
        [0.0, 0.0, 1.0, 1.0],
        Affine([1.0, 1.0, -1.0, 1.0, 2.0, 0.0]),
        [0, 255, 0, 255],
        1.0,
    );
    assert_eq!(pixel(&bitmap, 1, 0), [0, 255, 0, 128]);
    assert_eq!(pixel(&bitmap, 2, 0), [0, 255, 0, 128]);
    assert_eq!(pixel(&bitmap, 1, 1), [0, 255, 0, 128]);
    assert_eq!(pixel(&bitmap, 2, 1), [0, 255, 0, 128]);
    assert_eq!(pixel(&bitmap, 0, 0), [0; 4]);
}

#[test]
fn alpha_blends_and_clear_ignores_paint_alpha() {
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    fill(
        &mut bitmap,
        [0.0, 0.0, 1.0, 1.0],
        Affine::IDENTITY,
        [0, 0, 255, 255],
        1.0,
    );
    fill(
        &mut bitmap,
        [0.0, 0.0, 1.0, 1.0],
        Affine::IDENTITY,
        [255, 0, 0, 255],
        0.5,
    );
    assert_eq!(pixel(&bitmap, 0, 0), [128, 0, 128, 255]);
    let plan = plan_rect([1, 1], [0.0, 0.0, 0.5, 1.0], Affine::IDENTITY).unwrap();
    clear_rect(&mut bitmap, &plan).unwrap();
    assert_eq!(pixel(&bitmap, 0, 0), [128, 0, 128, 128]);
}

#[test]
fn singular_and_outside_geometry_are_empty_without_work() {
    for (rect, matrix) in [
        ([0.0, 0.0, 2.0, 2.0], Affine([1.0, 0.0, 0.0, 0.0, 0.0, 0.0])),
        ([20.0, 20.0, 2.0, 2.0], Affine::IDENTITY),
    ] {
        let plan = plan_rect([2, 2], rect, matrix).unwrap();
        assert!(plan.is_empty());
        assert_eq!(plan.work(), 0);
    }
}

#[test]
fn clipped_source_crop_moves_the_corresponding_destination_edge() {
    let mut source = Bitmap::new(2, 1).unwrap();
    fill(
        &mut source,
        [0.0, 0.0, 2.0, 1.0],
        Affine::IDENTITY,
        [255, 0, 0, 255],
        1.0,
    );
    let mut destination = Bitmap::new(4, 1).unwrap();
    let plan = plan_blit(
        [2, 1],
        [4, 1],
        [-1.0, 0.0, 2.0, 1.0],
        [0.0, 0.0, 4.0, 1.0],
        Affine::IDENTITY,
        false,
    )
    .unwrap();
    blit(&mut destination, &source, &plan, 1.0).unwrap();
    assert_eq!(pixel(&destination, 0, 0), [0; 4]);
    assert_eq!(pixel(&destination, 1, 0), [0; 4]);
    assert_eq!(pixel(&destination, 2, 0), [255, 0, 0, 255]);
    assert_eq!(pixel(&destination, 3, 0), [255, 0, 0, 255]);
}

#[test]
fn negative_image_extents_do_not_flip_the_source() {
    let mut source = Bitmap::new(2, 1).unwrap();
    source
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 255, 0, 0, 255, 255]);
    let mut destination = Bitmap::new(2, 1).unwrap();
    let plan = plan_blit(
        [2, 1],
        [2, 1],
        [2.0, 0.0, -2.0, 1.0],
        [2.0, 0.0, -2.0, 1.0],
        Affine::IDENTITY,
        false,
    )
    .unwrap();
    blit(&mut destination, &source, &plan, 1.0).unwrap();
    assert_eq!(destination.pixels, source.pixels);
}

#[test]
fn smoothing_interpolates_premultiplied_color_without_transparent_halos() {
    let mut source = Bitmap::new(2, 1).unwrap();
    source
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 255, 0, 0, 255, 0]);
    let mut destination = Bitmap::new(3, 1).unwrap();
    let plan = plan_blit(
        [2, 1],
        [3, 1],
        [0.0, 0.0, 2.0, 1.0],
        [0.0, 0.0, 3.0, 1.0],
        Affine::IDENTITY,
        true,
    )
    .unwrap();
    blit(&mut destination, &source, &plan, 1.0).unwrap();
    assert_eq!(pixel(&destination, 1, 0), [255, 0, 0, 128]);
}

#[test]
fn self_copy_reads_the_pre_draw_snapshot() {
    let mut bitmap = Bitmap::new(4, 1).unwrap();
    bitmap
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255]);
    let source = bitmap.clone();
    let plan = plan_blit(
        [4, 1],
        [4, 1],
        [0.0, 0.0, 3.0, 1.0],
        [1.0, 0.0, 3.0, 1.0],
        Affine::IDENTITY,
        false,
    )
    .unwrap();
    blit(&mut bitmap, &source, &plan, 1.0).unwrap();
    assert_eq!(
        (0..4).map(|x| pixel(&bitmap, x, 0)[0]).collect::<Vec<_>>(),
        [1, 1, 2, 3]
    );
    assert_eq!(
        (0..4).map(|x| pixel(&source, x, 0)[0]).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
}

#[test]
fn invalid_state_or_stale_dimensions_leave_destination_unchanged() {
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    let snapshot = bitmap.clone();
    let stale = plan_rect([2, 2], [0.0, 0.0, 2.0, 2.0], Affine::IDENTITY).unwrap();
    assert!(fill_rect(&mut bitmap, &stale, [255; 4], 1.0).is_err());
    let plan = plan_rect([1, 1], [0.0, 0.0, 1.0, 1.0], Affine::IDENTITY).unwrap();
    assert!(fill_rect(&mut bitmap, &plan, [255; 4], f64::NAN).is_err());
    assert_eq!(bitmap, snapshot);
}

#[test]
fn coverage_and_sampling_are_charged_before_execution() {
    let plain = plan_rect([512, 512], [0.0, 0.0, 512.0, 512.0], Affine::IDENTITY).unwrap();
    let image = plan_blit(
        [512, 512],
        [512, 512],
        [0.0, 0.0, 512.0, 512.0],
        [0.0, 0.0, 512.0, 512.0],
        Affine::IDENTITY,
        true,
    )
    .unwrap();
    assert!(plain.work() > 512 * 512);
    assert!(image.work() > plain.work());
    assert!(image.work() <= mars_core::canvas::WORK);
}
