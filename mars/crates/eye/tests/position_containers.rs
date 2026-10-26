/** Positioning rules the `position` lane implemented: sticky containing blocks, blockified table
 * parts, the root and body boxes, abspos static positions in flex containers, nowrap atomic
 * inlines and the text indent of a blockified inline. */
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

/** A sticky cell travels within its whole table, not just its row (position-sticky-table-td-top):
 * scrolled 225px, a `top: 25px` cell in the third 50px row moves from 200 down to 250. */
#[test]
fn a_sticky_cell_sticks_within_its_table_not_its_row() {
    let markup = "<div id=s><div id=pre></div><table><tr><td><div></div></td></tr>\
                  <tr><td><div></div></td></tr><tr><td id=c><div></div></td></tr>\
                  <tr><td><div></div></td></tr></table><div id=post></div></div>";
    let sheet = "body{margin:0}table{border-collapse:collapse}td{padding:0}td>div{height:50px;width:50px}\
                 #s{width:100px;height:150px;overflow:auto}#pre{height:100px}#post{height:250px}\
                 #c{position:sticky;top:25px}";
    let (document, display) = laid(markup, sheet, &[("s", [0.0, 225.0])]);
    assert_eq!(at(&document, &display, "c").1, 250.0 - 225.0);
}

/** An absolutely positioned `display: table-row-group` is blockified (CSS 2.1 9.7), so its cell
 * paints instead of waiting for a table (CSS2 top-applies-to-001). */
#[test]
fn an_abspos_table_part_is_blockified_and_paints() {
    let markup = "<div id=t><div id=g><div id=r><div id=c></div></div></div></div>";
    let sheet = "#t{display:table}#g{display:table-row-group;position:absolute;top:0}\
                 #r{display:table-row}#c{display:table-cell;width:96px;height:96px}";
    let (document, display) = laid(markup, sheet, &[]);
    let cell = local(&document, "c");
    let item = display.items.iter().find(|i| i.local_id == cell).unwrap();
    assert_eq!((item.width, item.height, item.y), (96.0, 96.0, 0.0));
}

/** A positioned, sized root moves its border box by `left`/`top` and keeps its authored width
 * (abspos-containing-block-initial-004a). */
#[test]
fn a_positioned_sized_root_places_its_border_box() {
    let sheet = "html{position:absolute;left:100px;top:100px;width:100px;height:100px;\
                 border:10px solid black}";
    let (_, display) = laid("<p>x</p>", sheet, &[]);
    let root = display.root_decoration.unwrap();
    assert_eq!(
        (root.x, root.y, root.width, root.height),
        (100.0, 100.0, 120.0, 120.0)
    );
}

/** An absolutely positioned body is placed by its insets in the viewport. */
#[test]
fn an_abspos_body_is_placed_by_its_insets() {
    let sheet = "body{position:absolute;left:100px;top:50px;margin:0;width:100px;height:100px;\
                 border:10px solid black}";
    let (_, display) = laid("<div id=d>x</div>", sheet, &[]);
    let body = display.body_decoration.unwrap();
    assert_eq!((body.x, body.y), (100.0, 50.0));
}

/** An abspos child of a centered flex container sits centered, as the sole flex item would
 * (CSS Flexbox 4.1; position-absolute-center-001). */
#[test]
fn an_abspos_flex_child_takes_the_containers_alignment() {
    let markup = "<div id=f><div id=a></div></div>";
    let sheet = "body{margin:0}#f{display:flex;position:relative;width:100px;height:100px;\
                 justify-content:center;align-items:flex-end}\
                 #a{position:absolute;width:40px;height:20px}";
    let (document, display) = laid(markup, sheet, &[]);
    assert_eq!(at(&document, &display, "a"), (30.0, 80.0));
}

/** A `white-space: nowrap` row of inline-blocks stays on one line and overflows its block
 * (position-sticky-left-002). */
#[test]
fn nowrap_keeps_inline_blocks_on_one_line() {
    let markup = "<div id=w><div id=a></div><div id=b></div></div>";
    let sheet = "body{margin:0}#w{width:100px;white-space:nowrap}\
                 #a,#b{display:inline-block;width:80px;height:10px}";
    let (document, display) = laid(markup, sheet, &[]);
    assert_eq!(
        at(&document, &display, "a").1,
        at(&document, &display, "b").1
    );
}

