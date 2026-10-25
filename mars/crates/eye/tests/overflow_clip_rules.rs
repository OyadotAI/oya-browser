/** Overflow clip rules ported from Blink's LayoutBox::OverflowClipRect and scroll container
 * geometry: `overflow-clip-margin` reference boxes and lengths, and the clips that follow. */
use mars_eye::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** Styles `markup` with one sheet and lays it out 800px wide. */
fn laid(markup: &str, sheet: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}
/** The first item painted for the element with `id`. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** Blink parses the length with ValueRange::kAll: a negative margin pulls the clip edge inside
 * the reference box, so `padding-box -10px` on a 10px-padded box clips at its content box. */
#[test]
fn a_negative_clip_margin_pulls_the_clip_inside_its_reference_box() {
    let (document, display) = laid(
        "<div id=clip><div id=big></div></div>",
        "body{margin:0}#clip{width:100px;height:100px;padding:10px;overflow:clip;\
         overflow-clip-margin:padding-box -10px}#big{width:300px;height:300px}",
    );
    let clip = item(&document, &display, "big").clip.unwrap();
    assert_eq!(
        (clip.x, clip.y, clip.width, clip.height),
        (10.0, 10.0, 100.0, 100.0)
    );
}

/** The clip's corners outset with corner correction (css-backgrounds-3 corner shaping): a 10px
 * inner radius on a 100px box outset by 20px grows to 27.52px, not 30px, and a radius already
 * larger than the outset simply adds it. */
#[test]
fn clip_margin_corners_follow_the_corner_shaping_cubic() {
    let (document, display) = laid(
        "<div id=clip><div id=big></div></div>",
        "body{margin:0}#clip{width:100px;height:100px;border:5px solid;overflow:clip;\
         overflow-clip-margin:20px;border-radius:0 15px 25px 35px}#big{width:300px;height:300px}",
    );
    let clip = item(&document, &display, "big").clip.unwrap();
    assert_eq!((clip.x, clip.width), (-15.0, 140.0));
    assert!((clip.radius[1] - 27.52).abs() < 0.01, "{:?}", clip.radius);
    assert_eq!(
        [clip.radius[0], clip.radius[2], clip.radius[3]],
        [0.0, 40.0, 50.0]
    );
}

/** A box that clips only one axis (`overflow: clip visible`) clips square: Blink applies the
 * inner border-radius clip only when both axes clip. */
#[test]
fn a_one_axis_clip_ignores_border_radius() {
    let (document, display) = laid(
        "<div id=clip><div id=child></div></div>",
        "#clip{width:100px;height:50px;overflow:clip visible;border-radius:25px}\
         #child{width:200px;height:100px}",
    );
    let clip = item(&document, &display, "child").clip.unwrap();
    assert_eq!(clip.radius, [0.0; 4]);
    assert_eq!(clip.width, 100.0);
}
