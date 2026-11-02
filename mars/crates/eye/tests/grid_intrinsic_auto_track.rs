/** An implicit `auto` grid column sized under a min-content constraint: BBC News' lead card
headline sits in a grid whose only track is implicit, and the track grew to the headline's
max-content width instead of the min-content size the grid's width leaves room for (CSS Grid 1
12.5: auto maxima take min-content contributions when the grid has no definite available
space). */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** The width of every painted box in `markup`, in paint order, laid out at 800px. */
fn widths(markup: &str) -> Vec<(String, f64)> {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .map(|item: &DisplayItem| (item.tag.clone(), item.width))
        .collect()
}

/** The width of the first `tag` box in `markup`. */
fn width_of(markup: &str, tag: &str) -> f64 {
    widths(markup)
        .into_iter()
        .find(|(name, _)| name == tag)
        .map(|(_, width)| width)
        .unwrap()
}

/** A headline inside an implicit-column grid stays within the grid's 258px: the nested grid
takes the track's width, not the headline's 620px max-content. */
#[test]
fn nested_grid_in_implicit_auto_column_keeps_the_track_width() {
    let markup = "<div style='display:grid;width:258px'><div style='display:grid'><div><h2 style='margin:0;font:600 22px/26px sans-serif'>Shops looted as German cost of living protests turn violent</h2></div></div></div>";
    let inner = widths(markup)
        .into_iter()
        .filter(|(name, _)| name == "div")
        .map(|(_, width)| width)
        .nth(1)
        .unwrap();
    assert_eq!(inner.round(), 258.0);
    assert!(width_of(markup, "h2") <= 258.0 + 1.0);
}

/** A grid that is itself shrink-to-fit still takes its items' max-content width: the min-content
rule applies only where the grid's width is indefinite under a min-content constraint. */
#[test]
fn inline_grid_still_sizes_to_its_max_content() {
    let markup = "<div style='display:inline-grid'><div><h2 style='margin:0;font:600 22px/26px sans-serif'>Shops looted as German cost of living protests turn violent</h2></div></div>";
    assert!(width_of(markup, "h2") > 500.0);
}
