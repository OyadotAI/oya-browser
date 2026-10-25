/** A block-level button sizes its border box from min-height, padding and border like any block. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first item painted for the element with `id`. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** Equal within layout rounding. */
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/** A restyled `display: block` button is a block box: border-box min-height, padding and border
 * size it (32px, as Chromium), it fits its content's width, and the next box starts below it. */
#[test]
fn block_button_honours_min_height_padding_and_border() {
    let (document, display) = layout(
        "<div><button id=a style='display:block;min-height:32px;font-size:14px;\
         padding:4px 8px;border:1px solid'>Go</button><p id=b style='margin:0'>x</p></div>",
    );
    let button = find(&document, &display, "a");
    assert!(close(button.height, 32.0), "height {}", button.height);
    assert!(button.width < 100.0, "width {}", button.width);
    assert!(close(find(&document, &display, "b").y, button.y + 32.0));
}

/** A native-looking block button honours min-height as well. */
#[test]
fn native_block_button_honours_min_height() {
    let (document, display) =
        layout("<button id=a style='display:block;min-height:32px'>Go</button>");
    assert!(close(find(&document, &display, "a").height, 32.0));
}

/** A taller button centers its whole content box, so an absolutely positioned icon with `top:
 * auto` inside it moves with the text beside it (Google's "AI Mode" pill drew its icon 8px
 * above the label when only in-flow boxes were shifted). */
#[test]
fn centered_button_content_carries_its_positioned_icon() {
    let (document, display) = layout(
        "<button id=b style='display:block;height:36px;padding:0 8px;border:0;font:14px/20px Arial;\
         position:relative'><div id=c style='display:flex;position:relative'>\
         <span id=i style='position:absolute;left:0;width:20px;height:20px'></span>\
         <span style='margin-left:20px'>AI Mode</span></div></button>",
    );
    let button = find(&document, &display, "b");
    let content = find(&document, &display, "c");
    let icon = find(&document, &display, "i");
    assert!(
        close(content.y - button.y, 8.0),
        "content {}",
        content.y - button.y
    );
    assert!(
        close(icon.y, content.y),
        "icon {} content {}",
        icon.y,
        content.y
    );
}
