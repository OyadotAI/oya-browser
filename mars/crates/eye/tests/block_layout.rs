/** Block layout after Blink's BlockLayoutAlgorithm: margin collapsing, floats, clearance and
block formatting contexts. Expected values are Chromium's. */
use mars_eye::{compute_styles, parse, render};

/** Dumps the display items of the file at `DUMP` (a debugging aid, not a check). */
#[test]
#[ignore = "debugging aid: DUMP=<file> cargo test --test block_layout -- --ignored dump"]
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

/** A min-height that grows a block past its content separates the last child's bottom margin,
which Blink then drops instead of collapsing it through (min-height-separates-margin). */
#[test]
fn a_min_height_block_drops_its_last_childs_bottom_margin() {
    let markup = "<div style='min-height:50px'><div style='height:49px;margin-bottom:10px'></div>\
                  </div><div id=next style='height:50px'></div>";
    assert_eq!(geometry(markup, "next").0, 50.0);
}

/** With no min-height in play the last child's bottom margin still collapses through. */
#[test]
fn an_auto_height_block_passes_its_last_childs_bottom_margin_through() {
    let markup = "<div><div style='height:49px;margin-bottom:10px'></div></div>\
                  <div id=next style='height:50px'></div>";
    assert_eq!(geometry(markup, "next").0, 59.0);
}

/** A new formatting context with an authored height must fit beside the floats over its whole
height, not only its first line (Blink's opportunity block-size check). */
#[test]
fn a_formatting_context_fits_beside_floats_over_its_whole_height() {
    let markup = "<div style='width:400px'>\
                  <div style='float:left;clear:left;width:50px;height:75px'></div>\
                  <div style='float:left;clear:left;width:100px;height:75px'></div>\
                  <div style='overflow:hidden;width:200px;height:50px'></div>\
                  <div id=b style='overflow:hidden;width:200px;height:50px'></div></div>";
    let (y, _, x) = geometry(markup, "b");
    assert_eq!((x, y), (100.0, 50.0));
}
