/** Clearance and margin collapsing after Blink's BlockLayoutAlgorithm (HandleInflow and
FinishInflow). Expected values are Chromium's. */
use mars_eye::{compute_styles, parse, render};

/** Dumps the display items of the file at `DUMP` (a debugging aid, not a check). */
#[test]
#[ignore = "debugging aid: DUMP=<file> cargo test --test flow_clearance -- --ignored dump"]
fn dump() {
    let path = std::env::var("DUMP").unwrap();
    let markup = std::fs::read_to_string(path).unwrap();
    let parsed = parse(&markup).unwrap();
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
    for item in &display.items {
        let id = document.nodes.iter().find(|n| n.local_id == item.local_id);
        println!(
            "{:8} {:10} x={:7.2} y={:7.2} w={:7.2} h={:7.2} bg={:?}",
            item.tag,
            id.map_or("", |n| n.html_id.as_str()),
            item.x,
            item.y,
            item.width,
            item.height,
            item.background
        );
    }
}

/** (y, height, x) of `#id` after laying out `markup` at 800px with no body margin. */
fn geometry(markup: &str, id: &str) -> (f64, f64, f64) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.y, item.height, item.x)
}

/** A `clear` that needs no clearance (its margin already takes it past the float) leaves the
margin collapsing through its parent (no-clearance-due-to-large-margin). */
#[test]
fn a_clear_with_no_clearance_keeps_collapsing_through_its_parent() {
    let markup = "<div style='float:left;width:100px;height:100px'></div>\
                  <div style='padding-top:1px'><div id=parent>\
                  <div style='clear:left;margin-top:150px;height:10px'></div></div></div>";
    assert_eq!(geometry(markup, "parent").0, 151.0);
}

/** Clearing a float adjoining the open strut is clearance however large the margin: the child
sits at the float's bottom and the parent resolves above its margin. */
#[test]
fn clearing_an_adjoining_float_always_gets_clearance() {
    let markup = "<div id=parent><div style='float:left;width:10px;height:50px'></div>\
                  <div id=child style='clear:left;margin-top:80px;height:10px'></div></div>";
    assert_eq!(geometry(markup, "child").0, 50.0);
    assert_eq!(geometry(markup, "parent").0, 0.0);
}

/** A formatting-context root beside a float keeps a margin that sits under the float out of
the fit: margins shrink the opportunity from the content edge (new-fc-beside-float-with-margin). */
#[test]
fn a_new_formatting_context_margin_under_a_float_does_not_push_it_down() {
    let markup = "<div style='width:100px;height:100px'>\
                  <div style='float:left;width:50px;height:100px'></div>\
                  <div id=fc style='overflow:hidden;margin-right:1px;width:50px;height:100px'>\
                  </div></div>";
    assert_eq!(geometry(markup, "fc"), (0.0, 100.0, 50.0));
}

/** A formatting-context root that does not fit beside floats adjoining the open strut has its
top margin separated: the floats stay at the top and the root goes below them
(new-fc-separates-from-float). */
#[test]
fn a_new_formatting_context_below_adjoining_floats_separates_its_margin() {
    let markup = "<div id=outer style='overflow:hidden;width:200px'><div><div>\
                  <div id=float style='float:left;width:200px;height:200px'></div></div>\
                  <div id=fc style='margin-top:200px;overflow:hidden;width:200px;height:1px'>\
                  </div></div></div>";
    assert_eq!(geometry(markup, "float").0, 0.0);
    assert_eq!(geometry(markup, "fc").0, 200.0);
    assert_eq!(geometry(markup, "outer").1, 201.0);
}

/** An auto-width formatting-context root whose start margin reaches past a right float's edge
does not fit beside it and moves below the float (floats-wrap-bfc-with-margin-004). */
#[test]
fn an_auto_width_root_whose_margin_crosses_a_right_float_moves_below_it() {
    let markup = "<div style='width:100px;display:flow-root'>\
                  <div style='float:right;width:50px;height:40px'></div>\
                  <div id=fc style='display:flow-root;height:60px;margin-left:51px'></div></div>";
    assert_eq!(geometry(markup, "fc").0, 40.0);
}

/** A root wider than its containing block may not overlap a float beyond that block's edge, so
it moves below the float (floats-wrap-bfc-with-margin-008). */
#[test]
fn a_root_overflowing_into_a_float_moves_below_it() {
    let markup = "<div style='width:100px'><div style='float:right;width:50px;height:50px'></div>\
                  <div style='margin-right:50px'>\
                  <div id=fc style='overflow:hidden;width:100px;height:50px'></div></div></div>";
    assert_eq!(geometry(markup, "fc").0, 50.0);
}

/** A zero-width float occupies no room, so a root with negative margins may extend over it
(zero-width-floats). */
#[test]
fn a_zero_width_float_does_not_push_a_root_with_negative_margins() {
    let markup = "<div style='margin-left:50px;width:100px'>\
                  <div style='float:left;width:0;height:50px'></div>\
                  <div id=fc style='overflow:hidden;margin:0 -50px;height:100px'></div></div>";
    assert_eq!(geometry(markup, "fc"), (0.0, 100.0, 0.0));
}
