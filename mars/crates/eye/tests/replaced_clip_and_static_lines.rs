/** Rules from the position lane's fifth round: replaced elements' UA overflow clip, a lone inline
 * absolute box's aligned static position, percentage offsets through inline parents and a
 * positioned body as containing block. */
use mars_eye::{DisplayList, Document, RenderContext, compute_styles, parse, render_with_context};

/** Styles `markup` with one sheet and lays it out at 800 by 600. */
fn laid(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let context = RenderContext {
        width: 800.0,
        height: 600.0,
        ..RenderContext::default()
    };
    let display = render_with_context(&document, context).unwrap();
    (document, display)
}
/** The first painted item of the element with `id`. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a mars_eye::DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    display.items.iter().find(|i| i.local_id == local).unwrap()
}

/** An image clips by default (html.css `overflow: clip`); an authored `overflow: visible` marks
 * its item so painters draw the whole object rectangle (overflow-img). */
#[test]
fn an_image_paints_past_its_box_only_with_overflow_visible() {
    let (document, display) = laid(
        "<img id=a src=a.png><img id=b src=b.png>",
        "#a{width:10px;height:10px}#b{width:10px;height:10px;overflow:visible}",
    );
    assert!(!item(&document, &display, "a").overflow_visible);
    assert!(item(&document, &display, "b").overflow_visible);
    let clipped = document.nodes.iter().find(|n| n.html_id == "a").unwrap();
    assert_eq!(clipped.style.overflow_x, mars_eye::Overflow::Clip);
}

/** An inline absolute box alone on its line takes the line's `text-align` in the room a float
 * leaves (inline-level-absolute-in-block-level-context-002). */
#[test]
fn a_lone_inline_absolute_box_follows_text_align() {
    let (document, display) = laid(
        "<div id=c><div id=f></div><span id=a></span></div>",
        "body{margin:0}#c{width:100px;text-align:center}#f{float:left;width:50px;height:50px}\
         #a{position:absolute;width:10px;height:10px}",
    );
    assert_eq!(item(&document, &display, "a").x, 75.0);
}

/** A block inside a relatively positioned inline resolves percentage offsets against the block
 * around the inline (position-relative-002). */
#[test]
fn percentage_offsets_skip_inline_parents() {
    let (document, display) = laid(
        "<div id=c><span><div id=d></div></span></div>",
        "body{margin:0}#c{width:100px;height:100px}#d{position:relative;top:-100%;height:10px}",
    );
    assert_eq!(item(&document, &display, "d").y, -100.0);
}

/** A positioned body is the containing block of top-level absolute boxes (CSS2 abspos-009). */
#[test]
fn a_positioned_body_contains_absolute_boxes() {
    let (document, display) = laid(
        "<div id=a></div>",
        "body{position:relative;margin:8px}#a{position:absolute;right:0;width:100px;height:10px}",
    );
    assert_eq!(item(&document, &display, "a").x, 792.0 - 100.0);
}
