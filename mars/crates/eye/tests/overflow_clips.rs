/** Overflow and positioning rules from the position lane's second round: physical unit
 * precision, out-of-flow table parts, the body's own overflow clip, replaced-element clip
 * margins and scroll marker group containment. */
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

/** `25.4mm` is exactly one inch, 96px, not a float hair short of it (CSS2 left-055). */
#[test]
fn physical_units_convert_without_rounding_error() {
    let (document, display) = laid(
        "<div id=d></div>",
        "body{margin:0}#d{width:25.4mm;height:1in}",
    );
    assert_eq!(item(&document, &display, "d").width, 96.0);
}

/** An absolutely positioned cell inside a row is blockified and placed, not lost because it
 * takes no grid slot (CSS2 top-applies-to-007). */
#[test]
fn an_abspos_cell_in_a_row_is_placed() {
    let (document, display) = laid(
        "<div id=t><div id=r><div id=c></div></div></div>",
        "#t{display:table}#r{display:table-row}\
         #c{display:table-cell;position:absolute;top:0;width:50px;height:50px}",
    );
    assert_eq!(item(&document, &display, "c").y, 0.0);
}

/** With the root's overflow hidden, the body keeps its own and clips its content at its padding
 * box (overflow-body-propagation-012). */
#[test]
fn the_body_clips_when_its_overflow_does_not_propagate() {
    let (document, display) = laid(
        "<div id=d></div>",
        "html{overflow:hidden}body{overflow:hidden;margin:0;width:50px;height:50px}\
         #d{width:200px;height:200px}",
    );
    let clip = item(&document, &display, "d").clip.unwrap();
    assert_eq!((clip.width, clip.height), (50.0, 50.0));
}

/** With a visible root, the body's overflow goes to the viewport and it clips nothing. */
#[test]
fn the_body_does_not_clip_when_its_overflow_propagates() {
    let (document, display) = laid(
        "<div id=d></div>",
        "body{overflow:hidden;margin:0;width:50px;height:50px}#d{width:200px;height:200px}",
    );
    assert!(item(&document, &display, "d").clip.is_none());
}

/** A replaced element's vector content may paint `overflow-clip-margin` past its box
 * (overflow-clip-margin-svg). */
#[test]
fn an_svg_clip_grows_by_its_clip_margin() {
    let (document, display) = laid(
        "<svg id=s><rect width=150 height=150 fill=green></rect></svg>",
        "body{margin:0}#s{width:100px;height:100px;overflow-clip-margin:10px}",
    );
    let clip = item(&document, &display, "s").clip.unwrap();
    assert_eq!((clip.x, clip.width), (-10.0, 120.0));
}

/** A scroll marker group contains its absolutely positioned markers (scroll-marker-008). */
#[test]
fn a_marker_group_contains_its_abspos_markers() {
    let (_, display) = laid(
        "<div id=s><div></div></div>",
        "body{margin:0}#s{overflow:hidden;scroll-marker-group:after}\
         #s::scroll-marker-group{width:100px;height:100px}\
         #s>*::scroll-marker{position:absolute;right:0;top:0;width:10px;height:10px;content:''}",
    );
    let group = display
        .items
        .iter()
        .find(|i| i.tag == "::scroll-marker-group")
        .unwrap();
    let marker = display
        .items
        .iter()
        .find(|i| i.tag == "::scroll-marker")
        .unwrap();
    assert_eq!((marker.x, marker.y), (group.x + 90.0, group.y));
}
