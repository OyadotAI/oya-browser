/** Relative offsets and overflow clip edges, after Blink's relative_utils.cc
(ComputeRelativeOffset) and LayoutBox::OverflowClipRect with overflow-clip-margin. */
use mars_eye::{DisplayItem, compute_styles, parse, render};

/** Renders `body` at 800px and returns `#a`'s display item. */
fn item(body: &str) -> DisplayItem {
    let markup = format!("<body style='margin:0'>{body}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "a")
        .unwrap()
        .local_id;
    display
        .items
        .into_iter()
        .find(|i| i.local_id == local)
        .unwrap()
}

/** `#a`'s clip as (x, y, width, height). */
fn clip(body: &str) -> (f64, f64, f64, f64) {
    let clip = item(body).clip.expect("clipped");
    (clip.x, clip.y, clip.width, clip.height)
}

/** A percentage `top` against an auto-height parent is indefinite, so it acts as `auto`. */
#[test]
fn percent_top_in_an_auto_height_parent_does_not_move() {
    let a = item("<div><div id=a style='position:relative;top:100%;height:50px'></div></div>");
    assert_eq!(a.y, 0.0);
}

/** A percentage `top` resolves against the parent's authored content height. */
#[test]
fn percent_top_resolves_against_a_definite_parent_height() {
    let a = item(
        "<div style='height:80px;padding:10px'><div id=a style='position:relative;top:50%;height:5px'></div></div>",
    );
    assert_eq!(a.y, 10.0 + 40.0);
}

/** In a table, rows' percentages see the section's height, not their own. */
#[test]
fn percent_top_on_a_table_row_ignores_the_rows_own_height() {
    let a = item(
        "<table style='border-spacing:0'><tbody><tr style='position:relative;top:100%'><td style='padding:0'><div id=a style='height:100px;width:10px'></div></td></tr></tbody></table>",
    );
    assert_eq!(a.y, 0.0);
}

/** Percent `left` resolves against the parent's content width, not its border box. */
#[test]
fn percent_left_resolves_against_the_content_width() {
    let a = item(
        "<div style='width:200px;padding:0 50px'><div id=a style='position:relative;left:50%;height:5px'></div></div>",
    );
    assert_eq!(a.x, 50.0 + 100.0);
}

/** With both `left` and `right` set, an rtl container's `right` wins. */
#[test]
fn rtl_container_lets_right_win() {
    let a = item(
        "<div style='direction:rtl;width:200px'><div id=a style='position:relative;left:10px;right:30px;height:5px'></div></div>",
    );
    assert_eq!(a.x, -30.0);
}

/** `overflow: clip` lets content paint `overflow-clip-margin` past the padding box. */
#[test]
fn clip_margin_outsets_the_padding_box() {
    let r = clip(
        "<div style='overflow:clip;overflow-clip-margin:10px;width:100px;height:100px'><div id=a style='width:300px;height:300px'></div></div>",
    );
    assert_eq!(r, (-10.0, -10.0, 120.0, 120.0));
}

/** The `content-box` reference box pulls the clip edge in to the content box. */
#[test]
fn clip_margin_content_box_clips_at_the_content_edge() {
    let r = clip(
        "<div style='overflow:clip;overflow-clip-margin:content-box;padding:10px;width:100px;height:100px'><div id=a style='width:300px;height:300px'></div></div>",
    );
    assert_eq!(r, (10.0, 10.0, 100.0, 100.0));
}

/** A scroll container ignores `overflow-clip-margin`. */
#[test]
fn clip_margin_is_ignored_on_a_scroller() {
    let r = clip(
        "<div style='overflow:hidden;overflow-clip-margin:10px;width:100px;height:100px'><div id=a style='width:300px;height:300px'></div></div>",
    );
    assert_eq!(r, (0.0, 0.0, 100.0, 100.0));
}

/** `contain: paint` clips its content and honours the margin. */
#[test]
fn paint_containment_clips_with_the_margin() {
    let r = clip(
        "<div style='contain:paint;overflow-clip-margin:5px;width:100px;height:100px'><div id=a style='width:300px;height:300px'></div></div>",
    );
    assert_eq!(r, (-5.0, -5.0, 110.0, 110.0));
}

/** `overflow-x: clip` beside `overflow-y: visible` clips only horizontally. */
#[test]
fn clip_on_one_axis_leaves_the_other_open() {
    let r = clip(
        "<div style='overflow-x:clip;width:100px;height:100px'><div id=a style='width:300px;height:300px'></div></div>",
    );
    assert_eq!((r.0, r.2), (0.0, 100.0));
    assert!(r.3 > 10_000.0);
}

/** `overflow-clip-margin: inherit` takes the parent's margin even where it had no effect. */
#[test]
fn clip_margin_inherits_explicitly() {
    let r = clip(
        "<div style='overflow-clip-margin:20px'><div style='overflow:clip;overflow-clip-margin:inherit;width:100px;height:100px'><div id=a style='width:300px;height:300px'></div></div></div>",
    );
    assert_eq!(r, (-20.0, -20.0, 140.0, 140.0));
}
