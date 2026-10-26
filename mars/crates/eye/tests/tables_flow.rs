/** Block, inline and table flow beside floats and inside inline boxes: paint order after a block
in an inline box, empty inline boxes on a line, intrinsic widths split by blocks, formatting
contexts and atomics that fit floats over their height, clearance a child's margin makes
unneeded, and table boxes made of columns, row text or their own text. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at 800px with no body margin. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** Every item painted for the element with `id`, in paint order. */
fn all<'a>(document: &Document, display: &'a DisplayList, id: &str) -> Vec<&'a DisplayItem> {
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
/** The first item painted for `id`. */
fn first<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    all(document, display, id)[0]
}

/** An inline box's fragment after a block inside it paints after the block's content. */
#[test]
fn fragment_after_a_block_paints_after_it() {
    let (document, display) =
        layout("<span id=s style='border:2px solid'>a<div id=d>b</div>c</span>");
    let pieces = all(&document, &display, "s");
    let block = first(&document, &display, "d");
    let index = |item: &DisplayItem| display.items.iter().position(|i| std::ptr::eq(i, item));
    assert!(pieces.last().unwrap().paint_after.is_some());
    assert!(pieces[0].paint_after.is_none());
    assert!(index(block).is_some());
}

/** An empty inline box's line-height still counts on a line with other content (empty-inline-003). */
#[test]
fn empty_inline_box_raises_its_line() {
    let (document, display) =
        layout("<div id=d style='line-height:20px'><span style='line-height:100px'></span>X</div>");
    assert_eq!(first(&document, &display, "d").height, 100.0);
}

/** An empty inline box alone makes no line. */
#[test]
fn empty_inline_box_alone_makes_no_line() {
    let (document, display) = layout("<div id=d><span style='line-height:100px'></span></div>");
    assert_eq!(first(&document, &display, "d").height, 0.0);
}

/** Inline content split by a block is as wide as its widest run (inline-table-valign-001). */
#[test]
fn intrinsic_width_takes_the_widest_run_between_blocks() {
    let (document, display) = layout(
        "<div id=f style='float:left;font:16px/1 monospace'><span>aaaa</span>\
         <div>b</div><span>cc</span></div>",
    );
    let width = first(&document, &display, "f").width;
    assert!(width > 30.0 && width < 50.0, "{width}");
}

/** A formatting context that grows tall enough to meet a wider float takes the band that float
leaves (floats-bfc-003). */
#[test]
fn formatting_context_refits_beside_lower_floats() {
    let (document, display) = layout(
        "<div style='width:300px'><div style='float:left;clear:left;width:100px;height:25px'></div>\
         <div style='float:left;clear:left;width:200px;height:25px'></div>\
         <div id=b style='overflow:hidden'><div style='float:left;width:100px;height:50px'></div>\
         <div style='float:left;width:100px;height:50px'></div></div></div>",
    );
    let bfc = first(&document, &display, "b");
    assert_eq!((bfc.x, bfc.width, bfc.height), (200.0, 100.0, 100.0));
}

/** An inline-block on a new line goes below a lower float it would overlap
(floats-wrap-top-below-inline-002l). */
#[test]
fn atomic_inline_fits_floats_over_its_height() {
    let (document, display) = layout(
        "<div style='width:400px'><div style='float:left;width:150px;height:75px'></div>\
         <div style='float:right;width:300px;height:75px'></div>\
         <span style='display:inline-block;width:200px;height:50px'></span>\
         <span id=s style='display:inline-block;width:200px;height:50px'></span></div>",
    );
    assert_eq!(first(&document, &display, "s").y, 150.0);
}

/** Side margins wider than the room beside a float leave no room there
(floats-wrap-bfc-with-margin-005). */
#[test]
fn margins_wider_than_the_room_move_below_the_float() {
    let (document, display) = layout(
        "<div style='width:100px;display:flow-root'><div style='float:left;width:50px;height:40px'></div>\
         <div id=b style='display:flow-root;height:60px;margin-right:51px'></div></div>",
    );
    assert_eq!(first(&document, &display, "b").y, 40.0);
}

