/** An img with no source yet sizes to its alt text as a flex item, as Blink's LayoutImage does. */
use mars_eye::{compute_styles, parse, render};

/** The painted width of the element with `id` when every img still shows its alt text. */
fn width(markup: &str, id: &str) -> f64 {
    let mut document = parse(markup).unwrap();
    // The runtime shows an unloaded image's alt as its text.
    for node in document.nodes.iter_mut().filter(|n| n.tag == "img") {
        node.text = node.image_alt.clone();
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|i| i.local_id == local)
        .unwrap()
        .width
}

/** A lazy img with alt in a flex row is as wide as its alt text, not zero. */
#[test]
fn unloaded_flex_item_image_takes_its_alt_width() {
    let markup = "<div style='display:flex'><img id=i alt='Profile photo of a person'></div>";
    let flex = width(markup, "i");
    let block = width("<span id=s>Profile photo of a person</span>", "s");
    assert!(
        flex > 0.0 && (flex - block).abs() < 1.0,
        "flex {flex} text {block}"
    );
}

/** Blink lays an alt-text image out as a block of its fallback text, so an authored height alone
 * (LinkedIn's lazy `h-[840px]` image) leaves the width at the alt text's width. */
#[test]
fn unloaded_flex_item_image_with_only_a_height_takes_its_alt_width() {
    let markup = "<div style='display:flex'><img id=i alt='Profile photo of a person' style='height:840px'></div>";
    let flex = width(markup, "i");
    let block = width("<span id=s>Profile photo of a person</span>", "s");
    assert!(
        flex > 0.0 && (flex - block).abs() < 1.0,
        "flex {flex} text {block}"
    );
}
