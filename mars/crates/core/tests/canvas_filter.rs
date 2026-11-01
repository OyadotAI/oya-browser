/** CSS filter lists: parsing is strict about syntax, url() is ignored, and each function's matrix matches the Filter Effects formulas. */
use mars_core::canvas::{Bitmap, FilterStep, apply_filter, parse_filter};

/** Apply a filter string to a one-pixel bitmap of the given color and read the result back. */
fn filtered(text: &str, color: [u8; 4]) -> [u8; 4] {
    let filter = parse_filter(text).expect("valid filter");
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    bitmap.pixels.make_mut().unwrap().copy_from_slice(&color);
    apply_filter(&mut bitmap, &filter.steps).unwrap();
    [0, 1, 2, 3].map(|i| bitmap.pixels[i])
}

/** Canonical text reports the parsed functions, and none is the empty list. */
#[test]
fn canonical_text_lists_parsed_functions_in_order() {
    assert_eq!(
        parse_filter("blur(4px) grayscale(50%)").unwrap().text,
        "blur(4px) grayscale(0.5)"
    );
    assert_eq!(parse_filter("none").unwrap().text, "none");
    assert_eq!(parse_filter("url(#f)").unwrap().text, "none");
}

/** Syntax errors, unknown functions and bad arguments are invalid, so the previous filter stays. */
#[test]
fn malformed_and_unknown_filters_are_invalid() {
    for text in [
        "",
        "blur(4)",
        "blur(-1px)",
        "bogus(1)",
        "invert(",
        "saturate(-1)",
        "blur(4px) none",
    ] {
        assert!(parse_filter(text).is_none(), "accepted {text:?}");
    }
}

/** A url() reference is dropped, so a filter made only of references behaves as none. */
#[test]
fn url_references_are_ignored() {
    assert!(parse_filter("url(#a) url(#b)").unwrap().steps.is_empty());
}

/** Grayscale at full amount maps red to the Rec. 709 luma on every channel. */
#[test]
fn grayscale_maps_red_to_its_luma() {
    assert_eq!(
        filtered("grayscale(1)", [255, 0, 0, 255]),
        [54, 54, 54, 255]
    );
}

/** Invert at full amount flips every color channel and leaves alpha alone. */
#[test]
fn invert_flips_color_channels_and_keeps_alpha() {
    assert_eq!(
        filtered("invert(1)", [0, 128, 255, 255]),
        [255, 127, 0, 255]
    );
}

/** Opacity scales alpha, and brightness scales color with clipping at full scale. */
#[test]
fn opacity_scales_alpha_and_brightness_clips_color() {
    assert_eq!(
        filtered("opacity(0.5)", [10, 20, 30, 255]),
        [10, 20, 30, 128]
    );
    assert_eq!(
        filtered("brightness(2)", [100, 200, 30, 255]),
        [200, 255, 60, 255]
    );
}

/** Contrast stretches around middle gray, and a zero hue rotation is the identity. */
#[test]
fn contrast_stretches_about_middle_gray_and_zero_rotation_is_identity() {
    assert_eq!(
        filtered("contrast(2)", [64, 128, 192, 255]),
        [0, 129, 255, 255]
    );
    assert_eq!(
        filtered("hue-rotate(0deg)", [40, 90, 200, 255]),
        [40, 90, 200, 255]
    );
}

/** Blur spreads a single opaque pixel's coverage while the center keeps its premultiplied color. */
#[test]
fn blur_spreads_coverage_without_darkening_color() {
    let filter = parse_filter("blur(2px)").unwrap();
    assert!(matches!(filter.steps[0], FilterStep::Blur(px) if px == 2.0));
    let mut bitmap = Bitmap::new(9, 9).unwrap();
    let center = (4 * 9 + 4) * 4;
    bitmap.pixels.make_mut().unwrap()[center..center + 4].copy_from_slice(&[200, 0, 0, 255]);
    apply_filter(&mut bitmap, &filter.steps).unwrap();
    let off = (4 * 9 + 6) * 4;
    assert!(bitmap.pixels[off + 3] > 0 && bitmap.pixels[off + 3] < 255);
    assert_eq!(bitmap.pixels[center], 200);
}
