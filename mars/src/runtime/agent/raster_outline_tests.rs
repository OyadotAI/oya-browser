/** Outlines in the screenshot raster: a ring paints after the content it surrounds, so a child's
 * background cannot cover an inset ring. */
use super::*;

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 400.0;
/** The canvas the cases paint, in px. */
const SIZE: [f64; 2] = [40.0, 40.0];

/** Every item of `markup` painted onto one canvas, then its outlines, as the screenshot paints. */
fn paint(markup: &str) -> Canvas {
    let sheet = "body{margin:0}";
    let document = crate::document::parse(markup).unwrap();
    let styled = crate::document::compute_styles(&document, &[sheet.into()]).unwrap();
    let display = crate::document::render(&styled, VIEWPORT).unwrap();
    let mut canvas = Canvas::new(0.0, 0.0, SIZE[0], SIZE[1], WHITE);
    for item in &display.items {
        canvas.item(item, BLACK, None);
    }
    canvas.paint_outlines();
    canvas
}

/** The RGB pixel at canvas column `x`, row `y`. */
fn at(canvas: &Canvas, x: usize, y: usize) -> [u8; 3] {
    let i = (y * canvas.width + x) * 3;
    [canvas.pixels[i], canvas.pixels[i + 1], canvas.pixels[i + 2]]
}

/** A ring with a negative offset sits inside its box. A child's background paints after the box's
 * own item, so only a ring painted after the content keeps its color over the child. */
#[test]
fn an_inset_outline_stays_over_a_child_background() {
    let canvas = paint(
        "<div style='width:40px;height:40px;background:white;outline:4px solid rgb(255,0,0);outline-offset:-2px'>\
         <div style='width:40px;height:40px;background:rgb(0,0,255)'></div></div>",
    );
    // The ring's outer band sits at the box's left edge; the child's blue must not cover it.
    let [r, g, b] = at(&canvas, 1, 20);
    assert!(r > 200 && g < 60 && b < 60, "ring covered: {:?}", [r, g, b]);
}
