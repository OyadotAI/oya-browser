/** Border widths as Blink's `StyleBuilderConverter::ConvertBorderWidth` computes them, and table
 * part backgrounds as `TableSectionPainter` and `TableColumnPainter` paint them. */
use mars_eye::{DisplayItem, StyleSource, compute_styles, parse, render};

/** Every display item of `markup`, with its inline style sheets applied. */
fn items(markup: &str) -> Vec<DisplayItem> {
    let parsed = parse(markup).unwrap();
    let sheets: Vec<String> = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    render(&compute_styles(&parsed, &sheets).unwrap(), 800.0)
        .unwrap()
        .items
}

/** A wide border paints whole: there is no cap below the geometry bound. */
#[test]
fn wide_border_widths_are_not_capped() {
    let markup = "<div style='border-top:96px solid;height:0'></div>";
    let item = items(markup).into_iter().find(|i| i.tag == "div").unwrap();
    assert_eq!(item.border_widths[0], 96.0);
    assert_eq!(item.height, 96.0);
}

/** `ClampLineWidth`: a hairline paints one pixel and wider widths floor to whole pixels. */
#[test]
fn border_widths_snap_like_clamp_line_width() {
    let markup = "<div style='border:solid;border-width:0.25px 2.7px 0 1pt'></div>";
    let item = items(markup).into_iter().find(|i| i.tag == "div").unwrap();
    assert_eq!(item.border_widths, [1.0, 2.0, 0.0, 1.0]);
}

/** A row group's background fills the cells it holds, painted before them. */
#[test]
fn row_group_background_paints_behind_its_cells() {
    let markup = "<style>#t{display:table;width:96px}#g{display:table-row-group;\
        background:black}.r{display:table-row}.c{display:table-cell;height:48px}</style>\
        <div id=t><div id=g><div class=r><div class=c></div><div class=c></div></div>\
        <div class=r><div class=c></div></div></div></div>";
    let all = items(markup);
    let at = all
        .iter()
        .position(|i| i.background.as_deref() == Some("black"))
        .expect("the row group painted");
    let group = &all[at];
    assert_eq!((group.width, group.height), (96.0, 96.0));
    assert!(all[at + 1..].iter().any(|i| i.height == 48.0));
}

/** A column's background covers only its own column of cells. */
#[test]
fn column_background_covers_its_column() {
    let markup = "<style>#t{display:table;table-layout:fixed;width:96px}\
        .k{display:table-column}#k2{background:black}.r{display:table-row}\
        .c{display:table-cell;height:48px}</style><div id=t><div class=k></div>\
        <div class=k id=k2></div><div class=r><div class=c></div><div class=c></div></div>\
        <div class=r><div class=c></div><div class=c></div></div></div>";
    let column = items(markup)
        .into_iter()
        .find(|i| i.background.as_deref() == Some("black"))
        .expect("the column painted");
    assert_eq!(
        [column.x, column.width, column.height],
        [8.0 + 48.0, 48.0, 96.0]
    );
}
