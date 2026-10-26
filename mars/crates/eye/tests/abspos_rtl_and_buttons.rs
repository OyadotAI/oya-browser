/** Rules from the position lane's third round: right-to-left absolute placement, percentage
 * offsets of top-level boxes, block static positions beside floats, scroll ranges that skip
 * clamped content, `::scroll-button()` boxes and CSS-sized SVG rects. */
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
/** A 200px right-to-left containing block at the page origin. */
const RTL_BLOCK: &str =
    "body{margin:0}#c{position:relative;width:200px;height:200px;direction:rtl}";

/** An over-constrained absolute box in a right-to-left block drops `left` (CSS 2.1 10.3.7). */
#[test]
fn an_over_constrained_rtl_box_keeps_its_right_inset() {
    let sheet = format!("{RTL_BLOCK}#a{{position:absolute;left:10px;right:20px;width:50px}}");
    let (document, display) = laid("<div id=c><div id=a></div></div>", &sheet);
    assert_eq!(item(&document, &display, "a").x, 130.0);
}

/** Negative auto margins go to `margin-left` in a right-to-left block
 * (absolute-non-replaced-width-005). */
#[test]
fn negative_auto_margins_go_left_in_rtl() {
    let sheet = format!(
        "{RTL_BLOCK}#a{{position:absolute;left:100px;right:100px;width:100px;margin:0 auto}}"
    );
    let (document, display) = laid("<div id=c><div id=a></div></div>", &sheet);
    assert_eq!(item(&document, &display, "a").x, 0.0);
}

/** A top-level relative box's percentage `top` resolves against the body's height (top-113). */
#[test]
fn a_top_level_percentage_top_uses_the_body_height() {
    let (document, display) = laid(
        "<div id=a></div>",
        "body{margin:0;height:100px}#a{position:relative;top:50%;height:10px}",
    );
    assert_eq!(item(&document, &display, "a").y, 50.0);
}

/** A block-level absolute box's static position is the content edge, not past a float
 * (abspos-028). */
#[test]
fn a_block_static_position_ignores_floats() {
    let (document, display) = laid(
        "<div id=w><div id=f></div><div id=a></div></div>",
        "body{margin:0}#f{float:left;width:30px;height:30px}\
         #a{position:absolute;width:10px;height:10px}",
    );
    assert_eq!(item(&document, &display, "a").x, 0.0);
}

/** Lines a line clamp hides are no scrollable overflow (line-clamp-021). */
#[test]
fn clamped_lines_add_no_scroll_range() {
    let (_, display) = laid(
        "<div id=s><div id=c>1<br>2<br>3<br>4<br>5<br>6</div></div>",
        "body{margin:0}#s{overflow:scroll;height:40px;font:10px/20px serif}#c{line-clamp:2}",
    );
    assert!(display.scrollers.is_empty());
}

/** `::scroll-button()` boxes follow their element in block-start, inline-start, inline-end,
 * block-end order, and `*` styles them all (scroll-buttons-004). */
#[test]
fn scroll_buttons_follow_their_element_in_order() {
    let (_, display) = laid(
        "<div id=d></div>",
        "body{margin:0}#d::scroll-button(*){content:'';display:block;height:10px}\
         #d::scroll-button(block-end){height:20px}",
    );
    let buttons: Vec<(f64, f64)> = display
        .items
        .iter()
        .filter(|i| i.tag == "::scroll-button")
        .map(|i| (i.y, i.height))
        .collect();
    assert_eq!(
        buttons,
        [(0.0, 10.0), (10.0, 10.0), (20.0, 10.0), (30.0, 20.0)]
    );
}

/** CSS `width` and `height` size an SVG rect over its attributes (rounded-overflow-clip-visible). */
#[test]
fn css_sizes_an_svg_rect() {
    let (document, display) = laid(
        "<svg id=s width=300 height=150><rect width=10 height=10></rect></svg>",
        "body{margin:0}rect{width:200px;height:100px}",
    );
    let svg = item(&document, &display, "s");
    let path = &svg.vector[0];
    assert!(format!("{path:?}").contains("200"), "{path:?}");
}

/** An absolute svg with no width between set insets fills its containing block from `left`
 * and its margins, dropping `right` (absolute-replaced-width-065). */
#[test]
fn an_unsized_abspos_svg_fills_from_its_left_inset() {
    let (document, display) = laid(
        "<div id=c><svg id=s height=50></svg></div>",
        "body{margin:0}#c{position:relative;width:300px;height:100px}\
         #s{position:absolute;left:20px;right:40px;margin:0 10px}",
    );
    let svg = item(&document, &display, "s");
    assert_eq!((svg.x, svg.width), (30.0, 260.0));
}
