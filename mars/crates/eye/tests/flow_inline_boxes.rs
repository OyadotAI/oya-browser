/** Inline box geometry after Blink's inline layout: a non-atomic inline box covers the font's
content area around the baseline, not the whole line-height. */
use mars_eye::{compute_styles, parse, render};

/** (y, height) of `#id` after laying out `markup` at 800px with no body margin. */
fn place(markup: &str, id: &str) -> (f64, f64) {
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
    (item.y, item.height)
}

/** A tall line-height leaves half its leading above and below the span's box; the half above
 * is floored to a layout pixel (a half CSS px at 2x), as Blink's CalculateLeadingSpace does. */
#[test]
fn inline_box_excludes_the_leading() {
    let (line_y, line_h) = place(
        "<div id=d style='line-height:40px'><span id=s>x</span></div>",
        "d",
    );
    let (y, height) = place(
        "<div id=d style='line-height:40px'><span id=s>x</span></div>",
        "s",
    );
    assert!(height < line_h - 10.0, "{height}");
    let above = ((line_h - height) / 2.0 * 2.0).floor() / 2.0;
    assert!(((y - line_y) - above).abs() < 0.01, "{y} {height}");
}

/** Padding extends the inline box outward from the content area. */
#[test]
fn padding_extends_the_content_area() {
    let plain = place(
        "<div style='line-height:40px'><span id=s>x</span></div>",
        "s",
    );
    let padded = place(
        "<div style='line-height:40px'><span id=s style='padding:3px 0'>x</span></div>",
        "s",
    );
    assert!((padded.1 - plain.1 - 6.0).abs() < 0.01);
    assert!((plain.0 - padded.0 - 3.0).abs() < 0.01);
}

/** An inline box starts where it opens: its own leading space and its children's margins are
 * inside it, a space before it is not (HN's `<span> (<a>site</a>)</span>`). */
#[test]
fn the_box_spans_its_leading_space_and_child_margins() {
    let x = |markup: &str, id: &str| {
        let markup = format!("<body style='margin:0;font:16px sans-serif'>{markup}");
        let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
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
            .x
    };
    let own = x("<div>ab<span id=s> (c)</span></div>", "s");
    let outside = x("<div>ab <span id=s>(c)</span></div>", "s");
    assert!(outside - own > 3.0, "{own} {outside}");
    let margin = x(
        "<div><span id=s><b style='margin-left:5px'>a</b></span></div>",
        "s",
    );
    assert!(margin.abs() < 0.01, "{margin}");
}
