/** A definite flex column sizes its items from their own heights, as Google's home page needs. */
use mars_eye::{
    DisplayList, Document, RenderContext, compute_styles_with_context, parse, render_with_context,
};

/** The viewport the page is laid out in. */
fn viewport() -> RenderContext {
    RenderContext {
        width: 1280.0,
        height: 800.0,
        ..RenderContext::default()
    }
}

/** Styles `body` with `css` in the viewport, then lays it out. */
fn layout(body: &str, css: &str) -> (Document, DisplayList) {
    let document = parse(body).unwrap();
    let styled = compute_styles_with_context(&document, &[css.into()], viewport()).unwrap();
    let display = render_with_context(&styled, viewport()).unwrap();
    (styled, display)
}

/** The border-box height of the element with `id`. */
fn height(document: &Document, display: &DisplayList, id: &str) -> f64 {
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
        .height
}

/** The column the page uses: a full-height flex column under `html, body {height:100%}`. */
const COLUMN: &str =
    "html,body{height:100%;margin:0}#c{display:flex;flex-direction:column;height:100%}";

/** `height: calc(100% - 560px)` on a column item resolves against the column, clamped by min and max. */
#[test]
fn a_mixed_calc_height_resolves_against_the_definite_column() {
    let body = "<div id=c><div id=top style='height:60px'></div>\
                <div id=logo style='min-height:92px;height:calc(100% - 560px);max-height:290px'></div>\
                <div id=after style='height:10px'></div></div>";
    let (document, display) = layout(body, COLUMN);
    assert_eq!(height(&document, &display, "logo"), 240.0);
    let css = format!("{COLUMN}#c{{height:1000px}}");
    let (document, display) = layout(body, &css);
    assert_eq!(height(&document, &display, "logo"), 290.0);
}

/** A mixed calc() that resolves below zero is a zero height, not an invalid declaration. */
#[test]
fn a_negative_mixed_calc_height_clamps_at_zero() {
    let (document, display) = layout(
        "<div id=c><div id=a style='height:calc(10% - 500px)'></div></div>",
        COLUMN,
    );
    assert_eq!(height(&document, &display, "a"), 0.0);
    assert!(
        !document.diagnostics.iter().any(|d| d.contains("height")),
        "{:?}",
        document.diagnostics
    );
}

/** A plain negative height is still invalid and dropped with a diagnostic. */
#[test]
fn a_plain_negative_height_is_still_dropped() {
    let (document, _) = layout("<div id=a style='height:-5px'></div>", "");
    assert!(document.nodes[0].style.height.is_none());
    assert!(!document.diagnostics.is_empty());
}

/** A percentage height item in a definite column takes its share instead of its content height. */
#[test]
fn a_percentage_height_item_takes_its_share_of_the_column() {
    let (document, display) = layout(
        "<div id=c><div id=a style='height:25%'></div></div>",
        COLUMN,
    );
    assert_eq!(height(&document, &display, "a"), 200.0);
}
