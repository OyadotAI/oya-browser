/** Native GIF/WebP decoding must return real straight-alpha pixels with bounded first-frame semantics. */
use mars_eye::platform::decode_canvas_image;
const GIF: &[u8] = include_bytes!("fixtures/canvas-images/first-frame.gif");
const ALPHA: &[u8] = include_bytes!("fixtures/canvas-images/alpha.webp");
const ANIMATED: &[u8] = include_bytes!("fixtures/canvas-images/animated.webp");

#[cfg(target_os = "macos")]
#[test]
fn native_lossless_webp_preserves_rgba_channels_alpha_and_top_left_row_order() {
    let bitmap = decode_canvas_image(ALPHA, 2, 2).unwrap();
    assert_eq!((bitmap.width, bitmap.height), (2, 2));
    assert_eq!(
        &*bitmap.pixels,
        &[255, 0, 0, 128, 0, 255, 0, 255, 0, 0, 0, 0, 0, 0, 255, 64]
    );
}

#[cfg(target_os = "macos")]
#[test]
fn native_lossy_webp_decodes_real_color_instead_of_placeholder_pixels() {
    let bitmap =
        decode_canvas_image(include_bytes!("fixtures/canvas-images/lossy.webp"), 4, 4).unwrap();
    for pixel in bitmap.pixels.chunks_exact(4) {
        assert!(pixel[0] >= 240 && pixel[1] <= 15 && pixel[2] <= 15);
        assert_eq!(pixel[3], 255);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn animated_gif_returns_the_first_complete_frame_with_palette_transparency() {
    let bitmap = decode_canvas_image(GIF, 2, 2).unwrap();
    assert_eq!(
        &*bitmap.pixels,
        &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255]
    );
    assert_eq!(decode_canvas_image(GIF, 2, 2).unwrap(), bitmap);
}

#[test]
fn unsupported_animation_and_partial_first_frames_never_publish_synthetic_pixels() {
    assert!(decode_canvas_image(ANIMATED, 2, 2).is_err());
    let mut partial = GIF.to_vec();
    let descriptor = partial
        .windows(10)
        .position(|row| row[0] == 0x2c && row[1..9] == [0, 0, 0, 0, 2, 0, 2, 0])
        .unwrap();
    partial[descriptor + 1] = 1;
    assert!(decode_canvas_image(&partial, 2, 2).is_err());
}

#[test]
fn native_decode_refuses_truncation_oversize_and_mismatched_dimensions() {
    for (bytes, width, height) in [
        (GIF, 513, 1),
        (ALPHA, 1, 2),
        (ALPHA, 0, 2),
        (&GIF[..13], 2, 2),
        (&ALPHA[..20], 2, 2),
        (b"not an image".as_slice(), 2, 2),
    ] {
        assert!(decode_canvas_image(bytes, width, height).is_err());
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn other_platforms_explicitly_refuse_native_image_decoding() {
    assert!(decode_canvas_image(ALPHA, 2, 2).is_err());
    assert!(decode_canvas_image(GIF, 2, 2).is_err());
}
