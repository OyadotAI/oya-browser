/** `filter: blur()` painting after Blink's FEGaussianBlur: a blurred solid box spreads into a
 * soft glow 3 sigma wider on every side, fading out instead of a hard opaque shape. */
use mars_eye::{DisplayBackground, DisplayItem, compute_styles, parse, render};

/** The painted item of `#b` styled by `css`. */
fn blurred(css: &str) -> DisplayItem {
    let markup = format!("<div id=b style='{css}'></div>");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    render(&document, 800.0)
        .unwrap()
        .items
        .into_iter()
        .find(|item| item.tag == "div")
        .expect("the box painted")
}

/** A blurred round box grows by 3 sigma and paints a ramp that is under full strength at its
 * center and transparent at its edge (nodejs.org's 300px glow painted as a solid green disc). */
#[test]
fn blurred_disc_paints_a_fading_glow() {
    let item =
        blurred("width:300px;height:300px;border-radius:50%;background:#99cc7d;filter:blur(120px)");
    assert!(item.background.is_none());
    assert_eq!(item.width, 300.0 + 6.0 * 120.0);
    let Some(DisplayBackground::Radial { stops, .. }) = item.backgrounds.first() else {
        panic!("no glow: {:?}", item.backgrounds);
    };
    let alpha = |color: &str| u8::from_str_radix(&color[7..9], 16).unwrap();
    let (center, edge) = (&stops[0].color, &stops.last().unwrap().color);
    assert!(
        center.len() == 9 && (100..200).contains(&alpha(center)),
        "{center}"
    );
    assert_eq!(alpha(edge), 0, "{edge}");
}

/** A slight blur of a square box keeps its solid fill: its edges barely move. */
#[test]
fn slight_blur_keeps_a_square_box() {
    let item = blurred("width:300px;height:100px;background:#123456;filter:blur(2px)");
    assert_eq!(item.background.as_deref(), Some("#123456"));
    assert!(item.backgrounds.is_empty());
}
