/** Flex intrinsic sizes, flex bases and definite sizes, after Blink's `FlexLayoutAlgorithm`. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let sheet = "body{margin:0}".to_owned();
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The (x, y, width, height) of the first item painted for the element with id `id`. */
fn rect(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let (document, display) = layout(markup);
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    let item: &DisplayItem = display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap();
    (item.x, item.y, item.width, item.height)
}

/** A wrapping column's max-content width adds up its lines: two 50px items that each fill the
100px height sit side by side, so the container is 100px wide, not 50px. */
#[test]
fn column_wrap_max_content_sums_lines() {
    let markup = "<div id=c style='display:flex;flex-flow:column wrap;height:100px;width:max-content'>\
         <i style='display:block;width:50px;flex:0 0 100px;min-height:0'></i>\
         <i style='display:block;width:50px;flex:0 0 100px;min-height:0'></i></div>";
    assert_eq!(rect(markup, "c").2, 100.0);
}

/** Without a definite height a wrapping column breaks its lines at a fixed `max-height`. */
#[test]
fn column_wrap_breaks_at_max_height() {
    let markup = "<div style='display:flex;flex-flow:column wrap;max-height:100px;width:200px;align-content:flex-start'>\
         <i style='display:block;width:50px;flex:0 0 100px;min-height:0'></i>\
         <i id=b style='display:block;width:50px;flex:0 0 100px;min-height:0'></i></div>";
    assert_eq!(rect(markup, "b").0, 50.0);
}

/** Floats sit side by side at max-content: two 50px float divs make a 100px box. */
#[test]
fn floats_add_up_at_max_content() {
    let markup = "<div id=c style='width:max-content'>\
         <div style='float:left;width:50px;height:50px'></div>\
         <div style='float:left;width:50px;height:50px'></div></div>";
    assert_eq!(rect(markup, "c").2, 100.0);
}

/** A stretched item with an aspect ratio gives an `inline-flex` its width through the ratio. */
#[test]
fn stretched_ratio_item_sizes_inline_flex() {
    let markup = "<div id=c style='display:inline-flex;height:100px'><div style='aspect-ratio:1/1'></div></div>";
    assert_eq!(rect(markup, "c").2, 100.0);
}

/** A balanced row's max-content width is its longest balanced line, not all items in one. */
#[test]
fn balanced_row_max_content_is_longest_line() {
    let markup = "<div id=c style='display:inline-flex;gap:20px;flex-wrap:balance;flex-line-count:2;height:100px'>\
         <div style='width:40px'></div><div style='width:40px'></div><div style='width:40px'></div></div>";
    assert_eq!(rect(markup, "c").2, 100.0);
}

/** `flex-basis: content` sizes an item from its content and ignores its authored width. */
#[test]
fn content_basis_ignores_width() {
    let markup = "<div style='display:flex;width:300px'>\
         <div id=a style='flex:0 0 content;width:10px'><div style='width:60px;height:5px'></div></div></div>";
    assert_eq!(rect(markup, "a").2, 60.0);
}

/** In a column of indefinite height, an item with an indefinite flex basis is not definite, so
its `height: 100%` child stays content-sized even though the item has an authored height. */
#[test]
fn indefinite_basis_leaves_percentages_unresolved() {
    let markup = "<div style='display:flex;flex-direction:column'>\
         <div style='flex:1 1 content;height:100px;min-height:0'>\
         <div style='height:100px'></div><div id=p style='height:100%'></div></div></div>";
    assert_eq!(rect(markup, "p").3, 0.0);
}

/** A row item's percentage height resolves against the container's definite height. */
#[test]
fn row_item_percent_height_resolves() {
    let markup = "<div style='display:flex;width:100px;height:200px'>\
         <div id=a style='height:50%;flex:1'></div></div>";
    assert_eq!(rect(markup, "a").3, 100.0);
}

/** A stretched replaced item takes its width from its natural ratio and the line's height. */
#[test]
fn stretched_canvas_width_follows_ratio() {
    let markup = "<div style='display:flex;width:300px;height:100px'>\
         <canvas id=a width=20 height=10 style='min-width:0'></canvas></div>";
    assert_eq!(rect(markup, "a").2, 200.0);
}

/** A text item's automatic minimum in a column is its text height, not its authored height, so
50px items shrink into a 100px column. */
#[test]
fn text_items_shrink_to_their_lines() {
    let markup = "<div style='display:flex;flex-direction:column;height:100px;width:100px'>\
         <div id=a style='height:50px'>1</div><div style='height:50px'>2</div>\
         <div style='height:50px'>3</div><div style='height:50px'>4</div></div>";
    assert_eq!(rect(markup, "a").3, 25.0);
}

/** A `calc()` basis that resolves below zero is clamped to zero, as css-values-4 range checking
requires: in a 400px row, `calc(10px - 50%)` is -190px and takes no space, so the two items split
the row evenly at 200px each. */
#[test]
fn a_negative_calc_basis_clamps_to_zero() {
    let markup = "<div style='display:flex;width:400px'>\
         <i id=a style='display:block;background:red;flex:1 1 calc(10px - 50%);min-width:0'></i>\
         <i id=b style='display:block;background:red;flex:1 1 0px;min-width:0'></i></div>";
    assert_eq!(rect(markup, "a").2, 200.0);
    assert_eq!(rect(markup, "b").2, 200.0);
}
