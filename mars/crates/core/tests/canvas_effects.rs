/** Shadow layers and layer compositing: blur conserves coverage, offsets move it exactly, and operators apply to whole layers. */
use mars_core::canvas::{
    Bitmap, Composite, blur_plane, blur_widths, composite_layer, shadow_layer,
};

/** A canvas-sized layer with one pixel of the given straight color. */
fn dot(width: u32, height: u32, at: (u32, u32), color: [u8; 4]) -> Bitmap {
    let mut bitmap = Bitmap::new(width, height).unwrap();
    let index = (at.1 * width + at.0) as usize * 4;
    bitmap.pixels.make_mut().unwrap()[index..index + 4].copy_from_slice(&color);
    bitmap
}

/** Pixel values as straight RGBA at one position. */
fn pixel(bitmap: &Bitmap, (x, y): (u32, u32)) -> [u8; 4] {
    let index = (y * bitmap.width + x) as usize * 4;
    [0, 1, 2, 3].map(|i| bitmap.pixels[index + i])
}

/** Box widths are odd, and a zero sigma leaves the image unchanged. */
#[test]
fn box_widths_are_odd_and_zero_sigma_is_identity() {
    for sigma in [0.5, 1.0, 3.7, 12.0] {
        assert!(blur_widths(sigma).iter().all(|width| width % 2 == 1));
    }
    assert_eq!(blur_widths(0.0), [1, 1, 1]);
}

/** Blur spreads an impulse without losing its total coverage, and keeps it centered. */
#[test]
fn blur_conserves_total_coverage_and_stays_centered() {
    let mut values = vec![0.0f32; 21 * 21];
    values[10 * 21 + 10] = 1.0;
    blur_plane(&mut values, 21, 21, 2.0);
    let total: f32 = values.iter().sum();
    assert!((total - 1.0).abs() < 1e-4, "total {total}");
    assert!(values[10 * 21 + 10] > values[10 * 21 + 12]);
    assert!((values[10 * 21 + 9] - values[10 * 21 + 11]).abs() < 1e-6);
}

/** A hard shadow is the layer's coverage moved by the offset, colored with the shadow color. */
#[test]
fn offset_shadow_moves_coverage_exactly_by_whole_pixels() {
    let layer = dot(8, 8, (2, 3), [255, 0, 0, 255]);
    let shadow = shadow_layer(&layer, [0, 0, 255, 255], 0.0, [2.0, 0.0]).unwrap();
    assert_eq!(pixel(&shadow, (4, 3)), [0, 0, 255, 255]);
    assert_eq!(pixel(&shadow, (2, 3))[3], 0);
}

/** Shadow alpha is the color alpha times the blurred coverage, so a half-alpha shadow stays half alpha. */
#[test]
fn shadow_alpha_scales_with_the_shadow_color() {
    let layer = dot(8, 8, (4, 4), [0, 0, 0, 255]);
    let shadow = shadow_layer(&layer, [0, 0, 0, 128], 0.0, [0.0, 0.0]).unwrap();
    assert_eq!(pixel(&shadow, (4, 4))[3], 128);
}

/** Copy clears everything outside the layer's coverage, while source-over leaves the destination alone. */
#[test]
fn copy_operator_clears_outside_the_layer_and_source_over_does_not() {
    let blue = dot(4, 4, (0, 0), [0, 0, 255, 255]);
    let mut over = Bitmap::new(4, 4).unwrap();
    over.pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&blue.pixels);
    let layer = dot(4, 4, (1, 1), [255, 0, 0, 255]);
    composite_layer(&mut over, &layer, None, 1.0, Composite::SourceOver).unwrap();
    assert_eq!(pixel(&over, (1, 1)), [255, 0, 0, 255]);
    assert_eq!(pixel(&over, (3, 3)), [0, 0, 0, 0]);

    let mut copied = Bitmap::new(4, 4).unwrap();
    copied
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&blue.pixels);
    composite_layer(&mut copied, &layer, None, 1.0, Composite::Copy).unwrap();
    assert_eq!(pixel(&copied, (1, 1)), [255, 0, 0, 255]);
    assert_eq!(pixel(&copied, (0, 0)), [0, 0, 0, 0]);
}

/** A layer of another size is a refused contract, not a silent misplaced composite. */
#[test]
fn mismatched_layer_size_is_refused() {
    let mut target = Bitmap::new(4, 4).unwrap();
    let layer = Bitmap::new(2, 2).unwrap();
    assert!(composite_layer(&mut target, &layer, None, 1.0, Composite::SourceOver).is_err());
}
