/** SVG paint servers written as real markup survive parsing: `linearGradient`, `stop` and the
 * other definition elements reach the painter instead of being dropped as unsupported. */
use mars_eye::{compute_styles, parse, render};

/** The fill of the first painted path, styled and laid out without author sheets. */
fn fill(markup: &str) -> Option<String> {
    let document = compute_styles(&parse(markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    display
        .items
        .iter()
        .flat_map(|item| &item.vector)
        .next()
        .and_then(|path| path.fill.clone())
}

/** A gradient written in markup fills its shape, as brand logos (Node.js) need. */
#[test]
fn markup_gradient_fills_its_shape() {
    let markup = "<svg width=10 height=10><defs><linearGradient id=g>\
        <stop offset=0 stop-color='#ff0000'></stop><stop offset=1 stop-color='#0000ff'></stop>\
        </linearGradient></defs><path fill='url(#g)' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(markup).as_deref(), Some("#800080"));
}

/** A clipPath's own shapes are definitions and never paint where they are written. */
#[test]
fn clip_path_content_does_not_paint_in_place() {
    let markup = "<svg width=10 height=10><clipPath id=c><rect width=5 height=5 fill='#00ff00'></rect>\
        </clipPath><path fill='#123456' d='M0 0H10V10Z'></path></svg>";
    assert_eq!(fill(markup).as_deref(), Some("#123456"));
}
