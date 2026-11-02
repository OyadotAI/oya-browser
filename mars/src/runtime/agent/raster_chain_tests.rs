/** Chained runs in the screenshot raster: text split into inline boxes paints the same pixels as
 * the same text in one run, and each run keeps its own color. */
use super::*;

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 400.0;
/** The canvas the cases paint, in px. */
const SIZE: [f64; 2] = [400.0, 40.0];

/** Every item of `markup` painted onto one canvas, chains included, as the screenshot paints. */
fn paint(markup: &str) -> Canvas {
    let sheet = "body{margin:0;font:16px serif}";
    let document = crate::document::parse(markup).unwrap();
    let styled = crate::document::compute_styles(&document, &[sheet.into()]).unwrap();
    let display = crate::document::render(&styled, VIEWPORT).unwrap();
    let mut canvas = Canvas::new(0.0, 0.0, SIZE[0], SIZE[1], WHITE);
    canvas.chains = crate::document::text_chains(display.items.iter());
    for item in &display.items {
        canvas.item(item, BLACK, None);
    }
    canvas
}

/** Runs of separate inline boxes draw exactly as one run of the same text. */
#[cfg(target_os = "macos")]
#[test]
fn split_runs_paint_as_one_run() {
    let joined = paint("<div>Filler Text Filler Text Filler Text</div>");
    let split = paint(
        "<div><span>Filler Text</span> <span>Filler Text</span> <span>Filler Text</span></div>",
    );
    assert!(joined.pixels == split.pixels);
}

/** A chained run paints its share in its own color. */
#[cfg(target_os = "macos")]
#[test]
fn chained_runs_keep_their_colors() {
    let canvas = paint("<div>AAAA<span style='color:red'>BBBB</span></div>");
    let red = canvas
        .pixels
        .chunks_exact(3)
        .any(|p| p[0] > 200 && p[1] < 80 && p[2] < 80);
    assert!(red);
}
