/** Round-5 CSS Text rules in Eye: letter spacing skips zero-width controls, Chinese autospacing
 * around ASCII punctuation, preserved white space adds no boundary space to max-content, and a
 * textarea lays out and aligns its own value. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** The page width the cases lay out at. */
const VIEWPORT: f64 = 800.0;
/** The base sheet: no margins, a 20px monospace face. */
const BASE: &str = "body{margin:0;font:20px monospace}";

/** The `#t` element's display item for `markup`. */
fn target(markup: &str) -> DisplayItem {
    let document = compute_styles(&parse(markup).unwrap(), &[BASE.into()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let display = render(&document, VIEWPORT).unwrap();
    display
        .items
        .into_iter()
        .find(|i| i.local_id == local)
        .unwrap()
}

/** A zero-width control takes no letter spacing of its own. */
#[cfg(target_os = "macos")]
#[test]
fn letter_spacing_skips_zero_width_controls() {
    let with = target("<span id=t style='letter-spacing:10px'>a\u{200d}b</span>");
    let without = target("<span id=t style='letter-spacing:10px'>ab</span>");
    assert!(
        (with.width - without.width).abs() < 0.5,
        "{} {}",
        with.width,
        without.width
    );
}

/** A run starting with a byte order mark still shapes with letter spacing (no abort). */
#[test]
fn a_leading_byte_order_mark_shapes() {
    let item = target("<span id=t style='letter-spacing:4px'>\u{feff}abc</span>");
    assert!(item.width > 0.0);
}

/** In Chinese, autospace also separates ASCII punctuation from ideographs. */
#[cfg(target_os = "macos")]
#[test]
fn chinese_autospace_spaces_ascii_punctuation() {
    let zh = target("<span id=t lang=zh style='text-autospace:normal'>国!国</span>");
    let ja = target("<span id=t lang=ja style='text-autospace:normal'>国!国</span>");
    assert!(
        (zh.width - ja.width - 5.0).abs() < 0.01,
        "{} {}",
        zh.width,
        ja.width
    );
}

/** Preserved white space counts with its own text, so it adds no boundary space to a
 * container's max-content. */
#[test]
fn preserved_newline_adds_no_boundary_space() {
    let pre = target(
        "<pre id=t style='float:left'><span style='display:inline-block;width:20px'></span>\n<span style='display:inline-block;width:40px'></span></pre>",
    );
    assert_eq!(pre.width, 40.0);
}

/** A textarea lays out its value at the top of its content box, aligned by `text-align`. */
#[test]
fn textarea_lays_out_and_aligns_its_value() {
    let item = target(
        "<textarea id=t style='width:100px;height:100px;padding:0;border:0;white-space:pre-wrap;text-align:right;font:20px monospace'>ab</textarea>",
    );
    let line = &item.lines[0];
    assert_eq!(line.text, "ab");
    assert_eq!(line.y, item.y);
    assert!(
        (line.x + line.width - (item.x + 100.0)).abs() < 0.5,
        "{line:?}"
    );
}

/** `display: contents` on a textarea shows none of its value. */
#[test]
fn textarea_with_display_contents_shows_nothing() {
    let document = compute_styles(
        &parse("<textarea style='display:contents'>FAIL</textarea>").unwrap(),
        &[BASE.into()],
    )
    .unwrap();
    let display = render(&document, VIEWPORT).unwrap();
    assert!(
        display
            .items
            .iter()
            .all(|i| i.lines.iter().all(|l| l.text != "FAIL"))
    );
}
