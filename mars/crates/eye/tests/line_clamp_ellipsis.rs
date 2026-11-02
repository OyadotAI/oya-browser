/** Rules for the line-clamp ellipsis and the boxes a clamp or `visibility` hides: the marker is
its own run in the line's root style, a line with no wrap opportunity keeps nothing, out-of-flow
boxes go with their containing block, and `visibility: hidden` keeps its room. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document with no body margin and the given sheet. */
fn styled(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let sheets = [format!("body{{margin:0}}{sheet}")];
    let document = compute_styles(&parse(markup).unwrap(), &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The display item of the element with this id. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display.items.iter().find(|i| i.local_id == local).unwrap()
}

/** The ellipsis is styled by the root inline box: it is a run on the block's own item, not glued
into the span's run, and it starts after the span's closing border. */
#[test]
fn block_ellipsis_is_its_own_run_in_the_line_root_style() {
    let (document, display) = styled(
        "<div id=c>One<br><span id=s>Two</span><br>Three</div>",
        "#c{line-clamp:2}#s{font-size:2em;border-right:5px solid}",
    );
    let span = item(&document, &display, "s");
    assert_eq!(span.lines[0].text, "Two");
    let clamp = item(&document, &display, "c");
    let marker = clamp.lines.iter().find(|l| l.text == "\u{2026}").unwrap();
    let span_end = span.lines[0].x + span.lines[0].width;
    assert!((marker.x - (span_end + 5.0)).abs() < 0.01);
}

/** Text that does not wrap has no opportunity to break before the marker, so a line too long
for it keeps nothing and the marker starts the line. */
#[test]
fn nowrap_line_without_room_keeps_only_the_marker() {
    let (document, display) = styled(
        "<div id=c>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nb</div>",
        "#c{line-clamp:1;white-space:pre;width:50px}",
    );
    let clamp = item(&document, &display, "c");
    assert_eq!(clamp.lines[0].text, "");
    let marker = clamp.lines.iter().find(|l| l.text == "\u{2026}").unwrap();
    assert_eq!(marker.x, clamp.x);
}

/** An empty `block-ellipsis` adds nothing and cuts nothing from the clamp line. */
#[test]
fn empty_block_ellipsis_leaves_the_line_whole() {
    let (document, display) = styled(
        "<div id=c>supercalifragilistic<br>gone</div>",
        "#c{line-clamp:1 \"\";width:20px}",
    );
    let clamp = item(&document, &display, "c");
    assert_eq!(clamp.lines[0].text, "supercalifragilistic");
}

/** An absolutely positioned box whose containing block lies after the clamp point is hidden
with it; layout's hiding survives the opacity pass. */
#[test]
fn abspos_after_the_clamp_point_is_hidden() {
    let (document, display) = styled(
        "<div id=c>One<br>Two<div style='position:relative'>\
         <div id=a style='position:absolute;width:9px;height:9px'></div>Three</div></div>",
        "#c{line-clamp:2}",
    );
    assert_eq!(item(&document, &display, "a").opacity, 0.0);
}

/** `visibility: hidden` keeps the box's room and paints nothing; a visible descendant shows,
and agents still treat the hidden box as hidden. */
#[test]
fn visibility_hidden_keeps_room_and_paints_nothing() {
    let (document, display) = styled(
        "<div id=h style='visibility:hidden;height:30px'>x<span id=v \
         style='visibility:visible'>y</span></div><div id=after>z</div>",
        "",
    );
    assert_eq!(item(&document, &display, "h").opacity, 0.0);
    assert_eq!(item(&document, &display, "v").opacity, 1.0);
    assert_eq!(item(&document, &display, "after").y, 30.0);
    let hidden = document.nodes.iter().find(|n| n.html_id == "h").unwrap();
    assert!(hidden.hidden() && !hidden.removed());
}

/** An inline table sits on its first row's baseline, not on its last line's. */
#[test]
fn inline_table_aligns_on_its_first_line() {
    let (document, display) = styled(
        "<p id=p>a<span id=t style='display:inline-table'>b<span style='display:block'>c\
         </span></span></p>",
        "",
    );
    let p = item(&document, &display, "p");
    let table = item(&document, &display, "t");
    assert_eq!(p.lines[0].y, table.lines[0].y);
}

/** Atomic inlines are line content: a line holding only inline-blocks counts, and those after
the clamp point are hidden. */
#[test]
fn atomic_inlines_count_as_lines_and_hide_after_the_clamp() {
    let (document, display) = styled(
        "<div id=c><span class=b></span><span class=b id=gone></span></div>",
        "#c{line-clamp:1;width:60px}.b{display:inline-block;width:50px;height:20px}",
    );
    assert_eq!(item(&document, &display, "gone").opacity, 0.0);
    assert_eq!(item(&document, &display, "c").height, 20.0);
}

/** A block after the clamp line clamps even when it has no lines of its own. */
#[test]
fn a_lineless_block_after_the_clamp_line_is_clamped() {
    let (document, display) = styled(
        "<div id=c>One<div id=gone style='height:30px'></div></div>",
        "#c{line-clamp:1}",
    );
    assert_eq!(item(&document, &display, "gone").opacity, 0.0);
    let clamp = item(&document, &display, "c");
    assert!(clamp.lines.iter().any(|l| l.text == "\u{2026}"));
}

/** With `line-clamp: auto`, a block that fits before the clamp point means the point follows
no line, so no ellipsis is drawn. */
#[test]
fn auto_clamp_after_a_block_draws_no_ellipsis() {
    let (document, display) = styled(
        "<div id=c>One<br>Two<div></div>Three</div>",
        "#c{line-clamp:auto;line-height:20px;max-height:40px}",
    );
    let clamp = item(&document, &display, "c");
    assert!(clamp.lines.iter().all(|l| l.text != "\u{2026}"));
}

/** A box between the container and the clamp line keeps its child's bottom margin when its own
border stops that margin collapsing. */
#[test]
fn clamped_parent_keeps_the_child_margin_its_border_holds() {
    let (document, display) = styled(
        "<div id=c><div id=o class=i><div class=i>One<br>Two</div></div></div>",
        "#c{line-clamp:1;line-height:20px}.i{margin:4px;border:1px solid}",
    );
    // Inner: border 1 + line 20 + border 1; outer adds its margin 4 and its own borders.
    assert_eq!(
        item(&document, &display, "o").height,
        1.0 + 4.0 + 22.0 + 4.0 + 1.0
    );
}

/** The marker is placed before alignment: a right-aligned clamp line ends with the marker at
the content edge. */
#[test]
fn right_aligned_clamp_line_ends_with_the_marker() {
    let (document, display) = styled(
        "<div id=c>One<br>Two</div>",
        "#c{line-clamp:1;text-align:right;width:200px}",
    );
    let clamp = item(&document, &display, "c");
    let marker = clamp.lines.iter().find(|l| l.text == "\u{2026}").unwrap();
    assert!((marker.x + marker.width - 200.0).abs() < 0.01);
}
