/** Vertical writing modes, writing-mode-aware logical properties, line bidi ordering and
right-to-left static positions, as the css-writing-modes and CSS2 reftests check them. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A document styled with `css` (body margin 0) laid out at 800 points. */
fn laid(markup: &str, css: &str) -> (Document, DisplayList) {
    let css = format!("body{{margin:0}}{css}");
    let document = compute_styles(&parse(markup).unwrap(), &[css]).unwrap();
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

/** A vertical-rl box swaps its axes: its authored height is the line length, its width the
stacked lines', its first line sits at its right edge, and its text is drawn turned. */
#[test]
fn vertical_rl_lines_stack_from_the_right() {
    let (document, display) = laid(
        "<div id=v>ab cd</div>",
        "#v{writing-mode:vertical-rl;height:100px;font:20px/1 monospace}",
    );
    let v = item(&document, &display, "v");
    assert_eq!((v.width, v.height), (20.0, 100.0));
    assert_eq!(v.lines[0].x, v.x);
    assert_ne!(v.writing, 0);
}

/** A vertical-lr box's left border stays on the left after the rotation. */
#[test]
fn vertical_lr_keeps_physical_borders() {
    let (document, display) = laid(
        "<div id=v>x</div>",
        "#v{writing-mode:vertical-lr;height:50px;border-left:3px solid red}",
    );
    let v = item(&document, &display, "v");
    assert_eq!(v.border_widths[3], 3.0);
}

/** `block-size` is the width and `margin-block-start` the left margin in vertical-lr. */
#[test]
fn logical_properties_follow_the_writing_mode() {
    let (document, display) = laid(
        "<div id=v>x</div>",
        "#v{writing-mode:vertical-lr;block-size:40px;inline-size:60px}",
    );
    let v = item(&document, &display, "v");
    assert_eq!((v.width, v.height), (40.0, 60.0));
    let style = &document.nodes[0].style;
    assert_eq!(style.width, Some(40.0));
}

/** In a right-to-left paragraph inline boxes run right to left: the first image is rightmost. */
#[test]
fn rtl_lines_order_atomics_right_to_left() {
    let (document, display) = laid(
        "<div dir=rtl><span id=a></span><span id=b></span></div>",
        "span{display:inline-block;width:10px;height:10px}",
    );
    let (a, b) = (
        item(&document, &display, "a"),
        item(&document, &display, "b"),
    );
    assert!(a.x > b.x, "{} {}", a.x, b.x);
    assert_eq!(a.x + a.width, 800.0);
}

/** An absolutely positioned box after the content of a right-to-left line ends at the line's
left edge, its right edge on the static position. */
#[test]
fn rtl_static_position_is_the_boxes_right_edge() {
    let (document, display) = laid(
        "<div dir=rtl style='position:relative'><span id=a></span><i id=p></i></div>",
        "#a{display:inline-block;width:100px;height:10px}\
         #p{position:absolute;width:20px;height:20px}",
    );
    let p = item(&document, &display, "p");
    assert_eq!(p.x + p.width, 700.0);
}

/** A table taller than its rows gives the rows the extra height. */
#[test]
fn table_height_grows_its_rows() {
    let (document, display) = laid(
        "<table style='height:100px;border-spacing:0'><tr id=a><td>a</td></tr>\
         <tr id=b><td>b</td></tr></table>",
        "",
    );
    let (a, b) = (
        item(&document, &display, "a"),
        item(&document, &display, "b"),
    );
    assert_eq!(a.height + b.height, 100.0);
}

/** A `unicode-bidi: normal` inline's own `direction` does nothing: its run is painted in its
paragraph's direction, while an isolating `dir` span keeps its own. */
#[test]
fn a_normal_inline_paints_in_its_paragraph_direction() {
    let (document, display) = laid(
        "<div><span id=n style='direction:rtl'>a</span><span id=d dir=rtl>b</span></div>",
        "",
    );
    assert!(!item(&document, &display, "n").rtl);
    assert!(item(&document, &display, "d").rtl);
}

/** A number after right-to-left text in an LTR paragraph stays a number (rule W7) and the
space before it goes right to left with it (N1): `א 3` displays as `3 א`, the gap kept. */
#[test]
fn a_number_after_hebrew_reverses_with_it_and_keeps_the_space() {
    let (document, display) = laid(
        "<div><span id=h>\u{5d0}</span><span id=n> 3</span></div>",
        "",
    );
    let (h, n) = (
        item(&document, &display, "h"),
        item(&document, &display, "n"),
    );
    let digit = n.lines.iter().find(|l| l.text.contains('3')).unwrap();
    assert!(digit.x + digit.width < h.lines[0].x);
    assert!(
        h.lines[0].x - (digit.x + digit.width) > 1.0,
        "the space stays between"
    );
}

/** An isolate is one neutral to the text around it: Hebrew isolated before a number does not
pull the number right to left. */
#[test]
fn an_isolate_counts_as_a_neutral_outside() {
    let (document, display) = laid(
        "<div><span id=h style='direction:rtl;unicode-bidi:isolate'>\u{5d0}</span>\
         <span id=n> 3</span></div>",
        "",
    );
    let (h, n) = (
        item(&document, &display, "h"),
        item(&document, &display, "n"),
    );
    let digit = n.lines.iter().find(|l| l.text.contains('3')).unwrap();
    assert!(h.lines[0].x < digit.x);
}

/** A run's leading neutral that sits between left-to-right text in an RTL paragraph gets a
left-to-right mark, so the shaper does not resolve it against the paragraph and mirror it. */
#[test]
fn edge_neutrals_get_the_direction_of_their_neighbour() {
    let (document, display) = laid(
        "<div dir=rtl><span id=a>a</span><span id=b> &gt; b</span></div>",
        "",
    );
    let b = item(&document, &display, "b");
    assert!(b.lines.iter().any(|l| l.text.starts_with('\u{200E}')));
}

/** An absolutely positioned box whose containing block is the viewport sits at its insets in
the viewport, however much padding a vertical root has. */
#[test]
fn viewport_abspos_in_a_vertical_root_ignores_its_padding() {
    let (document, display) = laid(
        "<div id=p>x</div>",
        "html{padding:100px 0;writing-mode:vertical-rl}\
         #p{position:absolute;left:8px;top:100px;writing-mode:horizontal-tb}",
    );
    let p = item(&document, &display, "p");
    assert_eq!((p.x, p.y), (8.0, 100.0));
}

/** Horizontal text inside a vertical root starts at its box's content edge: a line turned
twice keeps its own extent. */
#[test]
fn twice_turned_text_starts_at_its_box() {
    let (document, display) = laid(
        "<div id=p>Hi</div>",
        "html{writing-mode:vertical-rl}#p{writing-mode:horizontal-tb;line-height:30px}",
    );
    let p = item(&document, &display, "p");
    assert_eq!(p.lines[0].x, p.x);
}

/** An orthogonal box in a container of definite height shrinks to fit that room, minus its own
margins, rather than filling it. */
#[test]
fn orthogonal_box_shrinks_to_fit_the_definite_room() {
    let (document, display) = laid(
        "<div style='height:200px;width:500px'><div id=v>ab</div></div>",
        "#v{writing-mode:vertical-lr;margin:50px;font:20px/1 monospace}",
    );
    let v = item(&document, &display, "v");
    assert!(
        v.height <= 100.0,
        "fits inside 200px less 100px of margins: {}",
        v.height
    );
}

/** A writing-mode root holds its floats: a horizontal box in a vertical one is as tall as the
float inside it. */
#[test]
fn a_writing_mode_root_contains_its_floats() {
    let (document, display) = laid(
        "<div id=v><div id=h><div style='float:left;width:50px;height:40px'></div></div></div>",
        "#v{writing-mode:vertical-rl;height:100px}#h{writing-mode:horizontal-tb}",
    );
    assert_eq!(item(&document, &display, "h").height, 40.0);
}