/** A blockified inline (absolutely positioned `display: inline`) indents its own first line by its
 * own `text-indent`, not its parent's (static-position/htb-ltr-ltr). */
#[test]
fn an_abspos_inline_does_not_take_its_parents_indent() {
    let markup = "<div id=p>XX<span id=a>YY</span></div>";
    let sheet =
        "body{margin:0}#p{text-indent:20px}#a{display:inline;position:absolute;text-indent:0}";
    let (document, display) = laid(markup, sheet, &[]);
    let id = local(&document, "a");
    let item = display.items.iter().find(|i| i.local_id == id).unwrap();
    assert_eq!(item.lines[0].x, item.x);
}

/** An absolutely positioned box reaches as far as it paints: the scroller's end padding is added
 * to in-flow content only (position-sticky-padding-001). */
#[test]
fn abspos_overflow_gets_no_end_padding() {
    let markup = "<div id=s><div id=a></div></div>";
    let sheet = "body{margin:0}#s{position:relative;width:100px;height:100px;padding:10px;\
                 overflow:auto}#a{position:absolute;top:300px;width:10px;height:10px}";
    let (_, display) = laid(markup, sheet, &[]);
    assert_eq!(display.scrollers[0].max_y, 310.0 - 120.0);
}

/** A sticky cell inside a sticky header starts from where the header moved it, so the two
 * offsets do not add up (position-sticky-nested-thead-th). */
#[test]
fn a_nested_sticky_box_starts_from_its_sticky_ancestor() {
    let markup = "<div id=s><div id=pre></div><table><thead id=h><tr><th id=c></th></tr></thead>\
                  <tbody><tr><td></td></tr><tr><td></td></tr></tbody></table><div id=post></div></div>";
    let sheet = "body{margin:0}table{border-collapse:collapse}td,th{height:50px;width:50px;padding:0}\
                 #s{width:100px;height:200px;overflow:auto}#pre{height:100px}#post{height:600px}\
                 #h,#c{position:sticky;top:25px}";
    let (document, display) = laid(markup, sheet, &[("s", [0.0, 125.0])]);
    assert_eq!(at(&document, &display, "c").1, 25.0);
}

/** A sticky box under an absolute box that escapes a non-positioned scroller sticks to the
 * viewport, not that scroller (position-sticky-escape-scroller-003). */
#[test]
fn a_scroller_the_abspos_ancestor_escapes_is_not_the_sticky_scroller() {
    let markup = "<div id=s><div id=r></div><div id=a><div id=k></div></div><div id=t></div></div>";
    let sheet = "body{margin:0}#s{width:200px;height:100px;overflow:hidden}#r{height:100px}\
                 #a{position:absolute;height:400px}#k{position:sticky;top:0;width:200px;height:100px}\
                 #t{height:1000px}";
    let (document, display) = laid(markup, sheet, &[("s", [0.0, 200.0])]);
    assert_eq!(at(&document, &display, "k").1, 100.0);
}

/** Scrollable overflow counts a sticky box where layout put it, so a `bottom: 0` item pulled up
 * into view still leaves its scroller room to scroll (position-sticky-flex-item-001). */
#[test]
fn a_sticky_offset_does_not_hide_scrollable_overflow() {
    let markup = "<div id=f><div id=n></div><div id=s></div></div>";
    let sheet = "body{margin:0}#f{overflow:hidden;width:100px;height:100px;display:flex;\
                 flex-direction:column}#n{flex:0 0 80px}#s{position:sticky;flex:0 0 100px;bottom:0}";
    let (document, display) = laid(markup, sheet, &[("f", [0.0, 1000.0])]);
    assert_eq!(display.scrollers[0].max_y, 80.0);
    assert_eq!(at(&document, &display, "n").1, -80.0);
    assert_eq!(at(&document, &display, "s").1, 0.0);
}

/** A `display: table` root with an auto width shrinks to its content and centers by its auto
 * margins (css-tables html-display-table). */
#[test]
fn a_table_root_shrinks_to_fit_its_content() {
    let markup = "<div id=a></div><div id=b></div>";
    let sheet = "html{display:table;border:10px solid green;margin:auto}body{margin:0}\
                 #a{display:inline-block;width:200px;height:30px}\
                 #b{display:inline-block;width:80px;height:30px}";
    let (_, display) = laid(markup, sheet, &[]);
    let root = display.root_decoration.unwrap();
    assert_eq!((root.x, root.width), (250.0, 300.0));
}
