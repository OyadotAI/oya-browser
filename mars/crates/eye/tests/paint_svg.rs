/** SVG painting ported from Blink: `preserveAspectRatio` (SVGPreserveAspectRatio::ComputeTransform),
 * `object-fit` on SVG images (LayoutReplaced::ComputeReplacedContentRect), replaced-content
 * clipping and paint servers. Expectations follow the SVG 2 and CSS Images specs. */
use mars_eye::{DisplayItem, DisplayList, Document, PathCommand, compute_styles, parse, render};

/** Styles and lays out markup with one stylesheet. */
fn layout(markup: &str, css: &str) -> (Document, DisplayList) {
    let document = parse(markup).unwrap();
    let sheets = if css.is_empty() {
        vec![]
    } else {
        vec![css.to_owned()]
    };
    let document = compute_styles(&document, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first item painting vectors. */
fn drawing(display: &DisplayList) -> &DisplayItem {
    display
        .items
        .iter()
        .find(|item| !item.vector.is_empty())
        .expect("a drawing painted")
}
/** The first point of the first path, relative to the item's box. */
fn first_point(item: &DisplayItem) -> (f64, f64) {
    match item.vector[0].commands[0] {
        PathCommand::Move { x, y } => (x - item.x, y - item.y),
        other => panic!("expected a move, got {other:?}"),
    }
}
/** A 10x10 viewBox square from the origin, in a 100x50 svg with the given aspect rule. */
fn square(aspect: &str) -> String {
    format!(
        "<svg width=100 height=50 viewBox='0 0 10 10' preserveAspectRatio='{aspect}'>\
         <path d='M0 0H10V10H0Z' fill='red'></path></svg>"
    )
}

/** The initial `xMidYMid meet` centers the uniformly scaled box. */
#[test]
fn default_aspect_ratio_centers_and_meets() {
    let (_, display) = layout(&square("xMidYMid meet"), "");
    assert_eq!(first_point(drawing(&display)), (25.0, 0.0));
}

/** `xMinYMin` aligns the scaled box to the left edge. */
#[test]
fn min_alignment_pins_the_left_edge() {
    let (_, display) = layout(&square("xMinYMin meet"), "");
    assert_eq!(first_point(drawing(&display)), (0.0, 0.0));
}

/** `xMaxYMax` aligns it to the right edge. */
#[test]
fn max_alignment_pins_the_right_edge() {
    let (_, display) = layout(&square("xMaxYMax"), "");
    assert_eq!(first_point(drawing(&display)), (50.0, 0.0));
}

/** `none` stretches each axis independently: the square fills 100x50. */
#[test]
fn none_stretches_both_axes() {
    let (_, display) = layout(&square("none"), "");
    let item = drawing(&display);
    let PathCommand::Line { x, .. } = item.vector[0].commands[1] else {
        panic!("expected a line")
    };
    assert_eq!(x - item.x, 100.0);
    let PathCommand::Line { y, .. } = item.vector[0].commands[2] else {
        panic!("expected a line")
    };
    assert_eq!(y - item.y, 50.0);
}

/** `slice` scales to cover, overflowing vertically and centered, and the svg clips to its box. */
#[test]
fn slice_covers_and_clips_to_the_viewport() {
    let (_, display) = layout(&square("xMidYMid slice"), "");
    let item = drawing(&display);
    assert_eq!(first_point(item), (0.0, -25.0));
    let clip = item.clip.as_ref().expect("svg clips its content");
    assert_eq!((clip.width, clip.height), (100.0, 50.0));
}

/** `overflow="visible"` opts an inline svg out of the clip, as GitHub's octicons do. */
#[test]
fn overflow_visible_svg_is_not_clipped() {
    let markup = "<svg width=10 height=10 viewBox='0 0 10 10' overflow=visible><path d='M0 0H10V10Z'></path></svg>";
    let (_, display) = layout(markup, "");
    assert!(drawing(&display).clip.is_none());
}

/** A symbol's own `preserveAspectRatio` places it inside the `use` box. */
#[test]
fn symbol_aspect_ratio_applies_to_use() {
    let markup = "<svg width=100 height=50 viewBox='0 0 100 50'>\
        <symbol id=s viewBox='0 0 10 10' preserveAspectRatio='xMinYMid meet'><path d='M0 0H10V10Z'></path></symbol>\
        <use href='#s' width=100 height=50></use></svg>";
    let (_, display) = layout(markup, "");
    assert_eq!(first_point(drawing(&display)), (0.0, 0.0));
}

/** An img of an SVG drawing with `object-fit: cover; object-position: 100% 0` is scaled to
 * cover its box and anchored to the top right, as go.dev's ladder gopher is, and clipped. */
#[test]
fn svg_image_object_fit_cover_scales_and_anchors() {
    let mut document = parse(
        "<img alt=g style='width:100px;height:100px;object-fit:cover;object-position:100% 0'>",
    )
    .unwrap();
    let img = document.nodes.iter_mut().find(|n| n.tag == "img").unwrap();
    img.image_src = Some("data:image/svg+xml,<svg width=\"20\" height=\"40\" viewBox=\"0 0 20 40\"><path d=\"M0 0H20V40H0Z\"></path></svg>".into());
    img.image_size = Some((20, 40));
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let item = drawing(&display);
    // Cover scales 20x40 by 5 to 100x200; anchored at the top, so the path starts at the box top.
    assert_eq!(first_point(item), (0.0, 0.0));
    let PathCommand::Line { y, .. } = item.vector[0].commands[2] else {
        panic!("expected a line")
    };
    assert_eq!(y - item.y, 200.0);
    let clip = item.clip.as_ref().expect("cover clips to the content box");
    assert_eq!(clip.height, 100.0);
}

/** `object-fit: contain` (and the default meet) keeps the whole drawing inside the box. */
#[test]
fn svg_image_object_fit_contain_fits_inside() {
    let mut document =
        parse("<img alt=g style='width:100px;height:100px;object-fit:contain'>").unwrap();
    let img = document.nodes.iter_mut().find(|n| n.tag == "img").unwrap();
    img.image_src = Some("data:image/svg+xml,<svg width=\"20\" height=\"40\" viewBox=\"0 0 20 40\"><path d=\"M0 0H20V40H0Z\"></path></svg>".into());
    img.image_size = Some((20, 40));
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    assert_eq!(first_point(drawing(&display)), (25.0, 0.0));
}

/**
 * The fill of the first path of an inline drawing. Paint servers are written as `<g>` with a
 * `data-tag` and renamed after parsing, standing for the tree once the parser admits
 * `linearGradient` and `stop`, so the painter is tested on its own.
 */
fn fill(markup: &str) -> Option<String> {
    let mut document = parse(markup).unwrap();
    for node in &mut document.nodes {
        if let Some(tag) = node.attributes.get("data-tag").cloned() {
            node.tag = tag;
        }
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .flat_map(|item| &item.vector)
        .next()
        .and_then(|path| path.fill.clone())
}

/** A `url(#gradient)` fill paints instead of dropping the shape (Node.js's logo lost its
 * hexagon); a two-stop ramp paints its mean color. */
#[test]
fn gradient_fill_paints_its_ramp() {
    let markup = "<svg width=10 height=10><defs><g data-tag=lineargradient id=g>\
        <g data-tag=stop offset=0 stop-color='#ff0000'></g><g data-tag=stop offset=1 stop-color='#0000ff'></g>\
        </g></defs><path fill='url(#g)' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(markup).as_deref(), Some("#800080"));
}

/** A gradient with no stops of its own inherits them through `href`. */
#[test]
fn gradient_inherits_stops_through_href() {
    let markup = "<svg width=10 height=10><defs><g data-tag=lineargradient id=base>\
        <g data-tag=stop offset=0 stop-color='#00ff00'></g></g>\
        <g data-tag=lineargradient id=g href='#base' x1=0 x2=1></g></defs>\
        <path fill='url(#g)' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(markup).as_deref(), Some("#00ff00"));
}

/** A missing paint server paints the fallback color; without one the shape paints nothing. */
#[test]
fn missing_paint_server_uses_the_fallback() {
    let with =
        "<svg width=10 height=10><path fill='url(#nope) #123456' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(with).as_deref(), Some("#123456"));
    let without = "<svg width=10 height=10><path fill='url(#nope)' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(without), None);
}

/** Stop opacity weighs a stop's color, as premultiplied interpolation does. */
#[test]
fn transparent_stops_do_not_tint_the_ramp() {
    let markup = "<svg width=10 height=10><g data-tag=lineargradient id=g>\
        <g data-tag=stop offset=0 stop-color='#ff0000'></g><g data-tag=stop offset=1 stop-color='#0000ff' stop-opacity=0></g>\
        </g><path fill='url(#g)' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(markup).as_deref(), Some("#ff000080"));
}

/** The stroke opacity of the first path of an inline drawing. */
fn stroke_opacity(markup: &str) -> f64 {
    let (_, display) = layout(markup, "");
    drawing(&display).vector[0].stroke_opacity
}

/** A `mask` scales what shows by its content's coverage (SVG Masking 1, mask-type): a white
 * luminance mask keeps everything, an alpha mask fading out from 10% keeps a few percent, and an
 * empty mask hides the element (Node.js's hexagon backdrop painted at full strength). */
#[test]
fn mask_coverage_scales_masked_content() {
    let masked = |mask: &str| {
        format!(
            "<svg width=10 height=10><defs>{mask}<radialGradient id=g><stop stop-color='#d9d9d9' \
             stop-opacity=0.1></stop><stop offset=0.8 stop-color='#d9d9d9' stop-opacity=0></stop>\
             </radialGradient></defs><g mask='url(#m)'><path stroke='#000' d='M0 0H10'></path></g></svg>"
        )
    };
    let white = masked("<mask id=m><rect width=10 height=10 fill='#fff'></rect></mask>");
    assert!((stroke_opacity(&white) - 1.0).abs() < 1e-9);
    let fading = masked(
        "<mask id=m style='mask-type:alpha'><rect width=10 height=10 fill='url(#g)'></rect></mask>",
    );
    let fading = stroke_opacity(&fading);
    assert!(fading > 0.0 && fading < 0.1, "{fading}");
    let empty = masked("<mask id=m></mask>");
    assert_eq!(stroke_opacity(&empty), 0.0);
}
