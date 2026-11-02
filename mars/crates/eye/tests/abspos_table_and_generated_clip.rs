/** Rules from the position lane's seventh round: an absolute table centered by auto margins
 * between its insets, the svg UA overflow and a generated box under its element's clip. */
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

/** An absolute table between `top: 0` and `bottom: 0` keeps its rows' height and its auto
 * margins center it (position-absolute-center-007). */
#[test]
fn an_abspos_table_is_centered_not_stretched() {
    let (document, display) = laid(
        "<div id=cb><div id=t><div id=c></div></div></div>",
        "body{margin:0}#cb{position:relative;width:100px;height:200px}\
         #t{display:table;position:absolute;top:0;bottom:0;margin:auto;width:100px}\
         #c{width:100px;height:100px}",
    );
    let table = item(&document, &display, "t");
    assert_eq!((table.y, table.height), (50.0, 100.0));
}

/** An inline svg gets svg.css's `overflow: hidden`, so as a flex item it has no automatic
 * minimum size (display-flex-svg-overflow-default). */
#[test]
fn an_svg_flex_item_shrinks_below_its_content() {
    let (document, display) = laid(
        "<div id=f><svg id=s><rect width=150 height=150></rect></svg></div>",
        "body{margin:0}#f{display:flex;width:100px;height:100px}#s{flex-grow:1}",
    );
    assert_eq!(item(&document, &display, "s").width, 100.0);
}

/** A `::before` box is clipped by its own element's overflow clip
 * (overflow-clip-margin-border-radius-002). */
#[test]
fn a_generated_box_is_clipped_by_its_element() {
    let (_, display) = laid(
        "<div id=d></div>",
        "body{margin:0}#d{width:20px;height:20px;overflow:clip}\
         #d::before{content:'';display:block;width:100px;height:100px;background:black}",
    );
    let before = display.items.iter().find(|i| i.tag == "::before").unwrap();
    let clip = before.clip.unwrap();
    assert_eq!((clip.width, clip.height), (20.0, 20.0));
}
