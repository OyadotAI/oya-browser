/** css-multicol rules Eye implements: used column count and width, balancing, filling, slicing
a box across columns, `column-span: all` and column rules. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document with no body margin, laid out in an 800px viewport. */
fn laid(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &["body{margin:0}".to_owned()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** Every laid-out fragment of the element with this id, in paint order. */
fn fragments<'a>(document: &Document, display: &'a DisplayList, id: &str) -> Vec<&'a DisplayItem> {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .filter(|item| item.local_id == local && !item.tag.starts_with("::"))
        .collect()
}

/** (x, y, height) of the element's first fragment. */
fn spot(document: &Document, display: &DisplayList, id: &str) -> (f64, f64, f64) {
    let item = fragments(document, display, id)[0];
    (item.x, item.y, item.height)
}

/** `column-count` with an auto width splits the content box evenly around the gaps, and
balancing puts equal blocks side by side, the container as tall as one column. */
#[test]
fn count_balances_blocks_into_equal_columns() {
    let (document, display) = laid(
        "<div id=m style='columns:2;column-gap:20px;width:220px'>\
         <div id=a style='height:50px'></div><div id=b style='height:50px'></div></div>",
    );
    assert_eq!(spot(&document, &display, "a"), (0.0, 0.0, 50.0));
    assert_eq!(spot(&document, &display, "b"), (120.0, 0.0, 50.0));
    assert_eq!(fragments(&document, &display, "a")[0].width, 100.0);
    assert_eq!(spot(&document, &display, "m").2, 50.0);
}

/** `column-width` gives as many columns as fit, widened to share the leftover room. */
#[test]
fn width_fits_as_many_columns_as_room_allows() {
    let (document, display) = laid(
        "<div style='column-width:90px;column-gap:10px;width:300px'>\
         <div id=a style='height:30px'></div><div id=b style='height:30px'></div>\
         <div id=c style='height:30px'></div></div>",
    );
    let width = fragments(&document, &display, "a")[0].width;
    assert!(
        (width - 280.0 / 3.0).abs() < 0.01,
        "three columns, got {width}"
    );
    assert!(spot(&document, &display, "c").0 > 180.0);
}

/** `column-gap: normal` is 1em in a multicol container. */
#[test]
fn normal_gap_is_one_em() {
    let (document, display) = laid(
        "<div style='columns:2;width:216px;font-size:16px'>\
         <div id=a style='height:10px'></div><div id=b style='height:10px'></div></div>",
    );
    assert_eq!(spot(&document, &display, "b").0, 116.0);
}

/** A block taller than one column is sliced: one clipped copy per column it crosses. */
#[test]
fn tall_block_slices_across_columns() {
    let (document, display) = laid(
        "<div style='columns:2;column-gap:0;width:100px;height:100px'>\
         <div id=t style='height:200px;background:green'></div></div>",
    );
    let pieces = fragments(&document, &display, "t");
    assert_eq!(pieces.len(), 2);
    assert_eq!((pieces[1].x, pieces[1].y), (50.0, -100.0));
    let clip = pieces[1].clip.as_ref().unwrap();
    assert_eq!(clip.y, 0.0);
}

/** `column-fill: auto` with a definite height fills the first column before the next. */
#[test]
fn fill_auto_fills_columns_in_turn() {
    let (document, display) = laid(
        "<div style='columns:3;column-gap:0;column-fill:auto;width:300px;height:100px'>\
         <div id=a style='height:60px'></div><div id=b style='height:60px'></div></div>",
    );
    assert_eq!(spot(&document, &display, "a").0, 0.0);
    assert_eq!(
        fragments(&document, &display, "b").len(),
        2,
        "b crosses the break at 100px"
    );
}

/** A `column-span: all` child spans the content box between two balanced rows. */
#[test]
fn spanner_separates_column_rows() {
    let (document, display) = laid(
        "<div style='columns:2;column-gap:0;width:200px'>\
         <div style='height:40px'></div><div id=s style='column-span:all;height:10px'></div>\
         <div id=after style='height:40px'></div></div>",
    );
    let (x, y, _) = spot(&document, &display, "s");
    assert_eq!((x, y), (0.0, 20.0));
    assert_eq!(fragments(&document, &display, "s")[0].width, 200.0);
    assert_eq!(spot(&document, &display, "after").1, 30.0);
}

/** A rule paints centered in each gap between used columns, as tall as the columns. */
#[test]
fn rule_paints_in_the_gap() {
    let (_, display) = laid(
        "<div style='columns:2;column-gap:20px;width:220px;column-rule:4px solid red'>\
         <div style='height:30px'></div><div style='height:30px'></div></div>",
    );
    let rules: Vec<_> = display
        .items
        .iter()
        .filter(|item| item.tag == "::column-rule")
        .collect();
    assert_eq!(rules.len(), 1);
    assert_eq!(
        (rules[0].x, rules[0].y, rules[0].height),
        (108.0, 0.0, 30.0)
    );
    assert_eq!(rules[0].border_widths[3], 4.0);
}

/** Lines never split: a break between them moves the next line whole to the next column. */
#[test]
fn lines_move_whole_to_the_next_column() {
    let (document, display) = laid(
        "<div id=m style='columns:2;column-gap:0;width:200px;line-height:20px;font-size:10px'>\
         a<br>b<br>c</div>",
    );
    assert_eq!(spot(&document, &display, "m").2, 40.0);
}

/** css-multicol-2 `column-height` with `column-wrap: wrap` stacks rows of N columns, each
`column-height` tall, separated by the row gap. */
#[test]
fn column_height_wraps_rows() {
    let (document, display) = laid(
        "<div id=m style='columns:2;column-fill:auto;width:100px;gap:10px 0;\
         column-height:50px;column-wrap:wrap'><div id=t style='height:200px'></div></div>",
    );
    let pieces = fragments(&document, &display, "t");
    let tops: Vec<_> = pieces.iter().map(|p| (p.x, p.y)).collect();
    assert_eq!(
        tops,
        [(0.0, 0.0), (50.0, -50.0), (0.0, -40.0), (50.0, -90.0)]
    );
    assert_eq!(spot(&document, &display, "m").2, 110.0);
}
