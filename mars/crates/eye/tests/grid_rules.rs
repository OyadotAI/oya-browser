/** Grid and grid-lanes rules pinned against Chromium's results: abspos grid areas, placement,
template validity, baseline self-alignment, grid-lanes auto-repeat, stretch and auto-fit,
subgrid gaps and orthogonal subgrids, and order-modified paint order. */
use mars_eye::{compute_styles, parse, render};

/** Laid-out box of one element: (x, y, width, height). */
type Rect = (f64, f64, f64, f64);

/** Every element with an `id`, in paint order, with its first box, laid out at 800px with no
 * body margin. */
fn boxes(body: &str) -> Vec<(String, Rect)> {
    let markup = format!("<body style='margin:0;font:16px/1 monospace'>{body}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let mut out: Vec<(String, Rect)> = Vec::new();
    for item in &display.items {
        let Some(node) = document.nodes.iter().find(|n| n.local_id == item.local_id) else {
            continue;
        };
        if !node.html_id.is_empty() && !out.iter().any(|(id, _)| *id == node.html_id) {
            out.push((
                node.html_id.clone(),
                (item.x, item.y, item.width, item.height),
            ));
        }
    }
    out
}

/** The box of the element with `id`. */
fn rect(body: &str, id: &str) -> Rect {
    boxes(body)
        .into_iter()
        .find(|(name, _)| name == id)
        .unwrap_or_else(|| panic!("no box for #{id}"))
        .1
}

/** An absolutely positioned child of a positioned grid takes its grid area as containing block. */
#[test]
fn an_abspos_child_is_contained_by_its_grid_area() {
    let body = "<div style='display:grid;position:relative;grid:50px 60px/100px 200px;padding:5px'>\
                <i id=a style='position:absolute;inset:0;grid-area:2/2/3/3'></i></div>";
    assert_eq!(rect(body, "a"), (105.0, 55.0, 200.0, 60.0));
}

/** An abspos box inside a nested grid keeps the outer grid's padding box. */
#[test]
fn an_abspos_box_inside_a_nested_grid_keeps_the_padding_box() {
    let body = "<div style='display:grid;position:relative;grid:50px/100px 200px;width:300px'>\
                <div style='display:grid;grid-column:2'>\
                <i id=a style='position:absolute;inset:0;grid-column:2'></i></div></div>";
    assert_eq!(rect(body, "a"), (0.0, 0.0, 300.0, 50.0));
}

/** Row-locked items step past occupied cells, growing implicit columns instead of overlapping. */
#[test]
fn row_locked_items_grow_implicit_columns() {
    let body = "<div style='display:grid;width:300px'>\
                <i id=a style='grid-row:1;height:10px'></i><i id=b style='grid-row:2'></i>\
                <i id=c style='grid-row:1;height:10px'></i></div>";
    assert_eq!(rect(body, "c").0, 150.0);
}

/** A template of `.` cells still sizes the explicit grid, so `-1` is past its last column. */
#[test]
fn unnamed_template_areas_size_the_explicit_grid() {
    let body = "<div style='display:grid;grid-template-areas:\". . .\";grid-auto-columns:50px'>\
                <i id=a style='grid-column:-2;height:5px'></i></div>";
    assert_eq!(rect(body, "a").0, 100.0);
}

/** With an auto-repeat group, any other intrinsic track makes the declaration invalid. */
#[test]
fn an_auto_repeat_beside_an_intrinsic_track_is_invalid() {
    let body = "<div style='display:grid;width:300px;grid-template-columns:100px;\
                grid-template-columns:auto repeat(auto-fill,50px)'><i id=a></i></div>";
    assert_eq!(rect(body, "a").2, 100.0);
}

/** `align-self: baseline` items of one row share a baseline and the row grows by the shim. */
#[test]
fn baseline_items_share_their_rows_baseline() {
    let body = "<div id=g style='display:grid;grid-template-columns:50px 50px;align-items:baseline'>\
                <i id=a style='display:block;font-size:20px'>x</i>\
                <i id=b style='display:block;font-size:40px'>x</i></div>";
    let (_, ay, _, ah) = rect(body, "a");
    let (_, by, _, _) = rect(body, "b");
    // Monospace ascents differ by half the size difference times the ascent ratio.
    assert!(ay > by);
    assert!(rect(body, "g").3 >= ah + (ay - by));
}

/** A synthesized baseline that a percentage height would make cyclic falls back to start. */
#[test]
fn a_percentage_height_without_a_line_falls_back_to_start() {
    let body = "<div style='display:grid;grid-template-columns:50px 50px;align-items:baseline'>\
                <canvas id=a style='width:100%;height:100%'></canvas>\
                <div style='height:100px'><br></div></div>";
    assert_eq!(rect(body, "a").1, 0.0);
}

/** Grid-lanes stacking stretches only an authored `stretch`, never `normal`. */
#[test]
fn grid_lanes_stretch_fills_the_lane_but_normal_does_not() {
    let lanes = |align: &str| {
        format!(
            "<div style='display:grid-lanes;grid-template-columns:50px 50px;align-items:{align}'>\
             <i id=a style='display:block;height:100px'></i><i id=b style='display:block'>x</i></div>"
        )
    };
    assert_eq!(rect(&lanes("stretch"), "b").3, 100.0);
    assert_eq!(rect(&lanes("normal"), "b").3, 16.0);
}

/** An auto-fit group collapses the lanes no item can take; auto-placed items fill the first. */
#[test]
fn grid_lanes_auto_fit_collapses_empty_lanes() {
    let body = "<div style='display:grid-lanes;width:300px;justify-content:center;\
                grid-template-columns:repeat(auto-fit,100px)'><i id=a style='height:5px'></i></div>";
    assert_eq!(rect(body, "a").0, 100.0);
}

/** Row grid-lanes repeat an intrinsic auto-repeat group as often as its sized tracks fit. */
#[test]
fn row_grid_lanes_repeat_intrinsic_tracks() {
    let body = "<div style='display:grid-lanes;height:100px;flow-tolerance:0;\
                grid-template-rows:repeat(auto-fill,min-content)'>\
                <i id=a style='display:block;width:10px;height:50px'></i>\
                <i id=b style='display:block;width:10px;height:50px'></i></div>";
    assert_eq!(rect(body, "b").1, 50.0);
}

/** A row grid-lanes container is as wide as its items stack along the rows. */
#[test]
fn row_grid_lanes_intrinsic_width_stacks_items() {
    let body = "<div id=g style='display:inline-grid-lanes;grid-template-rows:20px'>\
                <i style='display:block;width:30px'></i><i style='display:block;width:40px'></i></div>";
    assert_eq!(rect(body, "g").2, 70.0);
}

/** A subgrid gap wider than its parent's stays centered on the parent's lines, so an empty
 * track may go negative. */
#[test]
fn a_wide_subgrid_gap_stays_on_the_parent_lines() {
    let body = "<div style='display:grid;grid-template-columns:auto 200px;width:200px'>\
                <div style='display:grid;grid-template-columns:subgrid;gap:100px;grid-column:span 2'>\
                <i id=a style='width:100px;height:10px;grid-column:2'></i></div></div>";
    assert_eq!(rect(body, "a").0, 50.0);
}

/** A vertical-rl subgrid's rows take its parent's columns, right to left. */
#[test]
fn an_orthogonal_subgrid_inherits_the_parent_columns() {
    let body = "<div style='display:grid;grid-template-columns:50px 100px'>\
                <div style='width:50px;height:100px'></div>\
                <div style='display:grid;writing-mode:vertical-rl;grid:subgrid/auto;grid-column:2'>\
                <i id=a style='display:block;width:30px;height:40px'></i></div></div>";
    assert_eq!(rect(body, "a").0, 120.0);
}

/** Flex and grid items paint in order-modified document order. */
#[test]
fn grid_items_paint_in_order_modified_order() {
    let body = "<div style='display:grid'><i id=a style='grid-area:1/1;order:1'></i>\
                <i id=b style='grid-area:1/1'></i></div>";
    let order: Vec<String> = boxes(body)
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| id == "a" || id == "b")
        .collect();
    assert_eq!(order, ["b", "a"]);
}