/** A child's top margin that already passes the float makes clearance unneeded
(margin-collapse-clear-015). */
#[test]
fn child_margin_can_make_clearance_unneeded() {
    let (document, display) = layout(
        "<div style='border-top:1px solid'><div style='float:left;width:10px;height:100px'></div>\
         <div style='clear:left'><div id=c style='margin-top:140px;height:10px'></div></div></div>",
    );
    assert_eq!(first(&document, &display, "c").y, 141.0);
}

/** A table of columns alone paints none of them (table-column-rendering-001). */
#[test]
fn columns_alone_paint_nothing() {
    let (document, display) = layout(
        "<div style='display:table'><div id=c style='display:table-column;background:red;\
         width:50px;height:50px'></div></div>",
    );
    assert!(all(&document, &display, "c").is_empty());
}

/** A CSS row holding only text lays the text out as its one cell
(table-vertical-align-baseline-009). */
#[test]
fn row_text_is_its_cell() {
    let (document, display) =
        layout("<div style='display:table'><div id=r style='display:table-row'>X</div></div>");
    assert!(
        all(&document, &display, "r")
            .iter()
            .any(|item| !item.lines.is_empty())
    );
}

/** The space before an atomic inline is its parent's text, so `font-size: 0` makes it zero. */
#[test]
fn space_before_an_atomic_takes_the_parents_font() {
    let (document, display) = layout(
        "<div style='font-size:0'><span id=a style='display:inline-block;width:10px;height:10px'>\
         </span> <span id=b style='display:inline-block;width:10px;height:10px;font-size:50px'></span></div>",
    );
    assert_eq!(first(&document, &display, "b").x, 10.0);
}

/** A `display: table` holding only its own text shrinks to it (inline-table-height-002-ref). */
#[test]
fn table_of_text_shrinks_to_fit() {
    let (document, display) = layout("<div id=t style='display:table'>text</div>");
    assert!(first(&document, &display, "t").width < 100.0);
}

/** An element standing in for a cell adds its margins to the cell, a percentage resolved against
zero for the table's width (calc-margins-table-caption). */
#[test]
fn stand_in_cell_margins_resolve_percentages_against_zero() {
    let (document, display) = layout(
        "<div id=t style='display:table;width:min-content;height:100px'>\
         <div style='margin-left:calc(10% + 100px)'></div></div>",
    );
    assert_eq!(first(&document, &display, "t").width, 100.0);
}

/** Clearance is measured against the margins that collapse through the cleared block's top, a
child's negative one included, and that child's margin stays inside the block
(clear-on-parent-with-margins). */
#[test]
fn clearance_counts_a_childs_negative_margin() {
    let (document, display) = layout(
        "<div style='width:200px;height:200px'>\
         <div style='float:left;width:200px;height:100px'></div>\
         <div style='clear:left;margin-top:100px'>\
         <div id=c style='height:100px;margin-top:-1000px'></div></div></div>",
    );
    assert_eq!(first(&document, &display, "c").y, 100.0);
}

/** A block's positive text-indent widens its max-content by the indent and its min-content to
the first word past the indent (text-indent-intrinsic-001). */
#[test]
fn text_indent_widens_intrinsic_widths() {
    let (document, display) = layout(
        "<div id=a style='float:left;font:20px Ahem'>X</div><br style='clear:left'>\
         <div id=b style='float:left;font:20px Ahem;text-indent:60px'>X</div>",
    );
    let width = |id: &str| first(&document, &display, id).width;
    assert_eq!(width("b"), width("a") + 60.0);
}

/** `capitalize` leaves a word that an earlier element began alone: only a letter after a space
or a non-letter starts a word (text-transform-capitalize-033). */
#[test]
fn capitalize_continues_a_word_across_elements() {
    let (document, display) =
        layout("<p style='text-transform:capitalize'>ab<span id=s>cd</span> ef</p>");
    let text: String = all(&document, &display, "s")
        .iter()
        .flat_map(|item| item.lines.iter().map(|line| line.text.clone()))
        .collect();
    assert_eq!(text, "cd");
}
