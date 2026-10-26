/** Rules from the position lane's sixth round: the centered inset-modified containing block of
 * an absolute child of a centering flex container, and a sticky box under a fixed ancestor. */
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

/** An absolute child of a `justify-content: center` flex container shrinks to the widest span
 * centered on the container's middle that fits its containing block, and centers there
 * (position-absolute-center-001). */
#[test]
fn a_centered_static_position_centers_a_fitting_width() {
    let (document, display) = laid(
        "<div id=cb><div id=f><div id=a><span></span> <span></span></div></div></div>",
        "body{margin:0}#cb{position:relative;width:100px;height:100px}\
         #f{display:flex;justify-content:center;margin-left:10px;width:40px}\
         #a{position:absolute}span{display:inline-block;width:50px;height:10px}",
    );
    let a = item(&document, &display, "a");
    assert_eq!((a.x, a.width), (0.0, 60.0));
}

/** A `bottom: 0` sticky box inside a fixed container sticks to the viewport's bottom
 * (position-sticky-fixed-ancestor-002). */
#[test]
fn a_sticky_box_under_a_fixed_ancestor_sticks_to_the_viewport() {
    let (document, display) = laid(
        "<div id=f><div id=gap></div><div id=s></div></div>",
        "body{margin:0}#f{position:fixed;top:0;left:0;width:100px;height:600px}\
         #gap{height:600px}#s{position:sticky;bottom:0;height:100px}",
    );
    assert_eq!(item(&document, &display, "s").y, 500.0);
}
