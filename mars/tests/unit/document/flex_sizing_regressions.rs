/** Nested intrinsic contributions and percentage references retain the containing content-box contract. */
use mars::document::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** Tests exercise actual CSS, native intrinsic policies and the public display list together. */
fn layout(html: &str, css: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(html).unwrap(), &[css.into()]).unwrap();
    let display = render(&document, 640.0).unwrap();
    (document, display)
}

/** Author identifiers locate stable nodes without relying on insertion indices. */
fn item(document: &Document, display: &DisplayList, id: &str) -> DisplayItem {
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
        .clone()
}

/** A nested field's max-width caps its contribution to the outer flex item's automatic minimum. */
#[test]
fn nested_max_width_caps_the_outer_automatic_minimum() {
    let (document, display) = layout(
        "<div id=row><div id=a><input id=field></div><div id=b></div></div>",
        "#row{display:flex;width:200px}#a{flex:1 1 auto}#field{max-width:50px}#b{flex:0 0 100px;padding:10px 0}",
    );
    assert_eq!(item(&document, &display, "a").width, 100.0);
    assert_eq!(item(&document, &display, "field").width, 50.0);
    assert_eq!(item(&document, &display, "b").width, 100.0);
}

/** Border-box constraints propagate content units before child decorations are added once. */
#[test]
fn nested_border_box_maximum_includes_padding_and_border_once() {
    let (document, display) = layout(
        "<div id=row><div id=a><input id=field></div><div id=b></div></div>",
        "#row{display:flex;width:200px}#a{flex:1 1 auto}#field{box-sizing:border-box;max-width:80px;padding:10px;border:2px solid #000}#b{flex:0 0 100px;padding:10px 0}",
    );
    assert_eq!(item(&document, &display, "a").width, 100.0);
    assert_eq!(item(&document, &display, "field").width, 80.0);
}

/** Fixed nonshrinking child bases contribute even when the children have no text. */
#[test]
fn nested_fixed_flex_bases_determine_the_parent_intrinsic_width() {
    let (document, display) = layout(
        "<div id=row><div id=a><div id=first></div><div id=second></div></div><div id=b></div></div>",
        "#row{display:flex;width:300px}#a{display:flex;flex:0 1 auto}#a div{flex:0 0 100px;padding:10px 0}#b{flex:0 0 100px}",
    );
    assert_eq!(item(&document, &display, "a").width, 200.0);
    assert_eq!(item(&document, &display, "first").width, 100.0);
    assert_eq!(item(&document, &display, "second").width, 100.0);
    assert_eq!(item(&document, &display, "b").width, 100.0);
}

/** Column percentage widths use containing content width before automatic margin deduction. */
#[test]
fn column_percent_width_uses_the_containing_content_width() {
    let (document, display) = layout(
        "<div id=column><div id=child></div></div>",
        "#column{display:flex;flex-direction:column;width:200px}#child{width:50%;margin:0 20px;padding:10px 0}",
    );
    assert_eq!(item(&document, &display, "child").width, 100.0);
    assert_eq!(
        item(&document, &display, "child").x,
        item(&document, &display, "column").x + 20.0
    );
}

/** Ordinary blocks and native controls share the same percentage reference as flex columns. */
#[test]
fn block_and_control_percent_widths_use_the_containing_content_width() {
    let (document, display) = layout(
        "<div id=parent><div id=child></div><input id=field></div>",
        "#parent{width:200px}#child,#field{width:50%;margin:0 20px;padding:10px 0}",
    );
    assert_eq!(item(&document, &display, "child").width, 100.0);
    assert_eq!(item(&document, &display, "field").width, 100.0);
}
