/** Replaced-element sizing ported from Blink's ComputeReplacedSize (CSS 2.1 10.3.2, 10.4, 10.6.2, 10.7 and css-sizing-4 aspect-ratio transfers). */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** Lays out `markup` at 800px, giving every img with an `src` the natural size `size`. */
fn layout(markup: &str, size: (u32, u32)) -> (Document, DisplayList) {
    let mut document = parse(markup).unwrap();
    for node in document.nodes.iter_mut().filter(|n| n.tag == "img") {
        node.image_src = Some("x.png".into());
        node.image_size = Some(size);
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first item painted for the element with `id`, as (x, y, width, height). */
fn rect(markup: &str, size: (u32, u32), id: &str) -> (f64, f64, f64, f64) {
    let (document, display) = layout(markup, size);
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item: &DisplayItem = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width, item.height)
}

/** Equal within layout rounding. */
fn close(actual: (f64, f64, f64, f64), width: f64, height: f64) -> bool {
    (actual.2 - width).abs() < 0.05 && (actual.3 - height).abs() < 0.05
}

/** CSS 2.1 10.4: `max-width: 100%` narrows a wide image and its height follows the ratio. */
#[test]
fn max_width_scales_the_height_through_the_ratio() {
    let r = rect(
        "<div style='width:400px'><img id=i style='max-width:100%'></div>",
        (700, 560),
        "i",
    );
    assert!(close(r, 400.0, 320.0), "{r:?}");
}

/** CSS 2.1 10.7: max-height caps an automatic height and the width follows through the ratio. */
#[test]
fn max_height_transfers_to_the_width() {
    let r = rect(
        "<img id=i style='display:block;max-height:200px'>",
        (700, 560),
        "i",
    );
    assert!(close(r, 250.0, 200.0), "{r:?}");
}

/** css-sizing-4 transfer: min-height grows an automatic image and its width with it. */
#[test]
fn min_height_transfers_to_the_width() {
    let r = rect(
        "<img id=i style='display:block;min-height:100px'>",
        (100, 50),
        "i",
    );
    assert!(close(r, 200.0, 100.0), "{r:?}");
}

/** CSS 2.1 10.3.2: an auto-sized image keeps its natural width and overflows a narrower block. */
#[test]
fn natural_width_is_not_capped_by_the_container() {
    let r = rect(
        "<div style='width:300px'><img id=i style='display:block'></div>",
        (500, 100),
        "i",
    );
    assert!(close(r, 500.0, 100.0), "{r:?}");
}

/** An authored `aspect-ratio` overrides the image's natural ratio. */
#[test]
fn authored_aspect_ratio_overrides_the_natural_ratio() {
    let r = rect(
        "<img id=i style='display:block;width:200px;aspect-ratio:1'>",
        (700, 560),
        "i",
    );
    assert!(close(r, 200.0, 200.0), "{r:?}");
}

/** `aspect-ratio: auto && <ratio>` prefers the natural ratio when the image has one. */
#[test]
fn auto_aspect_ratio_prefers_the_natural_ratio() {
    let r = rect(
        "<img id=i style='display:block;width:200px;aspect-ratio:auto 1/1'>",
        (700, 560),
        "i",
    );
    assert!(close(r, 200.0, 160.0), "{r:?}");
}

/** A border-box authored ratio sizes the border box, not the content box. */
#[test]
fn border_box_ratio_includes_padding() {
    let r = rect(
        "<img id=i style='display:block;box-sizing:border-box;width:200px;padding:10px;aspect-ratio:2'>",
        (10, 10),
        "i",
    );
    assert!(close(r, 200.0, 100.0), "{r:?}");
}

/** A percentage height of a definite parent sizes the image and the width follows the ratio. */
#[test]
fn percent_height_of_a_definite_parent() {
    let r = rect(
        "<div style='height:200px'><img id=i style='display:block;height:50%'></div>",
        (400, 200),
        "i",
    );
    assert!(close(r, 200.0, 100.0), "{r:?}");
}

/** A percentage height of an indefinite parent behaves as auto. */
#[test]
fn percent_height_of_an_indefinite_parent_is_auto() {
    let r = rect(
        "<div><img id=i style='display:block;height:50%'></div>",
        (400, 200),
        "i",
    );
    assert!(close(r, 400.0, 200.0), "{r:?}");
}

/** Blink's compressible replaced contribution: a `width: 100%` image adds nothing to its grid
track's minimum, so a `1fr` column stays half the grid (LinkedIn's hero illustration). */
#[test]
fn percent_width_image_is_compressible_in_a_grid_track() {
    let markup = "<div style='display:grid;grid-template-columns:repeat(2,1fr)'><div>left</div>\
        <figure style='margin:0;height:100%'><img id=i style='display:block;max-width:100%;height:100%;width:100%'></figure></div>";
    let r = rect(markup, (700, 560), "i");
    assert!(close(r, 392.0, 313.6), "{r:?}");
}
