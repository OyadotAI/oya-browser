/** Percentage grid gaps after Blink's GridTrackSizingAlgorithm::CalculateGutterSize: they
resolve against the content box, a row gap against an auto height only once the rows are sized
(css-grid alignment/grid-gutters-007 to 016). Expected values are Chromium's. */
use mars_eye::{compute_styles, parse, render};

/** (x, y) of every child of `#grid` laid out at 800px with no body margin. */
fn cells(grid_style: &str) -> Vec<(f64, f64)> {
    let markup = format!(
        "<body style='margin:0'><div id=grid style='display:grid;width:200px;\
         grid-template-columns:90px 90px;grid-template-rows:90px 90px;{grid_style}'>\
         <div></div><div></div><div></div><div></div></div>"
    );
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let grid = document.nodes.iter().find(|n| n.html_id == "grid").unwrap();
    let ids: Vec<usize> = document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(grid.local_id))
        .map(|n| n.local_id)
        .collect();
    display
        .items
        .iter()
        .filter(|i| ids.contains(&i.local_id))
        .map(|i| (i.x, i.y))
        .collect()
}

/** A definite height resolves both percentages against the content box. */
#[test]
fn percentage_gaps_resolve_against_a_definite_content_box() {
    let at = cells("height:200px;gap:10%");
    assert_eq!(
        at,
        vec![(0.0, 0.0), (110.0, 0.0), (0.0, 110.0), (110.0, 110.0)]
    );
}

/** An auto height sizes the rows with the row gap as zero, then resolves it against that
 * height, so the rows overflow the grid as in Chromium. */
#[test]
fn a_percentage_row_gap_resolves_against_the_sized_auto_height() {
    let at = cells("gap:10%");
    assert_eq!(
        at,
        vec![(0.0, 0.0), (110.0, 0.0), (0.0, 108.0), (110.0, 108.0)]
    );
}

/** A calc() gap adds its length and its percentage parts. */
#[test]
fn a_calc_gap_adds_length_and_percentage() {
    let at = cells("height:200px;gap:calc(5% + 10px)");
    assert_eq!(
        at,
        vec![(0.0, 0.0), (110.0, 0.0), (0.0, 110.0), (110.0, 110.0)]
    );
}

/** Row grid-lanes resolve percentage gaps as a grid does, so a lanes container matches the
 * column-flow grid it mirrors (grid-lanes/gap/row-percentage-gap-001). */
#[test]
fn row_grid_lanes_resolve_percentage_gaps_like_a_grid() {
    let geometry = |container: &str| {
        let markup = format!(
            "<body style='margin:0'><div id=g style='{container};grid-template-rows:50px 50px;\
             gap:20%'><div style='width:100px'></div><div style='width:100px'></div>\
             <div style='width:100px'></div><div style='width:100px'></div></div>"
        );
        let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
        let display = render(&document, 800.0).unwrap();
        display
            .items
            .iter()
            .map(|i| (i.x, i.y, i.width, i.height))
            .collect::<Vec<_>>()
    };
    let lanes = geometry("display:grid-lanes");
    let grid = geometry("display:grid;grid-auto-flow:column;grid-template-columns:100px 100px");
    assert_eq!(lanes, grid);
    // The last item sits in the second row: 50px plus 20% of the 100px the rows sized to.
    assert_eq!(lanes[4].1, 70.0);
}
