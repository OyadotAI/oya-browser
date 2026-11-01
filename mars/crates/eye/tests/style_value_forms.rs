/**
 * Value forms Blink accepts that Eye used to drop: `env()` fallbacks inside `calc()`, and the
 * rest of the grammar gaps the YouTube and Airbnb stylesheets hit. One test per rule.
 */
use mars_eye::{Document, Node, compute_styles, parse, render};

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

/** The (x, y, width, height) of the element with `id` in the rendered page at 800 points. */
fn rect(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let document = compute_styles(&parse(markup).unwrap(), &["body{margin:0}".to_owned()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = by_id(&document, id).local_id;
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap();
    (item.x, item.y, item.width, item.height)
}

#[test]
fn radius_calc_of_infinity_saturates_to_a_full_radius() {
    let document = styled("<p id=a>a</p>", "#a{border-radius:calc(infinity * 1px)}");
    assert!(!dropped(&document, "border-radius"));
    assert_eq!(
        by_id(&document, "a").style.radius[0],
        mars_eye::constants::CSS_MAX_SCALAR
    );
}

#[test]
fn radius_calc_of_negative_infinity_is_invalid() {
    let document = styled(
        "<p id=a>a</p>",
        "#a{border-top-left-radius:calc(-infinity * 1px)}",
    );
    assert!(dropped(&document, "border-top-left-radius"));
}

#[test]
fn radius_calc_of_a_huge_number_is_accepted() {
    let document = styled(
        "<p id=a>a</p>",
        "#a{border-top-left-radius:calc(1e30 * 1px)}",
    );
    assert!(!dropped(&document, "border-top-left-radius"));
}

#[test]
fn display_inline_list_item_is_an_inline_list_item() {
    let document = styled("<span id=a>a</span>", "#a{display:inline-list-item}");
    let style = &by_id(&document, "a").style;
    assert!(!dropped(&document, "display"));
    assert!(style.inline && style.list_item);
}

#[test]
fn webkit_match_parent_aliases_match_parent() {
    let document = styled(
        "<div id=p style=text-align:right><p id=a>a</p></div>",
        "#a{text-align:-webkit-match-parent}",
    );
    assert!(!dropped(&document, "text-align"));
    assert_eq!(by_id(&document, "a").style.align, "right");
}

#[test]
fn linear_easing_is_accepted_for_transitions_and_animations() {
    let document = styled(
        "<p id=a>a</p>",
        "#a{transition:transform 500ms linear(0,0.5,1);animation-timing-function:linear(0,1)}",
    );
    assert!(!dropped(&document, "transition"));
    assert!(!dropped(&document, "animation-timing-function"));
}

#[test]
fn malformed_linear_easing_drops_the_transition() {
    let document = styled(
        "<p id=a>a</p>",
        "#a{transition-timing-function:linear(0, nope)}",
    );
    assert!(dropped(&document, "transition-timing-function"));
}

#[test]
fn flex_shorthand_takes_a_calc_basis_as_one_token() {
    let document = styled("<p id=a>a</p>", "#a{flex:0 0 calc(50% - 16px / 2)}");
    assert!(!dropped(&document, "flex"));
    assert!(matches!(
        by_id(&document, "a").style.flex_basis,
        mars_eye::FlexBasis::Mixed { .. }
    ));
}

#[test]
fn flex_basis_mixed_calc_resolves_against_the_row() {
    let (x, _, width, _) = rect(
        "<div style='display:flex;width:400px'><p id=a style='flex:0 0 calc(50% - 20px);height:10px'>a</p></div>",
        "a",
    );
    assert_eq!((x, width), (0.0, 180.0));
}

#[test]
fn flex_basis_percent_calc_takes_its_share_of_the_free_space() {
    let (_, _, width, _) = rect(
        "<div style='display:flex;width:400px'><p id=a style='flex:1 0 calc((100% - 32px) * 0.35);height:10px'>a</p><p style='flex:1 1 0;height:10px'>b</p></div>",
        "a",
    );
    // The basis is 35% of 400 less 11.2px (128.8px); the 271.2px left over splits evenly.
    assert!((width - 264.4).abs() < 1e-9);
}

#[test]
fn list_style_type_string_is_the_marker_and_keeps_its_case() {
    let document = styled(
        "<ul><li id=a>a</li></ul><ul><li id=b>b</li></ul>",
        "#a{list-style-type:'Ab-'} #b{list-style-type:\"-\"}",
    );
    assert!(!dropped(&document, "list-style-type"));
    assert_eq!(
        by_id(&document, "a").style.list_marker.as_deref(),
        Some("Ab-")
    );
    assert_eq!(
        by_id(&document, "b").style.list_marker.as_deref(),
        Some("-")
    );
}

#[test]
fn a_keyword_list_style_type_clears_a_string_marker() {
    let document = styled(
        "<ul><li id=a>a</li></ul>",
        "#a{list-style-type:'*';list-style-type:DISC}",
    );
    assert!(by_id(&document, "a").style.list_marker.is_none());
}

#[test]
fn malformed_or_long_string_markers_are_dropped() {
    // Well past the marker bound, so the test does not depend on its exact value.
    let long = "x".repeat(40);
    let sheet = format!("#a{{list-style-type:'a' 'b'}} #b{{list-style-type:'{long}'}}");
    let document = styled("<ul><li id=a>a</li><li id=b>b</li></ul>", &sheet);
    assert!(dropped(&document, "list-style-type"));
    assert!(by_id(&document, "a").style.list_marker.is_none());
    assert!(by_id(&document, "b").style.list_marker.is_none());
}
