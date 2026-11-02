/** Regression tests for `border-image` in the screenshot raster: it replaces the border, cuts a
 * bitmap into its nine slices, and falls back to the border when no bytes arrived. */
use super::*;

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;

/** The first `div`'s display item for `markup` styled by `sheet`. */
fn div(markup: &str, sheet: &str) -> DisplayItem {
    let document = crate::document::parse(markup).unwrap();
    let styled = crate::document::compute_styles(&document, &[sheet.into()]).unwrap();
    let display = crate::document::render(&styled, VIEWPORT).unwrap();
    display
        .items
        .into_iter()
        .find(|item| item.tag == "div")
        .unwrap()
}
/** The canvas pixel at document point (x, y). */
fn pixel(canvas: &Canvas, x: usize, y: usize) -> [u8; 3] {
    let at = (y * canvas.width + x) * 3;
    [
        canvas.pixels[at],
        canvas.pixels[at + 1],
        canvas.pixels[at + 2],
    ]
}
/** A 3x3 bitmap whose middle pixel is blue and every other pixel green. */
fn nine() -> Picture {
    let mut rgba = [0, 128, 0, 255].repeat(9);
    rgba[16..20].copy_from_slice(&[0, 0, 255, 255]);
    Picture::Raster(Bitmap {
        width: 3,
        height: 3,
        rgba,
    })
}
/** A canvas over the top-left 60x60 of the page with `item` painted. */
fn painted(item: &DisplayItem, picture: Option<&Picture>) -> Canvas {
    let mut canvas = Canvas::new(0.0, 0.0, 60.0, 60.0, WHITE);
    canvas.item(item, BLACK, picture);
    canvas
}

const GREEN: [u8; 3] = [0, 128, 0];
const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const BLANK: [u8; 3] = [255, 255, 255];

/** A gradient border image paints the ring instead of the red border and leaves the middle. */
#[test]
fn a_gradient_replaces_the_border() {
    let item = div(
        "<div></div>",
        "body{margin:0}div{width:20px;height:20px;border:10px solid red;\
         border-image:linear-gradient(green,green) 10}",
    );
    let canvas = painted(&item, None);
    assert_eq!(pixel(&canvas, 2, 2), GREEN);
    assert_eq!(pixel(&canvas, 35, 20), GREEN);
    assert_eq!(pixel(&canvas, 20, 20), BLANK);
}
/** `fill` paints the middle slice too. */
#[test]
fn fill_paints_the_middle() {
    let item = div(
        "<div></div>",
        "body{margin:0}div{width:20px;height:20px;border:10px solid red;\
         border-image:linear-gradient(green,green) 10 fill}",
    );
    assert_eq!(pixel(&painted(&item, None), 20, 20), GREEN);
}
/** A bitmap's one-pixel slices stretch: green ring, the blue middle pixel over the middle. */
#[test]
fn a_bitmap_is_cut_into_nine_slices() {
    let item = div(
        "<div></div>",
        "body{margin:0}div{width:20px;height:20px;border:10px solid red;\
         border-image:url(nine.png) 1 fill}",
    );
    let canvas = painted(&item, Some(&nine()));
    assert_eq!(pixel(&canvas, 5, 5), GREEN);
    assert_eq!(pixel(&canvas, 20, 35), GREEN);
    assert_eq!(pixel(&canvas, 20, 20), BLUE);
}
/** Without fetched bytes the ordinary border paints, as Blink falls back. */
#[test]
fn missing_bytes_keep_the_border() {
    let item = div(
        "<div></div>",
        "body{margin:0}div{width:20px;height:20px;border:10px solid red;\
         border-image:url(nine.png) 1}",
    );
    assert!(item.border_image.is_some());
    assert_eq!(pixel(&painted(&item, None), 5, 5), RED);
}
/** An outset grows the painted area past the border box. */
#[test]
fn outset_paints_outside_the_border_box() {
    let item = div(
        "<div></div>",
        "body{margin:0}div{margin:10px;width:20px;height:20px;border:5px solid red;\
         border-image:linear-gradient(green,green) 1 / 5px / 5px}",
    );
    let canvas = painted(&item, None);
    assert_eq!(pixel(&canvas, 7, 7), GREEN);
    assert_eq!(pixel(&canvas, 12, 12), BLANK);
}
