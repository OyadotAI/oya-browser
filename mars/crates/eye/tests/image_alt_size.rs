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

/** The painted (width, height) of the element with `id` when every img still shows its alt text. */
fn size(markup: &str, id: &str) -> (f64, f64) {
    let mut document = parse(markup).unwrap();
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
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.width, item.height)
}

/** An unloaded img with a percentage width fills its block at that width, not at its alt text's
 * width: Chrome sizes it by its CSS width and shows the alt text inline within that box. */
#[test]
fn unloaded_block_image_with_percent_width_fills_its_container() {
    let markup = "<div style='width:526px'><img id=i alt='z' style='width:100%'></div>";
    let (width, height) = size(markup, "i");
    assert!((width - 526.0).abs() < 0.05, "width {width}");
    assert!(height > 0.0 && height < 40.0, "height {height}");
}

/** A broken img with an authored px width keeps that width, whatever its alt text measures. */
#[test]
fn unloaded_block_image_with_px_width_keeps_its_css_width() {
    let markup = "<div><img id=i alt='a much longer alt text than fits' style='width:200px'></div>";
    let (width, _) = size(markup, "i");
    assert!((width - 200.0).abs() < 0.05, "width {width}");
}
