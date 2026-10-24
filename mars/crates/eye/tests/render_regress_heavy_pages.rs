/** Regressions from Google, Apple and YouTube: flex line height, content buttons, generated-box
opacity, alt text in zero-width images and url() masks. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at a fixed viewport. */
fn layout(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_owned()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** Every item an element with this id painted, in paint order. */
fn items<'a>(document: &Document, display: &'a DisplayList, id: &str) -> Vec<&'a DisplayItem> {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .filter(|item| item.local_id == local)
        .collect()
}
/** The first item an element with this id painted. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    items(document, display, id)[0]
}

/** A single-line row with a definite height centers and stretches within that height, not
within its tallest item (Google's search bar icons sat at the top of a 50px row). */
#[test]
fn single_line_row_aligns_within_container_height() {
    let markup = "<div id=row style='display:flex;height:50px;align-items:center'>\
        <div id=a style='height:24px;width:24px'></div></div>\
        <div id=s style='display:flex;height:50px'><div id=b style='width:10px'>x</div>\
        <div id=c style='width:10px;height:20px'></div></div>";
    let (document, display) = layout(markup, "");
    let row = find(&document, &display, "row");
    let a = find(&document, &display, "a");
    assert_eq!(a.y - row.y, 13.0);
    let s = find(&document, &display, "s");
    assert_eq!(find(&document, &display, "b").height, s.height);
    // An authored height is not stretched.
    assert_eq!(find(&document, &display, "c").height, 20.0);
}

/** min-height floors a single line, so align-items centers inside it. */
#[test]
fn single_line_row_takes_min_height() {
    let markup = "<div id=row style='display:flex;min-height:40px;align-items:center'>\
        <div id=a style='height:10px;width:10px'></div></div>";
    let (document, display) = layout(markup, "");
    let row = find(&document, &display, "row");
    assert_eq!(row.height, 40.0);
    assert_eq!(find(&document, &display, "a").y - row.y, 15.0);
}

/** A button with element children lays them out and centers them in its authored height;
before, every child was dropped and icon buttons painted empty. */
#[test]
fn content_button_lays_out_and_centers_children() {
    let markup = "<button id=b style='height:36px;padding:0;border:0'>\
        <span id=label style='display:inline-block;height:20px'>AI Mode</span></button>";
    let (document, display) = layout(markup, "");
    let button = find(&document, &display, "b");
    let label = find(&document, &display, "label");
    assert_eq!(button.height, 36.0);
    assert_eq!(label.lines[0].text, "AI Mode");
    assert!(
        (label.y - button.y - 8.0).abs() < 0.5,
        "{}",
        label.y - button.y
    );
}

/** A text-only control honors an authored height. */
#[test]
fn control_takes_authored_height() {
    let (document, display) = layout("<input id=i style='height:40px'>", "");
    assert_eq!(find(&document, &display, "i").height, 40.0);
}

/** A transparent `::before` overlay stays transparent instead of painting a solid block. */
#[test]
fn generated_box_keeps_its_own_opacity() {
    let sheet = "#o{position:relative;width:50px;height:20px}\
        #o::before{content:'';display:block;height:20px;background-color:#000;opacity:0}";
    let (document, display) = layout("<div id=o></div>", sheet);
    let before = items(&document, &display, "o")
        .into_iter()
        .find(|item| item.tag == "::before")
        .unwrap();
    assert_eq!(before.opacity, 0.0);
}

/** Alt text that only fits a zero-width positioned image by breaking every word paints nothing
(Apple's loading hero image showed its alt as a column of letters). */
#[test]
fn alt_text_never_breaks_into_letters() {
    let markup = "<div style='position:relative;width:300px;height:100px'>\
        <img id=i alt='held by two hands' style='position:absolute;top:0;left:0;width:0'></div>\
        <img id=ok alt='Logo'>";
    // The runtime shows an unloaded image's alt as its text.
    let mut parsed = parse(markup).unwrap();
    for node in &mut parsed.nodes {
        node.text = node.attributes.get("alt").cloned().unwrap_or_default();
    }
    let document = compute_styles(&parsed, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let image = find(&document, &display, "i");
    assert!(image.lines.is_empty());
    assert!(image.height < 30.0);
    assert_eq!(find(&document, &display, "ok").lines[0].text, "Logo");
}

/** A url() mask keeps the fill it shapes and carries its resolved geometry for the painter. */
#[test]
fn url_mask_keeps_fill_and_geometry() {
    let sheet = "#m{width:24px;height:24px;background-color:#f00;\
        mask-image:url(icon.svg);mask-size:contain;mask-repeat:no-repeat}";
    let (document, display) = layout("<div id=m></div>", sheet);
    let item = find(&document, &display, "m");
    assert_eq!(item.background.as_deref(), Some("#f00"));
    match item.mask.as_ref().unwrap() {
        mars_eye::DisplayBackground::Image { repeat, .. } => assert_eq!(*repeat, [false, false]),
        other => panic!("{other:?}"),
    }
}
