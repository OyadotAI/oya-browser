/** Regression tests for text runs in the screenshot raster: glyphs follow `word-spacing` and a
 * justified line's split runs, and a hinted run's ascent line lands on a whole pixel. */
use super::*;

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 400.0;
/** The canvas the cases paint, in px: wide and tall enough for one line of text. */
const SIZE: [f64; 2] = [400.0, 60.0];

/** How far from a whole pixel float rounding may leave a snapped edge. */
const GRID_TOLERANCE: f64 = 1e-9;

/** The `#t` element's display item for `markup` styled by `sheet`. */
fn item(markup: &str, sheet: &str) -> DisplayItem {
    let document = crate::document::parse(markup).unwrap();
    let styled = crate::document::compute_styles(&document, &[sheet.into()]).unwrap();
    let local = styled
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let display = crate::document::render(&styled, VIEWPORT).unwrap();
    display
        .items
        .into_iter()
        .find(|item| item.local_id == local)
        .unwrap()
}
/** The rightmost canvas column with any ink after painting `item` alone. */
fn rightmost_ink(item: &DisplayItem) -> Option<usize> {
    let mut canvas = Canvas::new(0.0, 0.0, SIZE[0], SIZE[1], WHITE);
    canvas.item(item, BLACK, None);
    (0..canvas.width)
        .rev()
        .find(|&x| (0..canvas.height).any(|y| canvas.pixels[(y * canvas.width + x) * 3] < u8::MAX))
}

/** Glyphs after a space move by `word-spacing`, so the run's ink reaches as far as layout
 * placed it instead of bunching at its start. */
#[cfg(target_os = "macos")]
#[test]
fn word_spacing_moves_the_glyphs_after_a_space() {
    let sheet = "body{margin:0;font:16px serif}";
    let plain = item("<div id=t>I I</div>", sheet);
    let spaced = item("<div id=t style='word-spacing:100px'>I I</div>", sheet);
    assert_eq!(spaced.word_spacing, 100.0);
    let (plain, spaced) = (
        rightmost_ink(&plain).unwrap(),
        rightmost_ink(&spaced).unwrap(),
    );
    assert!(spaced >= plain + 99, "{plain} {spaced}");
}

/** Coverage one level from clear or solid is an on-boundary edge's residue, and settles. */
#[cfg(target_os = "macos")]
#[test]
fn edge_residue_settles_to_clear_or_solid() {
    assert_eq!(glyphs::settled(1), 0);
    assert_eq!(glyphs::settled(254), 255);
    assert_eq!(glyphs::settled(128), 128);
}

/** A hinted run is drawn so its font's ascent line falls on a whole pixel, as a hinting
 * rasterizer snaps it; an unhinted one keeps its laid-out baseline. */
#[test]
fn a_hinted_runs_ascent_line_is_on_the_pixel_grid() {
    let item = item("<div id=t>x</div>", "body{margin:0;font:16px serif}");
    let ascent = glyphs::true_ascent(glyphs::face(&item));
    let drawn = glyphs::drawn_baseline(&item, 63.5, true);
    let top = drawn - ascent;
    assert!((top - top.round()).abs() < GRID_TOLERANCE, "{top}");
    assert_eq!(glyphs::drawn_baseline(&item, 63.5, false), 63.5);
}

/** A gradient tile placed by `background-size` and `-position` paints only inside the tile. */
#[test]
fn a_gradient_tile_paints_only_its_tile() {
    let item = item(
        "<div id=t></div>",
        "body{margin:0}#t{width:40px;height:40px;\
         background:linear-gradient(red,red) 10px 10px/10px 10px no-repeat}",
    );
    let mut canvas = Canvas::new(0.0, 0.0, SIZE[0], SIZE[1], WHITE);
    canvas.item(&item, BLACK, None);
    let red = |x: usize, y: usize| canvas.pixels[(y * canvas.width + x) * 3 + 1] == 0;
    assert!(red(15, 15));
    assert!(!red(5, 5));
    assert!(!red(25, 25));
}

/** Whether the canvas holds any ink after a paint on white. */
fn inked(canvas: &Canvas) -> bool {
    canvas.pixels.iter().any(|&channel| channel < u8::MAX)
}

/** A decorated block whose runs are deferred paints no text of its own; its echo paints them. */
#[test]
fn deferred_text_paints_only_at_its_echo() {
    let mut source = item("<div id=t>I</div>", "body{margin:0;font:16px serif}");
    source.text_deferred = true;
    let mut canvas = Canvas::new(0.0, 0.0, SIZE[0], SIZE[1], WHITE);
    canvas.item(&source, BLACK, None);
    assert!(!inked(&canvas));
    canvas.echo(&source, BLACK);
    assert!(inked(&canvas));
}
