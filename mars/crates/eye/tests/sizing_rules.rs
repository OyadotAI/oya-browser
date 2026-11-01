/** css-sizing rules: content keywords on the block axis and on replaced boxes, size containment
 * with `contain-intrinsic-size`, `aspect-ratio` size transfers, svg natural sizing, `stretch`
 * heights and the quirks-mode percentage height chain. */
use mars_eye::{DisplayList, Document, compute_styles, parse, render};

/** Lays `markup` out at 800px; every img and canvas gets the natural size `size`. */
fn layout(markup: &str, size: (u32, u32)) -> (Document, DisplayList) {
    let mut document = parse(markup).unwrap();
    for node in document.nodes.iter_mut() {
        if node.tag == "img" || node.tag == "canvas" {
            node.image_src = Some("x.png".into());
            node.image_size = Some(size);
        }
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The (width, height) of the element with `id`. */
fn size(markup: &str, id: &str) -> (f64, f64) {
    let (document, display) = layout(markup, (10, 10));
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.width, item.height)
}

/** `min-height: max-content` grows a box past its `height` to wrap its content. */
#[test]
fn min_height_content_keyword_wraps_the_content() {
    let r = size(
        "<div id=t style='width:100px;min-height:max-content;height:0'><div style='height:100px'></div></div>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** `max-height: max-content` caps a box at its content height. */
#[test]
fn max_height_content_keyword_caps_at_the_content() {
    let r = size(
        "<div id=t style='width:100px;max-height:max-content;height:200px'><div style='height:100px'></div></div>",
        "t",
    );
    assert_eq!(r.1, 100.0);
}

/** A content-keyword block size keeps an absolutely positioned box from stretching. */
#[test]
fn abspos_content_block_size_does_not_stretch() {
    let r = size(
        "<div id=t style='position:absolute;inset:0;height:min-content;width:100px'><div style='height:200px'></div></div>",
        "t",
    );
    assert_eq!(r.1, 200.0);
}

/** A replaced box's `min-width: min-content` is the width an auto width would take. */
#[test]
fn replaced_min_width_keyword_is_the_auto_width() {
    let r = size(
        "<canvas id=t width=10 height=10 style='width:10px;height:100px;min-width:min-content'></canvas>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** A replaced box's `max-height: min-content` is the height an auto height would take. */
#[test]
fn replaced_max_height_keyword_is_the_auto_height() {
    let r = size(
        "<canvas id=t width=10 height=10 style='width:100px;height:1000px;max-height:min-content'></canvas>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** `contain: size` sizes a box as if empty, holding one child of `contain-intrinsic-size`. */
#[test]
fn size_containment_uses_the_intrinsic_size_override() {
    let r = size(
        "<div id=t style='float:left;contain:size;contain-intrinsic-size:111px 222px'><div style='width:500px;height:500px'></div></div>",
        "t",
    );
    assert_eq!(r, (111.0, 222.0));
}

/** `contain: inline-size` contains the width only; the height still wraps the content. */
#[test]
fn inline_size_containment_leaves_the_height_to_content() {
    let r = size(
        "<div id=t style='float:left;contain:inline-size;contain-intrinsic-width:50px'><div style='width:500px;height:30px'></div></div>",
        "t",
    );
    assert_eq!(r, (50.0, 30.0));
}

/** A transferred `max-height` limits an auto-width block with an aspect ratio. */
#[test]
fn max_height_transfers_through_the_ratio_to_the_width() {
    let r = size(
        "<div style='height:100px'><div id=t style='aspect-ratio:1/1;max-height:100%'></div></div>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** A ratio box's own min-content width is the width its definite height gives it. */
#[test]
fn ratio_box_min_content_is_the_transferred_width() {
    let r = size(
        "<div id=t style='aspect-ratio:4/1;width:min-content;height:300px;max-height:25px'></div>",
        "t",
    );
    assert_eq!(r, (100.0, 25.0));
}

/** An inline svg is a replaced box: its viewBox ratio meets `aspect-ratio`, min and max sizes. */
#[test]
fn inline_svg_honours_min_height_through_its_ratio() {
    let r = size(
        "<div style='width:50px'><svg id=t viewBox='0 0 2 1' style='max-width:100px;min-height:100px'></svg></div>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** An outermost svg's `width` attribute is a CSS width, so a keyword bound still applies. */
#[test]
fn svg_width_attribute_is_a_presentation_hint() {
    let r = size(
        "<svg id=t width=0 height=100 style='width:100px;height:1000px;max-height:min-content'></svg>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** `height: stretch` fills the definite containing height; adjoining margins collapse away. */
#[test]
fn stretch_height_fills_the_containing_block() {
    let r = size(
        "<div style='height:100px'><div id=t style='width:100px;min-height:stretch;margin-bottom:50px'></div></div>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** In quirks mode an auto-height block passes its containing block's height to percentages. */
#[test]
fn quirks_percentage_heights_skip_auto_blocks() {
    let r = size(
        "<div style='height:100px'><div><div id=t style='height:50%'></div></div></div>",
        "t",
    );
    assert_eq!(r.1, 50.0);
}

/** A percentage margin is cyclic in a min-content contribution: only its fixed part counts. */
#[test]
fn percentage_margins_add_only_their_fixed_part_to_intrinsic_widths() {
    let r = size(
        "<div id=t style='float:left;height:10px'><div style='margin-left:calc(10% + 100px)'></div></div>",
        "t",
    );
    assert_eq!(r.0, 100.0);
}

/** A width the ratio transfers from a definite height may overflow a narrower container. */
#[test]
fn ratio_width_from_a_definite_height_is_not_capped_by_the_container() {
    let r = size(
        "<div style='height:100px;width:50px'><div id=t style='height:100%;aspect-ratio:1/1'></div></div>",
        "t",
    );
    assert_eq!(r, (100.0, 100.0));
}

/** An img showing an svg takes its natural size from the drawing's own attributes: a width
 * alone gives no ratio, so the authored ratio sizes the box from it. */
#[test]
fn svg_image_natural_width_comes_from_its_markup() {
    let mut document = parse(
        "<img id=t style='box-sizing:border-box;aspect-ratio:1/1;padding-left:50px;height:min-content'>",
    )
    .unwrap();
    for node in document.nodes.iter_mut().filter(|n| n.tag == "img") {
        node.image_src = Some("data:image/svg+xml,<svg width=\"50px\"></svg>".into());
        node.image_size = Some((300, 150));
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let item = display.items.iter().find(|i| i.tag == "img").unwrap();
    assert_eq!((item.width, item.height), (100.0, 100.0));
}

/** A drawing an img shows with only a viewBox has a ratio and no size: it lays out as the default
 * object size, 300 by 150, contained by that ratio, whether it is wider or taller than the box. */
#[test]
fn svg_image_with_only_a_view_box_takes_the_default_object_size() {
    for (view_box, expected) in [
        ("0 0 100 50", (300.0, 150.0)),
        ("0 0 50 100", (75.0, 150.0)),
    ] {
        let mut document = parse("<img id=t>").unwrap();
        for node in document.nodes.iter_mut().filter(|n| n.tag == "img") {
            node.image_src = Some(format!(
                "data:image/svg+xml,<svg viewBox=\"{view_box}\"></svg>"
            ));
            node.image_size = Some((100, 50));
        }
        let document = compute_styles(&document, &[]).unwrap();
        let display = render(&document, 800.0).unwrap();
        let item = display.items.iter().find(|i| i.tag == "img").unwrap();
        assert_eq!((item.width, item.height), expected, "viewBox {view_box}");
    }
}

/** `width: stretch` fills the layout opportunity beside a float, not the whole container. */
#[test]
fn stretch_width_fills_the_room_beside_floats() {
    let r = size(
        "<div style='width:200px'><div style='float:left;width:100px;height:10px'></div><div id=t style='display:flow-root;width:stretch;height:10px'></div></div>",
        "t",
    );
    assert_eq!(r.0, 100.0);
}
