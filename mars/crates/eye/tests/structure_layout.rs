/** Grid named areas, the `grid-template` shorthand and `display: contents` boxes. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** A styled document and its display list at an 800 point viewport. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first item painted for the element with `id`, if any. */
fn find<'a>(document: &Document, display: &'a DisplayList, id: &str) -> Option<&'a DisplayItem> {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display.items.iter().find(|item| item.local_id == local)
}
/** Equal within layout rounding. */
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.01
}

/** Items named by `grid-area` land in the areas of `grid-template-areas`, whatever their order. */
#[test]
fn named_areas_place_items() {
    let (document, display) = layout(
        "<div style=\"display:grid;width:400px;grid-template-columns:100px 1fr;\
         grid-template-areas:'head head' 'side main'\">\
         <div id=m style='grid-area:main'>m</div><div id=s style='grid-area:side'>s</div>\
         <div id=h style='grid-area:head'>h</div></div>",
    );
    let (h, s, m) = (
        find(&document, &display, "h").unwrap(),
        find(&document, &display, "s").unwrap(),
        find(&document, &display, "m").unwrap(),
    );
    assert!(close(h.width, 400.0));
    assert!(close(s.x, h.x) && close(s.width, 100.0));
    assert!(close(m.x, h.x + 100.0) && close(m.width, 300.0));
    assert!(s.y > h.y && close(s.y, m.y));
}

/** `grid-template: rows / columns` with area strings sets all three longhands. */
#[test]
fn grid_template_shorthand_sets_areas_and_tracks() {
    let (document, display) = layout(
        "<div style=\"display:grid;width:300px;grid-template:'a b' 40px / 100px 1fr\">\
         <div id=b style='grid-area:b'>b</div><div id=a style='grid-area:a'>a</div></div>",
    );
    let (a, b) = (
        find(&document, &display, "a").unwrap(),
        find(&document, &display, "b").unwrap(),
    );
    assert!(close(a.width, 100.0) && close(b.width, 200.0));
    assert!(close(a.height, 40.0) && close(a.y, b.y));
}

/** Areas that are not rectangles invalidate the declaration, and the items auto-place. */
#[test]
fn non_rectangular_areas_are_ignored() {
    let (document, display) = layout(
        "<div style=\"display:grid;width:200px;grid-template-columns:1fr 1fr;\
         grid-template-areas:'a b' 'b a'\">\
         <div id=x style='grid-area:b'>x</div><div id=y>y</div></div>",
    );
    let (x, y) = (
        find(&document, &display, "x").unwrap(),
        find(&document, &display, "y").unwrap(),
    );
    assert!(close(x.y, y.y) && x.x < y.x);
}

/** A `display: contents` wrapper's children become items of the flex container around it. */
#[test]
fn contents_children_join_the_parent_flex_line() {
    let (document, display) = layout(
        "<div style='display:flex;width:400px'><div id=w style='display:contents'>\
         <span id=a style='width:50px'>a</span><span id=b style='width:50px'>b</span></div>\
         <span id=c style='width:50px'>c</span></div>",
    );
    assert!(find(&document, &display, "w").is_none());
    let (a, b, c) = (
        find(&document, &display, "a").unwrap(),
        find(&document, &display, "b").unwrap(),
        find(&document, &display, "c").unwrap(),
    );
    assert!(close(a.y, b.y) && close(b.y, c.y));
    assert!(a.x < b.x && b.x < c.x);
}

/** Minified track lists with no space after a function still split into tracks. */
#[test]
fn minified_track_functions_split() {
    let (document, display) = layout(
        "<div style='display:grid;width:600px;grid-template-columns:minmax(0,1fr)max(200px,10%)'>\
         <div id=a>a</div><div id=b>b</div></div>",
    );
    let (a, b) = (
        find(&document, &display, "a").unwrap(),
        find(&document, &display, "b").unwrap(),
    );
    assert!(
        close(b.width, 200.0) && close(a.width, 400.0),
        "{} {} {:?}",
        a.width,
        b.width,
        document.diagnostics
    );
    assert!(close(a.y, b.y));
}

/** A more specific `grid-area` beats a general `grid-column`, as the CNN 8-4 zone layout relies on. */
#[test]
fn grid_area_competes_with_grid_column_in_the_cascade() {
    let markup = "<style>.g>*{grid-column:1}.g>:last-child{grid-area:1/2/span 100}</style>\
         <div class=g style='display:grid;width:300px;grid-template-columns:100px 1fr'>\
         <div id=a>a</div><div id=b>b</div><div id=c>c</div></div>";
    let parsed = parse(markup).unwrap();
    let sheets: Vec<String> = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            mars_eye::StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    let document = compute_styles(&parsed, &sheets).unwrap();
    let display = render(&document, 800.0).unwrap();
    let (a, b, c) = (
        find(&document, &display, "a").unwrap(),
        find(&document, &display, "b").unwrap(),
        find(&document, &display, "c").unwrap(),
    );
    assert!(close(c.x, a.x + 100.0) && close(c.y, a.y));
    assert!(close(b.x, a.x) && b.y > a.y);
}
