/** `::before` and `::after` boxes as blockified flex and grid items: first and last, ordered,
 * sized and auto-placed like any other item. */
use mars_eye::{DisplayItem, DisplayList, StyleSource, compute_styles, parse, render};

/** Styles from the document's own `<style>` elements, then the display list. */
fn layout(markup: &str) -> DisplayList {
    let parsed = parse(markup).unwrap();
    let sheets = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let document = compute_styles(&parsed, &sheets).unwrap();
    render(&document, 800.0).unwrap()
}
/** The one painted item with this tag. */
fn tagged<'a>(display: &'a DisplayList, tag: &str) -> &'a DisplayItem {
    let found = display
        .items
        .iter()
        .filter(|item| item.tag == tag)
        .collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "{tag} painted {} times", found.len());
    found[0]
}
/** Whether two layout values match to a hundredth of a pixel. */
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}
/** Shared item styles: 20px squares around one 30px child. */
const SQUARES: &str = ".b:before,.b:after{content:'';width:20px;height:20px;background:red}\
    .b:before{background:blue}i{display:block;width:30px;height:10px}";

/** In a row, `::before` is the first item and `::after` the last, each at its own size. */
#[test]
fn row_places_before_first_and_after_last() {
    let display = layout(&format!(
        "<style>body{{margin:0}}.b{{display:flex}}{SQUARES}</style><div class=b><i></i></div>"
    ));
    let (before, after, child) = (
        tagged(&display, "::before"),
        tagged(&display, "::after"),
        tagged(&display, "i"),
    );
    assert!(near(before.x, 0.0) && near(before.width, 20.0) && near(before.height, 20.0));
    assert!(near(child.x, 20.0));
    assert!(near(after.x, 50.0) && near(after.width, 20.0) && near(after.height, 20.0));
}

/** In a column they stack above and below the children. */
#[test]
fn column_stacks_before_above_and_after_below() {
    let display = layout(&format!(
        "<style>body{{margin:0}}.b{{display:flex;flex-direction:column;align-items:flex-start}}\
         {SQUARES}</style><div class=b><i></i></div>"
    ));
    let (before, after, child) = (
        tagged(&display, "::before"),
        tagged(&display, "::after"),
        tagged(&display, "i"),
    );
    assert!(near(before.y, 0.0) && near(before.height, 20.0) && near(before.width, 20.0));
    assert!(near(child.y, 20.0));
    assert!(near(after.y, 30.0) && near(after.height, 20.0));
}

/** Grid auto-placement gives `::before` the first cell and `::after` the cell after the children. */
#[test]
fn grid_auto_places_generated_items() {
    let display = layout(&format!(
        "<style>body{{margin:0}}.b{{display:grid;grid-template-columns:40px 40px 40px}}\
         {SQUARES}</style><div class=b><i></i></div>"
    ));
    let (before, after, child) = (
        tagged(&display, "::before"),
        tagged(&display, "::after"),
        tagged(&display, "i"),
    );
    assert!(near(before.x, 0.0) && near(before.width, 20.0) && near(before.height, 20.0));
    assert!(near(child.x, 40.0));
    assert!(near(after.x, 80.0) && near(after.y, 0.0) && near(after.width, 20.0));
}

/** `order` moves generated items like any other: a negative order puts `::after` first. */
#[test]
fn order_applies_to_generated_items() {
    let display = layout(&format!(
        "<style>body{{margin:0}}.b{{display:flex}}{SQUARES}.b:after{{order:-1}}\
         .b:before{{order:1}}</style><div class=b><i></i></div>"
    ));
    let (before, after, child) = (
        tagged(&display, "::before"),
        tagged(&display, "::after"),
        tagged(&display, "i"),
    );
    assert!(near(after.x, 0.0));
    assert!(near(child.x, 20.0));
    assert!(near(before.x, 50.0));
}

/** `content: none` generates no box, so no item and no gap. */
#[test]
fn content_none_makes_no_item() {
    let display = layout(
        "<style>body{margin:0}.b{display:flex;gap:10px}.b:before{content:none;width:20px}\
         .b:after{content:'';width:20px;height:20px}i{display:block;width:30px;height:10px}\
         </style><div class=b><i></i></div>",
    );
    assert!(display.items.iter().all(|item| item.tag != "::before"));
    assert!(near(tagged(&display, "i").x, 0.0));
    assert!(near(tagged(&display, "::after").x, 40.0));
}
