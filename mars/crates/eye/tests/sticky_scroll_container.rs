/** Sticky boxes inside scroll containers, ported from Blink's StickyPositionScrollingConstraints:
 * they stick to the container's content box at its scroll offset, within their parent's room. */
use mars_eye::{
    DisplayList, Document, RenderContext, ScrollOffsets, compute_styles, parse, render_scrolled,
};

/** Styles `markup` with one sheet and lays it out at 800 by 600 with `offsets` by element id. */
fn laid(markup: &str, sheet: &str, offsets: &[(&str, [f64; 2])]) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let offsets = offsets
        .iter()
        .map(|(id, offset)| (local(&document, id), *offset))
        .collect::<ScrollOffsets>();
    let context = RenderContext {
        width: 800.0,
        height: 600.0,
        ..RenderContext::default()
    };
    let display = render_scrolled(&document, context, &offsets).unwrap();
    (document, display)
}
/** The local identity of the element with `id`. */
fn local(document: &Document, id: &str) -> usize {
    document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id
}
/** Where the element with `id` paints, as (x, y). */
fn at(document: &Document, display: &DisplayList, id: &str) -> (f64, f64) {
    let id = local(document, id);
    let item = display.items.iter().find(|i| i.local_id == id).unwrap();
    (item.x, item.y)
}

/** `bottom: 0` pulls a box laid out below the scrollport back up to its bottom edge, even
 * unscrolled (position-sticky-margins-002: an auto-margined flex item at 400px shows at 0). */
#[test]
fn a_bottom_sticky_box_pulls_up_to_the_scrollport_bottom() {
    let (document, display) = laid(
        "<div id=scroller><div id=flex><div id=sticky></div></div></div>",
        "body{margin:0}#scroller{width:100px;height:100px;overflow:hidden}\
         #flex{display:flex;flex-direction:column;height:500px}\
         #sticky{width:100px;height:100px;position:sticky;margin-top:auto;bottom:0}",
        &[],
    );
    assert_eq!(at(&document, &display, "sticky"), (0.0, 0.0));
}

/** `top` holds a box at that distance below the scrolled viewport top, but never past the end
 * of its parent's content box. */
#[test]
fn a_top_sticky_box_follows_the_scroll_until_its_parent_ends() {
    let markup =
        "<div id=scroller><div id=parent><div id=sticky></div></div><div id=tail></div></div>";
    let sheet = "body{margin:0}#scroller{width:100px;height:200px;overflow:auto}\
                 #parent{height:300px}#sticky{height:50px;position:sticky;top:10px}#tail{height:500px}";
    let (document, display) = laid(markup, sheet, &[("scroller", [0.0, 100.0])]);
    // Pushed to 100 + 10 in content space, painted 100 higher by the scroll.
    assert_eq!(at(&document, &display, "sticky"), (0.0, 10.0));
    let (document, display) = laid(markup, sheet, &[("scroller", [0.0, 400.0])]);
    // The parent ends at 300, so the box stops at 250 and scrolls away with it.
    assert_eq!(at(&document, &display, "sticky"), (0.0, -150.0));
}
