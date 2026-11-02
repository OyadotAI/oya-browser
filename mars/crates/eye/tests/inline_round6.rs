/** Round-six rules from the inline builder: `calc-size()` maps the size its basis gives where
 * widths, heights and automatic minimums are found; a scrolled `local` `content-box` clip keeps
 * the padding box's rounded corners; and a sliced inline fragment's cut sides run its shadow on
 * and square its corners. */
use mars_eye::{
    DisplayItem, DisplayList, compute_styles, parse, render, render_scrolled, sliced_shadow,
};

/** The display list of `markup` styled by `sheet` at an 800px viewport. */
fn laid(markup: &str, sheet: &str) -> DisplayList {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    render(&document, 800.0).unwrap()
}

/** The item of the element with id `t`. */
fn target(markup: &str, sheet: &str) -> DisplayItem {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .local_id;
    let list = render(&document, 800.0).unwrap();
    list.items
        .into_iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** A `calc-size(auto, …)` width maps the box's intrinsic contribution, so a fit-content parent
 * of two 20px pieces at no room is 20px + 80px wide. */
#[test]
fn calc_size_maps_the_intrinsic_contribution() {
    let markup = "<div style='width:0'><div id=t style='width:fit-content'>\
        <div style='width:calc-size(auto, size + 80px)'><span></span> <span></span></div></div></div>";
    let sheet = "span{display:inline-block;width:20px}";
    assert_eq!(target(markup, sheet).width, 100.0);
}

/** A width through `aspect-ratio`, a length basis, and a `min(size, L)` cap each map. */
#[test]
fn calc_size_maps_ratio_length_and_capped_sizes() {
    let ratio =
        "<div id=t style='aspect-ratio:1/2;height:100px;width:calc-size(auto, size + 50px)'></div>";
    assert_eq!(target(ratio, "").width, 100.0);
    let length = "<div id=t style='width:calc-size(50px, size * 2)'></div>";
    assert_eq!(target(length, "").width, 100.0);
    let capped = "<div id=t style='height:calc-size(auto, min(size, 30px))'><div style='height:50px'></div></div>";
    assert_eq!(target(capped, "").height, 30.0);
}

/** A flex item's automatic minimum goes through a `calc-size()` min-width, and a column's
 * content height through a `calc-size()` height (its items then shrink to fit). */
#[test]
fn calc_size_maps_flex_minimums_and_column_heights() {
    let row = "<div style='display:flex;width:0'><div id=t style='min-width:calc-size(auto, size + 20px)'>\
        <div style='width:80px'></div></div></div>";
    assert_eq!(target(row, "").width, 100.0);
    let column = "<div id=t style='display:flex;flex-direction:column;height:calc-size(auto, min(size, 100px))'>\
        <div style='height:60px'></div><div style='height:60px'></div></div>";
    assert_eq!(target(column, "").height, 100.0);
}

/** A scrolled `local` background's `content-box` clip is the rounded padding box plus a
 * rectangle around the scrolled contents' content box. */
#[test]
fn local_content_box_clip_keeps_rounded_corners() {
    let markup = "<div id=s><div style='height:500px'></div></div>";
    let sheet = "body{margin:0} #s{width:200px;height:200px;padding:40px;border:10px solid;\
        border-radius:50%;overflow:hidden;background:green local content-box}";
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.to_string()]).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "s")
        .unwrap()
        .local_id;
    let offsets = mars_eye::ScrollOffsets::from([(local, [0.0, 15.0])]);
    let context = mars_eye::RenderContext {
        width: 800.0,
        height: 600.0,
        ..Default::default()
    };
    let list = render_scrolled(&document, context, &offsets).unwrap();
    let item = list
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap();
    assert_eq!(item.background_clip, [10.0; 4]);
    assert_eq!(item.background_rect, Some([50.0, 35.0, 200.0, 255.0]));
}

/** A span broken over two lines cuts its inner sides: their corners are square and its shadow
 * runs on to their edges. */
#[test]
fn sliced_fragments_cut_their_inner_sides() {
    let list = laid(
        "<div style='width:60px'><span id=t>aaaa bbbb</span></div>",
        "span{border-radius:5px;box-shadow:5px 0 red;font:20px/30px monospace}",
    );
    let spans: Vec<&DisplayItem> = list
        .items
        .iter()
        .filter(|item| item.tag == "span")
        .collect();
    assert_eq!(spans.len(), 2);
    assert_eq!(
        (spans[0].slice_cut, spans[1].slice_cut),
        ([false, true], [true, false])
    );
    assert_eq!((spans[0].radius[1], spans[0].radius[0]), (0.0, 5.0));
    assert_eq!(
        sliced_shadow([15.0, 0.0, 50.0, 10.0], [10.0, 50.0], [false, true]),
        [15.0, 0.0, 45.0, 10.0]
    );
    assert_eq!(
        sliced_shadow([15.0, 0.0, 50.0, 10.0], [10.0, 50.0], [true, false]),
        [10.0, 0.0, 55.0, 10.0]
    );
}
