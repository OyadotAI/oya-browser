/**
 * Value forms Blink accepts that Eye used to drop: `env()` fallbacks inside `calc()`, and the
 * rest of the grammar gaps the YouTube and Airbnb stylesheets hit. One test per rule.
 */
use mars_eye::{Document, Node, compute_styles, parse};

/** Styles `markup` with one sheet at the default viewport. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
}
/** Whether the cascade dropped a declaration of `property`. */
fn dropped(document: &Document, property: &str) -> bool {
    document
        .diagnostics
        .iter()
        .any(|d| d.contains(property) && d.contains("not supported"))
}

#[test]
fn env_with_a_unitless_zero_fallback_is_a_length_inside_calc() {
    let document = styled(
        "<p id=a>a</p>",
        "#a{margin-top:calc(24px + env(safe-area-inset-top,0));top:calc(16px + env(safe-area-inset-top, 0))}",
    );
    assert!(!dropped(&document, "margin-top"));
    assert!(!dropped(&document, "top"));
    assert_eq!(by_id(&document, "a").style.margin[0], 24.0);
}

#[test]
fn env_with_a_length_fallback_keeps_that_length() {
    let document = styled("<p id=a>a</p>", "#a{margin-top:calc(8px + env(x,12px))}");
    assert_eq!(by_id(&document, "a").style.margin[0], 20.0);
}

#[test]
fn bare_numbers_stay_invalid_beside_lengths_outside_env() {
    let document = styled("<p id=a>a</p>", "#a{margin-top:calc(24px + 0)}");
    assert!(dropped(&document, "margin-top"));
}
