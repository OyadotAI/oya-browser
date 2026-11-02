/** Absolutely positioned sizing and placement, after Blink's absolute_utils.cc (css-position-3
inset-modified containing block, CSS 2.1 10.3.7 and 10.6.4). */
use mars_eye::{compute_styles, parse, render};

/** Lays out `inner` inside a 400 by 300 positioned box and returns `#a` as (x, y, width, height). */
fn abs(inner: &str) -> (f64, f64, f64, f64) {
    let markup = format!(
        "<body style='margin:0'><div style='position:relative;width:400px;height:300px'>{inner}</div>"
    );
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "a")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width, item.height)
}

/** Auto margins with both insets and a size share the free space: the classic centering. */
#[test]
fn auto_margins_center_between_both_insets() {
    let r = abs(
        "<div id=a style='position:absolute;inset:0;margin:auto;width:100px;height:50px'></div>",
    );
    assert_eq!(r, (150.0, 125.0, 100.0, 50.0));
}

/** An authored height wins over the stretch between two insets. */
#[test]
fn authored_height_beats_inset_stretch() {
    let r = abs("<div id=a style='position:absolute;top:10px;bottom:10px;height:50px'></div>");
    assert_eq!((r.1, r.3), (10.0, 50.0));
}

/** A percentage height resolves against the containing block even with one inset. */
#[test]
fn percent_height_resolves_against_the_containing_block() {
    let r = abs("<div id=a style='position:absolute;top:10%;left:25%;width:50%;height:50%'></div>");
    assert_eq!(r, (100.0, 30.0, 200.0, 150.0));
}

/** Only an end inset set: free space goes before the box, which hugs that edge. */
#[test]
fn end_inset_alone_places_from_the_end_edge() {
    let r = abs(
        "<div id=a style='position:absolute;right:20px;bottom:30px;width:100px;height:40px'></div>",
    );
    assert_eq!(r, (280.0, 230.0, 100.0, 40.0));
}

/** One auto margin takes the free space when both insets and the size are set. */
#[test]
fn a_single_auto_margin_absorbs_the_free_space() {
    let r = abs(
        "<div id=a style='position:absolute;left:0;right:0;width:100px;margin-left:auto;height:10px'></div>",
    );
    assert_eq!(r.0, 300.0);
}

/** An overconstrained inline axis with both margins auto keeps the start margin at zero. */
#[test]
fn overflowing_auto_margins_start_at_zero_inline() {
    let r = abs(
        "<div id=a style='position:absolute;left:0;right:0;width:500px;margin:0 auto;height:10px'></div>",
    );
    assert_eq!(r.0, 0.0);
}

/** An absolutely positioned image sizes as a replaced box: max-height scales its width. */
#[test]
fn positioned_image_uses_replaced_sizing() {
    let markup = "<body style='margin:0'><div style='position:relative;width:400px;height:300px'><img id=a style='position:absolute;top:0;left:0;max-height:50%'></div>";
    let mut document = parse(markup).unwrap();
    let img = document.nodes.iter_mut().find(|n| n.tag == "img").unwrap();
    img.image_src = Some("x.png".into());
    img.image_size = Some((600, 400));
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "a")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    assert_eq!((item.width, item.height), (225.0, 150.0));
}

/** An auto width takes the width `aspect-ratio` transfers from a definite height instead of
shrinking an empty box to nothing (Wikipedia's video play button). */
#[test]
fn aspect_ratio_transfers_an_authored_height_to_an_auto_width() {
    let r = abs(
        "<span id=a style='position:absolute;left:50%;top:50%;height:50px;\
         aspect-ratio:1/1;transform:translate(-50%,-50%)'></span>",
    );
    assert_eq!((r.2, r.3), (50.0, 50.0));
}
