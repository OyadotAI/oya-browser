/** `line-height: normal` after Blink's FontMetrics on a 2x Mac: ascent and descent rounded to
whole device pixels, plus the 15% ascent hack for Times and Helvetica. Expected values are what
Chromium reports for the same markup. */
use mars_eye::{compute_styles, parse, render};

/** Border-box height of each body child after laying out `markup` at 800px. */
fn heights(markup: &str) -> Vec<f64> {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let divs = document.nodes.iter().filter(|n| n.tag == "div");
    divs.map(|div| {
        let item = display
            .items
            .iter()
            .find(|i| i.local_id == div.local_id)
            .unwrap();
        item.height
    })
    .collect()
}

/** Helvetica (sans-serif) and Times (serif) at 16px are 18.5px tall, at 14px 16px. */
#[test]
fn classic_faces_round_at_device_pixels() {
    let markup = "<div style='font:16px sans-serif'>x</div><div style='font:16px serif'>x</div>\
                  <div style='font:14px sans-serif'>x</div><div style='font:10px serif'>x</div>";
    assert_eq!(heights(markup), [18.5, 18.5, 16.0, 11.5]);
}

/** Every normal line height lands on a device pixel (a half CSS px at 2x). */
#[test]
fn normal_line_heights_are_whole_device_pixels() {
    let sizes = [9.333, 11.0, 13.0, 13.333, 15.0, 17.0, 21.0];
    let markup: String = sizes
        .iter()
        .map(|size| format!("<div style='font-size:{size}px;font-family:sans-serif'>x</div>"))
        .collect();
    for height in heights(&markup) {
        assert_eq!((height * 2.0).fract(), 0.0, "{height}");
    }
}

/** A line-height smaller than a span's font gives that span negative leading (Blink's
 * CalculateLeadingSpace), so the line is 34.5px as in Chromium, not the span's content height. */
#[test]
fn negative_leading_shrinks_a_tall_inline_box() {
    let markup = "<div style='font:16px sans-serif;line-height:30px'>\
                  <span style='font-size:30px'>Big</span> small</div>";
    assert_eq!(heights(markup), [34.5]);
}
