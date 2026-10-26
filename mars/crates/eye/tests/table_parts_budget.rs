/** Table part backgrounds stay bounded: a table with thousands of filled column groups renders
 * in linear time instead of rescanning the document once per part. */
use mars_eye::{StyleSource, compute_styles, parse, render};
use std::time::{Duration, Instant};

/** Column groups in the hostile table. */
const GROUPS: usize = 20_000;
/** The render bound for the whole page, generous for debug builds on a loaded machine. */
const LIMIT: Duration = Duration::from_secs(2);

/** Twenty thousand filled colgroups over one row of cells render in bounded time. */
#[test]
fn many_filled_colgroups_render_in_bounded_time() {
    let groups = "<div class=g></div>".repeat(GROUPS);
    let markup = format!(
        "<style>#t{{display:table;width:96px}}.g{{display:table-column-group;background:black}}\
         .k{{display:table-column}}.r{{display:table-row}}.c{{display:table-cell;height:8px}}\
         </style><div id=t>{groups}<div class=r><div class=c></div><div class=c></div></div>\
         <div class=r><div class=c></div></div></div>"
    );
    let parsed = parse(&markup).unwrap();
    let sheets: Vec<String> = parsed
        .styles
        .iter()
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .collect();
    let styled = compute_styles(&parsed, &sheets).unwrap();
    let started = Instant::now();
    let list = render(&styled, 800.0).unwrap();
    let elapsed = started.elapsed();
    assert!(elapsed < LIMIT, "render took {elapsed:?}");
    assert!(list.items.iter().any(|i| i.height == 8.0));
    // The first groups still cover the table's two leaf columns, painting under each of their
    // cells: two in the first column, one in the second.
    let painted = list
        .items
        .iter()
        .filter(|i| i.background.as_deref() == Some("black"));
    assert_eq!(painted.count(), 3);
}
