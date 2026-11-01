/** `min()`, `max()` and `clamp()` over mixed percentages and pixels on width, min-width and max-width resolve against the containing block at layout (BBC News' `max-width: min(calc(100% - 32px), 1008px)`). */
use mars_eye::{compute_styles, parse, render};

/** The painted width of the element with `id` in `markup` laid out at `viewport` px. */
fn width(markup: &str, viewport: f64, id: &str) -> f64 {
    let document = parse(markup).unwrap();
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, viewport).unwrap();
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

/** A block in a `container` px wide with the given inline style, measured by its id `i`. */
fn in_container(container: u32, style: &str) -> String {
    format!("<div style='width:{container}px'><div id=i style='{style}'></div></div>")
}

/** The percentage in `max-width: min(calc(100% - 32px), 1008px)` resolves against the container,
 * so a narrow container gives `100% - 32px`. */
#[test]
fn max_width_min_of_percent_calc_and_px_resolves_against_the_container() {
    let markup = in_container(500, "max-width:min(calc(100% - 32px),1008px)");
    assert!((width(&markup, 800.0, "i") - 468.0).abs() < 0.05);
}

/** A wide container is capped by the px operand of the same `min()`. */
#[test]
fn max_width_min_is_capped_by_its_px_operand_in_a_wide_container() {
    let markup = in_container(1200, "max-width:min(calc(100% - 32px),1008px)");
    assert!((width(&markup, 1300.0, "i") - 1008.0).abs() < 0.05);
}

/** `max()` takes the larger of its operands once resolved. */
#[test]
fn max_width_max_takes_the_larger_resolved_operand() {
    let markup = in_container(400, "max-width:max(50%,300px)");
    assert!((width(&markup, 800.0, "i") - 300.0).abs() < 0.05);
}

/** `clamp(low, value, high)` holds the percentage between its px bounds. */
#[test]
fn width_clamp_holds_a_percentage_between_its_px_bounds() {
    let low = in_container(300, "width:clamp(200px,50%,400px)");
    assert!((width(&low, 800.0, "i") - 200.0).abs() < 0.05);
    let high = in_container(1000, "width:clamp(200px,50%,400px)");
    assert!((width(&high, 1100.0, "i") - 400.0).abs() < 0.05);
    let middle = in_container(600, "width:clamp(200px,50%,400px)");
    assert!((width(&middle, 800.0, "i") - 300.0).abs() < 0.05);
}

/** `min()` in min-width raises a narrow box to the resolved minimum. */
#[test]
fn min_width_max_of_percent_and_px_raises_a_narrow_box() {
    let markup = in_container(400, "width:50px;min-width:max(50%,100px)");
    assert!((width(&markup, 800.0, "i") - 200.0).abs() < 0.05);
}

/** A `width` that is a `min()` of mixed operands is a sized box, not an auto one: it takes the
 * resolved length rather than filling the container. */
#[test]
fn width_min_of_percent_and_px_takes_the_resolved_length() {
    let markup = in_container(400, "width:min(60%,200px)");
    assert!((width(&markup, 800.0, "i") - 200.0).abs() < 0.05);
    let narrow = in_container(200, "width:min(60%,200px)");
    assert!((width(&narrow, 800.0, "i") - 120.0).abs() < 0.05);
}

/** A replaced img takes a `min()` width the same way as a block (its replaced sizing path). */
#[test]
fn img_max_width_min_of_percent_and_px_resolves_against_the_container() {
    let markup = "<div style='width:500px'><img id=i style='max-width:min(calc(100% - 32px),1008px);width:100%'></div>";
    let document = parse(markup).unwrap();
    let mut document = document;
    for node in document.nodes.iter_mut().filter(|n| n.tag == "img") {
        node.image_src = Some("x.png".into());
        node.image_size = Some((800, 400));
    }
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == "i")
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    assert!((item.width - 468.0).abs() < 0.05, "width {}", item.width);
}
