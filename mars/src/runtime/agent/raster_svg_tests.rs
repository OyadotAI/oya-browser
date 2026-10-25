/** Regression tests for SVG background images: CSS Images' default sizing and the fill rasterizer. */
use super::*;

/** A natural-size bitmap keeps its size under `auto`. */
#[test]
fn auto_keeps_the_natural_size() {
    let natural = (Some(8.0), Some(32.0), Some(0.25));
    assert_eq!(
        concrete_size(ImageSize::Auto, [100.0, 100.0], natural),
        (8.0, 32.0)
    );
}

/** One given length scales the other by the natural ratio. */
#[test]
fn one_length_follows_the_ratio() {
    let natural = (Some(8.0), Some(32.0), Some(0.25));
    let size = ImageSize::Fixed {
        width: Some(16.0),
        height: None,
    };
    assert_eq!(concrete_size(size, [128.0, 384.0], natural), (16.0, 64.0));
}

/** Cover fills the area along the ratio; contain fits inside it. */
#[test]
fn cover_and_contain_scale_the_ratio() {
    let natural = (None, Some(8.0), Some(0.5));
    assert_eq!(
        concrete_size(ImageSize::Cover, [100.0, 50.0], natural),
        (100.0, 200.0)
    );
    assert_eq!(
        concrete_size(ImageSize::Contain, [100.0, 50.0], natural),
        (25.0, 50.0)
    );
}

/** An image with only a width and no ratio takes the area's height. */
#[test]
fn a_missing_dimension_without_ratio_takes_the_area() {
    assert_eq!(
        concrete_size(ImageSize::Auto, [40.0, 30.0], (Some(10.0), None, None)),
        (10.0, 30.0)
    );
    assert_eq!(
        concrete_size(ImageSize::Auto, [40.0, 30.0], (None, None, None)),
        (40.0, 30.0)
    );
}

/** The root's absolute width and height are natural; percentages are not, and the viewBox
 * gives the ratio. */
#[test]
fn svg_natural_size_reads_the_root() {
    let svg = |attrs: &str| Picture::Vector(format!("<svg {attrs}></svg>")).natural();
    assert_eq!(
        svg(r#"width="8px" height="32px" viewBox="0 0 4 64""#),
        (Some(8.0), Some(32.0), Some(0.25))
    );
    assert_eq!(
        svg(r#"width="100%" height="8" viewBox="0 0 1 2""#),
        (None, Some(8.0), Some(0.5))
    );
}

/** A drawing paints its fills into the tile, stretched by `preserveAspectRatio="none"`. */
#[test]
fn drawing_fills_the_tile() {
    let markup = r#"<svg width="8" height="32" viewBox="0 0 4 64" preserveAspectRatio="none"><rect y="0" width="100%" height="50%" fill="lime"/><rect y="50%" width="100%" height="50%" fill="aqua"/></svg>"#;
    let bitmap = Bitmap::draw(markup, 16.0, 64.0);
    assert_eq!((bitmap.width, bitmap.height), (16, 64));
    assert_eq!(bitmap.sample(0.5, 0.25), [0, 255, 0, 255]);
    assert_eq!(bitmap.sample(0.5, 0.75), [0, 255, 255, 255]);
}

/** A blurred shadow edge is half covered, its middle fully, and far outside nothing. */
#[test]
fn shadow_blur_is_gaussian() {
    let region = [0.0, 0.0, 40.0, 40.0];
    let coverage = blurred(region, 2.0, &|x, _| x >= 20.0);
    let row = 20 * 40;
    assert!(
        (coverage[row + 20] - 0.6).abs() < 0.15,
        "{}",
        coverage[row + 20]
    );
    assert!(coverage[row + 30] > 0.99);
    assert!(coverage[row + 5] < 0.01);
    let paths = crate::document::VectorPath::from_image(
        r#"<svg width="8" height="32"><rect width="8" height="16" fill="lime"/></svg>"#,
        [0.0, 0.0, 8.0, 32.0],
    );
    assert_eq!(paths.len(), 1, "{paths:?}");
}

/** `space` fits whole tiles from the area's edge and spreads the leftover between them. */
#[test]
fn space_spreads_whole_tiles() {
    let tiling = Tiling::place(
        [10.0, 0.0, 100.0, 100.0],
        (30.0, 30.0),
        [0.0, 50.0, 0.0, 50.0],
        [true, true],
        [true, false],
    );
    assert_eq!(tiling.tile, [10.0, 35.0, 30.0, 30.0]);
    assert_eq!(tiling.gap, [5.0, 0.0]);
    assert_eq!(tiling.repeat, [true, true]);
}

/** A `space` axis that fits only one tile paints it once at its position. */
#[test]
fn space_with_one_tile_does_not_repeat() {
    let tiling = Tiling::place(
        [0.0, 0.0, 50.0, 50.0],
        (30.0, 30.0),
        [0.0, 50.0, 0.0, 0.0],
        [true, true],
        [true, true],
    );
    assert_eq!(tiling.repeat, [false, false]);
    assert_eq!(tiling.tile[0], 10.0);
}

/** `round` rescales to a whole number of tiles, and an auto other axis keeps the ratio. */
#[test]
fn round_fits_whole_tiles() {
    assert_eq!(
        rounded([100.0, 100.0], (30.0, 15.0), [true, false], [true, true]),
        (100.0 / 3.0, 15.0 * (100.0 / 3.0) / 30.0)
    );
    assert_eq!(
        rounded([100.0, 100.0], (30.0, 40.0), [true, true], [true, true]),
        (100.0 / 3.0, 100.0 / 3.0)
    );
}
