/** CSS Overflow 5 scroll markers: a scroller's `::scroll-marker-group` box and the
 * `::scroll-marker` boxes of the elements it scrolls. */
use mars_eye::{DisplayList, RenderContext, compute_styles, parse, render_with_context};

/** Styles `markup` with one sheet and lays it out at 800 by 600. */
fn laid(markup: &str, sheet: &str) -> DisplayList {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let context = RenderContext {
        width: 800.0,
        height: 600.0,
        ..RenderContext::default()
    };
    render_with_context(&document, context).unwrap()
}
/** The painted items with `tag`, as (x, y, width, height). */
fn boxes(display: &DisplayList, tag: &str) -> Vec<(f64, f64, f64, f64)> {
    display
        .items
        .iter()
        .filter(|item| item.tag == tag)
        .map(|item| (item.x, item.y, item.width, item.height))
        .collect()
}
/** A 600 by 300 scroller of two 300px children, each with a 10px inline-block marker. */
const SCROLLER: &str = "<div id=s>\n <div id=a></div>\n <div></div>\n</div>";
/** The scroller's sheet; `{side}` is its `scroll-marker-group`. */
fn sheet(side: &str) -> String {
    format!(
        "body{{margin:0}}#s{{width:600px;height:300px;overflow:scroll;scroll-marker-group:{side}}}\
         #s div{{height:300px}}#s::scroll-marker-group{{height:20px}}\
         #s div::scroll-marker{{content:'';width:10px;height:10px;display:inline-block}}\
         #a::scroll-marker{{margin-right:4px}}"
    )
}

/** `after` places the group below the scroller, holding one marker per scrolled element in
 * order, with no source white space between them (scroll-marker-001). */
#[test]
fn an_after_group_follows_its_scroller_with_one_marker_per_element() {
    let display = laid(SCROLLER, &sheet("after"));
    assert_eq!(
        boxes(&display, "::scroll-marker-group"),
        [(0.0, 300.0, 800.0, 20.0)]
    );
    let markers = boxes(&display, "::scroll-marker");
    assert_eq!(markers.len(), 2);
    assert_eq!((markers[0].0, markers[1].0), (0.0, 14.0));
}

/** `before` places the group above the scroller, pushing it down. */
#[test]
fn a_before_group_precedes_its_scroller() {
    let display = laid(SCROLLER, &sheet("before"));
    assert_eq!(boxes(&display, "::scroll-marker-group")[0].1, 0.0);
    let scroller = display.items.iter().find(|item| item.tag == "div").unwrap();
    assert_eq!(scroller.y, 20.0);
}

/** `none`, or a box that is no scroll container, gets no group. */
#[test]
fn no_group_without_a_scroll_container() {
    let display = laid(SCROLLER, &sheet("none"));
    assert!(boxes(&display, "::scroll-marker-group").is_empty());
    let display = laid(SCROLLER, &sheet("after").replace("overflow:scroll;", ""));
    assert!(boxes(&display, "::scroll-marker-group").is_empty());
}

/** An absolutely positioned element whose containing block is outside the scroller belongs to
 * no group there (scroll-marker-005). */
#[test]
fn an_element_escaping_the_scroller_has_no_marker_in_it() {
    let markup = "<div id=s><div id=a></div><div style='position:absolute'><div></div></div></div>";
    let display = laid(markup, &sheet("after"));
    assert_eq!(boxes(&display, "::scroll-marker").len(), 1);
}

/** Marker items take their originating element's identity, so hosts see only real nodes. */
#[test]
fn marker_items_keep_their_element_identity() {
    let document = compute_styles(&parse(SCROLLER).unwrap(), &[sheet("after")]).unwrap();
    let display = render_with_context(&document, RenderContext::default()).unwrap();
    let first = document.nodes.iter().find(|n| n.html_id == "a").unwrap();
    let marker = display
        .items
        .iter()
        .find(|i| i.tag == "::scroll-marker")
        .unwrap();
    assert_eq!(marker.local_id, first.local_id);
}

/** `content-visibility: auto` paint-contains its box (css-contain-2). */
#[test]
fn content_visibility_auto_contains_paint() {
    let document = compute_styles(
        &parse("<div id=c></div>").unwrap(),
        &["#c{content-visibility:auto}".into()],
    )
    .unwrap();
    assert!(document.nodes[0].style.contain_paint);
}
