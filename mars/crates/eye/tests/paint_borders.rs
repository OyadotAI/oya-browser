/** Paint plumbing for painters outside Eye: every CSS color decodes through Eye's own parser,
 * and each border side's line style reaches the display list as Blink's `EBorderStyle` does. */
use mars_eye::{BorderStyle, DisplayItem, compute_styles, painter_rgba, parse, render};

/** The painted item of `#b` styled by `css`. */
fn painted(css: &str) -> DisplayItem {
    let markup = format!("<div id=b style='{css}'></div>");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    render(&document, 800.0)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.tag == "div")
        .expect("the box painted")
}

/** Named, hex, functional and mixed colors all decode, as the cascade accepts them. */
#[test]
fn painter_colors_decode_every_css_spelling() {
    assert_eq!(painter_rgba("orange"), Some([255, 165, 0, 255]));
    assert_eq!(painter_rgba("#0f08"), Some([0, 255, 0, 136]));
    assert_eq!(painter_rgba("rgb(0 128 0 / 50%)"), Some([0, 128, 0, 128]));
    assert_eq!(painter_rgba("hsl(240, 100%, 50%)"), Some([0, 0, 255, 255]));
    assert_eq!(painter_rgba("transparent"), Some([0, 0, 0, 0]));
    assert_eq!(painter_rgba("teal"), Some([0, 128, 128, 255]));
    assert_eq!(painter_rgba("not-a-color"), None);
}

/** Per-side styles reach the painter; solid stays the default. */
#[test]
fn border_styles_reach_the_display_list() {
    let item = painted("border:6px dashed red;border-left-style:double;border-bottom-style:groove");
    assert_eq!(
        item.border_styles,
        [
            BorderStyle::Dashed,
            BorderStyle::Dashed,
            BorderStyle::Groove,
            BorderStyle::Double
        ]
    );
    assert_eq!(
        painted("border:2px solid").border_styles,
        [BorderStyle::Solid; 4]
    );
}
