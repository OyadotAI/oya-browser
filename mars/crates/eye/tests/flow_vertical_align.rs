/** `vertical-align` after Blink's InlineLayoutStateStack::ApplyBaselineShift: sub and super
shift by a fifth and a third of the parent font plus one layout pixel, and lengths and
percentages (of the box's own line-height) raise the baseline. Expected values are Chromium's
for 16px sans-serif. */
use mars_eye::{compute_styles, parse, render};

/** (top of `#s`, line baseline from a zero-height marker) for a span styled `style`. */
fn place(style: &str) -> (f64, f64) {
    let markup = format!(
        "<body style='margin:0'><div style='font:16px sans-serif'>x<span id=m \
         style='display:inline-block;width:1px;height:0'></span><span id=s style='{style}'>q</span></div>"
    );
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let y = |id: &str| {
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
            .y
    };
    (y("s"), y("m"))
}

/** Within a sixty-fourth of a pixel of Chromium. */
fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 0.02
}

/** `sub` lowers the baseline by 16/5 plus half a CSS px: the span's top is 3.7px down. */
#[test]
fn sub_shift_adds_one_device_pixel() {
    assert!(
        near(place("vertical-align:sub").0, 3.7),
        "{:?}",
        place("vertical-align:sub")
    );
}

/** `super` raises it by 16/3 plus half a CSS px, so the line baseline sits at 20.83. */
#[test]
fn super_shift_adds_one_device_pixel() {
    assert!(
        near(place("vertical-align:super").1, 20.83),
        "{:?}",
        place("vertical-align:super")
    );
}

/** A length raises the baseline by that many pixels. */
#[test]
fn a_length_raises_the_baseline() {
    assert!(
        near(place("vertical-align:5px").1, 20.0),
        "{:?}",
        place("vertical-align:5px")
    );
}

/** A percentage is of the box's own line-height: -20% of 18.5px lowers it 3.7px. */
#[test]
fn a_percentage_uses_the_line_height() {
    assert!(
        near(place("vertical-align:-20%").0, 3.7),
        "{:?}",
        place("vertical-align:-20%")
    );
}
