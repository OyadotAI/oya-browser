/** Absolutely positioned boxes with insets, auto margins and `aspect-ratio`, and shrink-to-fit
 * flex and grid containers that hold `::before` and `::after` items. */
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
/** The first painted item with this tag. */
fn tagged<'a>(display: &'a DisplayList, tag: &str) -> &'a DisplayItem {
    display
        .items
        .iter()
        .find(|item| item.tag == tag)
        .unwrap_or_else(|| panic!("{tag} was not painted"))
}
/** Whether two layout values match to a hundredth of a pixel. */
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}
/** A 600x400 positioned containing block at the page origin. */
const BLOCK: &str = "body{margin:0}.c{position:relative;width:600px;height:400px}";

/** `inset:0; margin:auto; width:300px; aspect-ratio:1` is 300x300 and centred (nodejs.org's
 * glowing backdrop). */
#[test]
fn aspect_ratio_sizes_and_auto_margins_centre_an_inset_box() {
    let display = layout(&format!(
        "<style>{BLOCK}b{{display:block;position:absolute;inset:0;margin:auto;width:300px;\
         aspect-ratio:1;background:red}}</style><div class=c><b></b></div>"
    ));
    let b = tagged(&display, "b");
    assert!(near(b.width, 300.0) && near(b.height, 300.0), "{b:?}");
    assert!(near(b.x, 150.0) && near(b.y, 50.0), "{b:?}");
}

/** Auto margins share the free space equally in both axes for an explicitly sized box, a
 * generated one included. */
#[test]
fn auto_margins_centre_an_explicitly_sized_inset_box() {
    let display = layout(&format!(
        "<style>{BLOCK}.c::before{{content:'';position:absolute;inset:0;margin:auto;\
         width:200px;height:100px;background:red}}</style><div class=c></div>"
    ));
    let b = tagged(&display, "::before");
    assert!(near(b.width, 200.0) && near(b.height, 100.0), "{b:?}");
    assert!(near(b.x, 200.0) && near(b.y, 150.0), "{b:?}");
}

/** The same rules place an absolutely positioned `::after`. */
#[test]
fn generated_absolute_box_takes_aspect_ratio_and_centres() {
    let display = layout(&format!(
        "<style>{BLOCK}.c::after{{content:'';position:absolute;inset:0;margin:auto;\
         width:300px;aspect-ratio:1;background:red}}</style><div class=c></div>"
    ));
    let after = tagged(&display, "::after");
    assert!(
        near(after.width, 300.0) && near(after.height, 300.0),
        "{after:?}"
    );
    assert!(near(after.x, 150.0) && near(after.y, 50.0), "{after:?}");
}

/** The width of the first `.f` container, with and without the extra rule. */
fn container_width(container: &str, extra: &str) -> f64 {
    let display = layout(&format!(
        "<style>body{{margin:0}}.f{{display:{container}}}i{{display:block;width:30px;\
         height:10px}}{extra}</style><div><span class=f><i></i></span></div>"
    ));
    tagged(&display, "span").width
}
/** An inline-flex container is as wide as its `::after` item and the gap before it. */
#[test]
fn inline_flex_grows_by_its_generated_item() {
    let after = ".f::after{content:'';width:20px;height:10px}";
    let (bare, with) = (
        container_width("inline-flex;column-gap:5px", ""),
        container_width("inline-flex;column-gap:5px", after),
    );
    assert!(with >= bare + 25.0 - 0.01, "{bare} -> {with}");
}
/** An inline-grid container is as wide as its `::before` item as well as its children. */
#[test]
fn inline_grid_grows_by_its_generated_item() {
    let before = ".f::before{content:'';width:20px;height:10px}";
    let (bare, with) = (
        container_width("inline-grid;grid-auto-flow:column", ""),
        container_width("inline-grid;grid-auto-flow:column", before),
    );
    assert!(with >= bare + 20.0 - 0.01, "{bare} -> {with}");
}
