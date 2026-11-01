/** Regression tests for decoration lines and outline rings in the screenshot raster: they paint
 * where the window strokes them, and page CSS cannot make their planning unbounded. */
use super::*;

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 400.0;
/** The canvas the cases paint, in px. */
const SIZE: [f64; 2] = [400.0, 120.0];

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

/** Inked pixels in canvas rows `rows` after painting `item` alone. */
fn ink(item: &DisplayItem, rows: std::ops::Range<usize>) -> usize {
    let mut canvas = Canvas::new(0.0, 0.0, SIZE[0], SIZE[1], WHITE);
    canvas.item(item, BLACK, None);
    rows.flat_map(|y| (0..canvas.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let at = (y * canvas.width + x) * 3;
            canvas.pixels[at..at + 3]
                .iter()
                .any(|&channel| channel < u8::MAX)
        })
        .count()
}

/** An underline inks a band below the text, where an undecorated run leaves the rows blank. */
#[test]
fn an_underline_paints_below_the_text() {
    let sheet = "body{margin:0;font:16px serif;line-height:20px}";
    let plain = item("<div id=t>underlined words</div>", sheet);
    let lined = item(
        "<div id=t style='text-decoration:underline'>underlined words</div>",
        sheet,
    );
    // The 16px font's baseline sits near row 14.5, so the underline lands on row 16.
    let below = 16..17;
    assert!(ink(&lined, below.clone()) > ink(&plain, below.clone()) + 50);
}

/** An outline rings the box outside its border edge, at its offset. */
#[test]
fn an_outline_rings_the_box() {
    let sheet = "body{margin:0}#t{margin:20px;width:100px;height:40px;outline:2px solid red;outline-offset:3px}";
    let boxed = item("<div id=t></div>", sheet);
    // The ring's top band spans rows 15 and 16: 20 minus the 3px offset minus the 2px width.
    assert!(ink(&boxed, 15..17) >= 2 * 100);
    assert_eq!(
        ink(&boxed, 25..55),
        2 * 2 * 30,
        "only the side bands inside the box's rows"
    );
}

/** A dotted outline as thin and as far out as page CSS allows still paints in bounded time. */
#[test]
fn a_hostile_dotted_outline_stays_bounded() {
    let sheet = "body{margin:0}#t{width:100px;height:40px;outline:0.01px dotted red;outline-offset:100000px}";
    let started = std::time::Instant::now();
    let _ = ink(&item("<div id=t></div>", sheet), 0..1);
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}

/** A wavy line with zero thickness still terminates and paints. */
#[test]
fn a_zero_thickness_wavy_line_terminates() {
    let sheet = "body{margin:0;font:16px serif}";
    let wavy = item(
        "<div id=t style='text-decoration:underline wavy;text-decoration-thickness:0'>wave</div>",
        sheet,
    );
    assert!(ink(&wavy, 0..40) > 0);
}

/** A box so far down the page that adding a point to its coordinate changes nothing still has its
dashed outline planned in bounded steps. */
#[test]
fn a_far_away_dashed_line_terminates() {
    let mut canvas = Canvas::new(1e17, 1e17, SIZE[0], SIZE[1], WHITE);
    let mut boxed = item(
        "<div id=t></div>",
        "#t{width:100px;height:40px;outline:1px dashed red}",
    );
    (boxed.x, boxed.y) = (1e17, 1e17);
    let started = std::time::Instant::now();
    canvas.item(&boxed, BLACK, None);
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
}
