/** Length units Blink's CSSPrimitiveValue accepts: viewport variants, root-relative and absolute. */
use mars_eye::{RenderContext, compute_styles_with_context, parse};

/** The computed width of `#t` for `width: <value>` in an 800x600 viewport. */
fn width(value: &str) -> Option<f64> {
    let context = RenderContext {
        width: 800.0,
        height: 600.0,
        ..RenderContext::default()
    };
    let sheet = format!("html {{ font-size: 10px }} #t {{ font-size: 20px; width: {value} }}");
    let document =
        compute_styles_with_context(&parse("<div id=t>x</div>").unwrap(), &[sheet], context)
            .unwrap();
    document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .style
        .width
}

#[test]
fn small_large_and_dynamic_viewport_units_equal_the_viewport_on_desktop() {
    assert_eq!(width("50dvw"), Some(400.0));
    assert_eq!(width("10svh"), Some(60.0));
    assert_eq!(width("10lvh"), Some(60.0));
    assert_eq!(width("10dvmin"), Some(60.0));
    assert_eq!(width("10vi"), Some(80.0));
    assert_eq!(width("10vb"), Some(60.0));
}

#[test]
fn absolute_units_convert_at_96_pixels_per_inch() {
    for value in ["2.54cm", "25.4mm", "101.6q", "1in", "72pt", "6pc"] {
        assert!((width(value).unwrap() - 96.0).abs() < 1e-9, "{value}");
    }
}

#[test]
fn root_relative_units_use_the_root_font() {
    assert_eq!(width("2rem"), Some(20.0));
    assert_eq!(width("2ric"), Some(20.0));
    assert_eq!(width("2ic"), Some(40.0));
}

/** `lh` and `rlh` are the element's and the root's used line heights: `normal` here (1.15 of
the font size, Eye's normal advance), an authored one when set. */
#[test]
fn line_height_units_use_the_normal_line_height() {
    assert_eq!(width("1lh"), Some(23.0));
    assert_eq!(width("1rlh"), Some(11.5));
}
