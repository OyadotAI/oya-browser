/** Outlines paint a ring outside the border box and never change layout, as Blink's OutlinePainter does. */
use mars_eye::{
    DisplayItem, DisplayList, DisplayOutline, Document, StyleSource, compute_styles, parse, render,
};

/** Lays out `markup` with `sheet` at 800px. */
fn layout(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let mut parsed = parse(markup).unwrap();
    let mut sheets: Vec<String> = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    sheets.push(sheet.to_string());
    parsed.styles.clear();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** The display item of the element with `id`. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
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
}

/** The outline of the element with `id`, which must paint one. */
fn outline(sheet: &str) -> DisplayOutline {
    let (document, display) = layout("<div id=b style='width:100px;height:50px'></div>", sheet);
    item(&document, &display, "b").outline.clone().unwrap()
}

/** `outline` with a width, a style and a color paints that ring at the initial zero offset. */
#[test]
fn shorthand_sets_width_style_and_color() {
    let ring = outline("#b{outline:2px dashed red}");
    assert_eq!(ring.width, 2.0);
    assert_eq!(ring.style, mars_eye::BorderStyle::Dashed);
    assert_eq!(ring.color.as_deref(), Some("red"));
    assert_eq!(ring.offset, 0.0);
}

/** `outline-offset` moves the ring outward, and a negative offset lets it overlap the box. */
#[test]
fn offset_moves_the_ring_and_may_be_negative() {
    assert_eq!(
        outline("#b{outline:1px solid;outline-offset:4px}").offset,
        4.0
    );
    assert_eq!(
        outline("#b{outline:1px solid;outline-offset:-2px}").offset,
        -2.0
    );
}

/** `outline-style: auto` paints as a solid ring of the medium width when none is authored. */
#[test]
fn auto_style_paints_a_solid_medium_ring() {
    let ring = outline("#b{outline:auto}");
    assert_eq!(ring.style, mars_eye::BorderStyle::Solid);
    assert_eq!(ring.width, 3.0);
}

/** `outline-style: none` and a zero width each paint no outline at all. */
#[test]
fn none_style_and_zero_width_paint_nothing() {
    for sheet in ["#b{outline:4px none red}", "#b{outline:0 solid red}"] {
        let (document, display) = layout("<div id=b style='width:100px;height:50px'></div>", sheet);
        assert!(item(&document, &display, "b").outline.is_none(), "{sheet}");
    }
}

/** An outline is out of flow: the box keeps the same geometry with or without one. */
#[test]
fn outline_does_not_change_layout() {
    let plain = layout(
        "<div id=b style='width:100px;height:50px;margin:8px'></div><p id=p>after</p>",
        "",
    );
    let ringed = layout(
        "<div id=b style='width:100px;height:50px;margin:8px'></div><p id=p>after</p>",
        "#b{outline:10px solid blue;outline-offset:6px}",
    );
    for id in ["b", "p"] {
        let (a, b) = (item(&plain.0, &plain.1, id), item(&ringed.0, &ringed.1, id));
        assert_eq!(
            (a.x, a.y, a.width, a.height),
            (b.x, b.y, b.width, b.height),
            "{id}"
        );
    }
}

/** An outline the author did not set paints nothing, and a `none` style is not a diagnostic. */
#[test]
fn outline_properties_are_no_longer_diagnostics() {
    let (document, _) = layout("<p>x</p>", "p{outline:2px solid red;outline-color:blue}");
    assert!(
        !document
            .diagnostics
            .iter()
            .any(|message| message.contains("outline")),
        "{:?}",
        document.diagnostics
    );
}

/** A width too thin to see is raised to the minimum line, so painters never stroke a vanishing ring. */
#[test]
fn thin_outline_width_is_raised_to_the_minimum_line() {
    assert_eq!(outline("#b{outline:0.01px solid}").width, 0.5);
}

/** A huge outline offset is clamped to the widest a box may carry, so its ring stays near the box. */
#[test]
fn huge_outline_offset_is_clamped() {
    assert_eq!(
        outline("#b{outline:2px dotted;outline-offset:1000000px}").offset,
        mars_eye::MAX_OUTLINE_OFFSET
    );
    assert_eq!(
        outline("#b{outline:2px dotted;outline-offset:-1000000px}").offset,
        -mars_eye::MAX_OUTLINE_OFFSET
    );
}
