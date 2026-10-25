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
    let svg = |attrs: &str| Picture::Vector(format!("<svg {attrs}></svg>").into()).natural();
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
    let bitmap = Bitmap::draw(markup, 16.0, 64.0, &mut budget(SHOT_DRAW_WORK)).unwrap();
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

/** A shadow cut to the canvas paints the same pixels as one blurred over its whole region: a
 * canvas far larger than the shadow keeps the uncut region, and its window must match. */
#[test]
fn clipped_shadow_blur_matches_the_whole_region() {
    let rect = [80.0, 30.0, 60.0, 40.0];
    let paint = |canvas: &mut Canvas| {
        let shape = |x: f64, y: f64| inside_rounded(x, y, rect, [[6.0; 2]; 4]);
        canvas.shadow(rect, None, 20.0, &shape, &|_, _| true, [255, 0, 0, 255]);
    };
    let mut small = Canvas::new(0.0, 0.0, 100.0, 100.0, WHITE);
    let mut large = Canvas::new(-200.0, -200.0, 500.0, 500.0, WHITE);
    paint(&mut small);
    paint(&mut large);
    for y in 0..100 {
        let row = |canvas: &Canvas, left: usize, top: usize| {
            let at = ((y + top) * canvas.width + left) * 3;
            canvas.pixels[at..at + 300].to_vec()
        };
        assert_eq!(row(&small, 0, 0), row(&large, 200, 200), "row {y}");
    }
}

/** The active edge table fills exactly the pixels a scan of every edge on every row does. */
#[test]
fn active_edge_fill_matches_a_full_scan() {
    let star: Vec<(f64, f64)> = (0..11)
        .map(|i| {
            let (angle, radius) = (f64::from(i) * 0.4 * std::f64::consts::PI, 30.0);
            (32.3 + radius * angle.cos(), 31.7 + radius * angle.sin())
        })
        .collect();
    let edges: Vec<[f64; 4]> = star
        .windows(2)
        .map(|p| [p[0].0, p[0].1, p[1].0, p[1].1])
        .collect();
    let blank = || Bitmap {
        width: 64,
        height: 64,
        rgba: vec![0; 64 * 64 * 4],
    };
    for even_odd in [false, true] {
        let mut fast = blank();
        fast.fill(&edges, even_odd, [9, 99, 199, 200], &mut usize::MAX.clone())
            .unwrap();
        let mut slow = blank();
        for row in 0..64 {
            let y = f64::from(row) + 0.5;
            let mut crossings: Vec<(f64, i32)> = edges
                .iter()
                .filter(|[_, y0, _, y1]| (*y0 <= y) != (*y1 <= y))
                .map(|[x0, y0, x1, y1]| {
                    (
                        x0 + (y - y0) * (x1 - x0) / (y1 - y0),
                        if y1 > y0 { 1 } else { -1 },
                    )
                })
                .collect();
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for pair in crossings.windows(2) {
                winding += pair[0].1;
                if (even_odd && winding % 2 != 0) || (!even_odd && winding != 0) {
                    slow.span(row, pair[0].0, pair[1].0, [9, 99, 199, 200]);
                }
            }
        }
        assert!(fast.rgba == slow.rgba, "even_odd {even_odd}");
    }
}

/** A drawing budget of `work` units with every edge allowed. */
fn budget(work: usize) -> DrawBudget {
    DrawBudget {
        work,
        edges: SHOT_DRAWN_EDGES,
    }
}

/** A drawing whose parse, pixels, edges, rows and painted pixels pass the budget paints
 * nothing instead of stalling; exactly enough paints it. */
#[test]
fn a_drawing_past_the_budget_paints_nothing() {
    let markup = r#"<svg width="8" height="8"><rect width="8" height="8" fill="lime"/></svg>"#;
    let paths = crate::document::VectorPath::from_image(markup, [0.0, 0.0, 64.0, 64.0]);
    // Parse, tile pixels, edges built, two sides' rows scanned, every pixel painted.
    let needed =
        markup.len() * PARSE_WEIGHT + 64 * 64 + pieces(&paths[0].commands) + 2 * 64 + 64 * 64;
    assert!(Bitmap::draw(markup, 64.0, 64.0, &mut budget(needed - 1)).is_none());
    assert!(Bitmap::draw(markup, 64.0, 64.0, &mut budget(needed)).is_some());
}

/** One drawing rasterizes at no more than DRAWN_SIZES sizes per screenshot; a size already
 * drawn comes from the cache, and a further size paints nothing. */
#[test]
fn a_drawing_is_drawn_at_a_bounded_number_of_sizes() {
    let markup: Rc<str> =
        r#"<svg width="8" height="8"><rect width="8" height="8" fill="lime"/></svg>"#.into();
    let mut canvas = Canvas::new(0.0, 0.0, 10.0, 10.0, WHITE);
    for size in 0..DRAWN_SIZES {
        assert!(canvas.drawing(&markup, 4.0 + size as f64, 4.0).is_some());
    }
    assert!(canvas.drawing(&markup, 4.0, 4.0).is_some());
    assert!(canvas.drawing(&markup, 100.0, 4.0).is_none());
}

/** A span that starts at or past the right edge paints nothing instead of panicking. */
#[test]
fn a_span_past_the_right_edge_is_empty() {
    let mut bitmap = Bitmap {
        width: 4,
        height: 1,
        rgba: vec![0; 16],
    };
    bitmap.span(0, 10.0, 12.0, [255, 0, 0, 255]);
    bitmap.span(0, 3.9, 3.2, [255, 0, 0, 255]);
    assert!(bitmap.rgba.iter().all(|&byte| byte == 0));
}
