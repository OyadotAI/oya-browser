/** Explicitly stretched replaced grid items, pinned against Chromium's results: an image with
no natural aspect ratio fills both axes, one with a ratio stretched in one axis takes the other
from the ratio, and grid-lanes stretch an image into its lane. */
use mars_eye::{compute_styles, parse, render};

/** The box of the `<img>` in `body` (with natural size `size` and `ratio_free`), laid out at
 * 800px with no body margin: (x, y, width, height). */
fn image(body: &str, size: (u32, u32), ratio_free: bool) -> (f64, f64, f64, f64) {
    let markup = format!("<body style='margin:0'>{body}");
    let mut document = parse(&markup).unwrap();
    for node in document.nodes.iter_mut().filter(|node| node.tag == "img") {
        node.image_size = Some(size);
        node.image_ratio_free = ratio_free;
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let item = display.items.iter().find(|item| item.tag == "img").unwrap();
    (item.x, item.y, item.width, item.height)
}

/** A grid of 350 by 250 holding one image styled `style`. */
fn grid(style: &str) -> String {
    format!(
        "<div style='display:grid;grid-template:100%/100%;width:350px;height:250px'>\
         <img style='{style}'></div>"
    )
}

/** With no natural ratio, `align-self: stretch` fills only the block axis. */
#[test]
fn a_ratio_free_image_stretches_one_axis_alone() {
    let (_, _, width, height) = image(&grid("align-self:stretch"), (300, 20), true);
    assert_eq!((width, height), (300.0, 250.0));
}

/** With a 1:2 ratio, a block-axis stretch takes its width from the ratio. */
#[test]
fn a_block_stretched_image_takes_its_width_from_its_ratio() {
    let (_, _, width, height) = image(&grid("align-self:stretch"), (10, 20), false);
    assert_eq!((width, height), (125.0, 250.0));
}

/** Stretched in both axes, the ratio is ignored; `normal` keeps the natural size. */
#[test]
fn both_axes_stretch_and_normal_keeps_the_natural_size() {
    let both = image(
        &grid("align-self:stretch;justify-self:stretch"),
        (10, 20),
        false,
    );
    assert_eq!((both.2, both.3), (350.0, 250.0));
    let natural = image(&grid(""), (10, 20), false);
    assert_eq!((natural.2, natural.3), (10.0, 20.0));
}

/** A grid-lanes image stretched in its lane takes the lane's width. */
#[test]
fn a_grid_lanes_image_stretches_to_its_lane() {
    let body = "<div style='display:grid-lanes;grid-template-columns:40px'>\
                <img style='justify-self:stretch'></div>";
    let (_, _, width, height) = image(body, (100, 100), false);
    assert_eq!((width, height), (40.0, 40.0));
}
