/**
 * Pseudo-elements and pseudo-classes Blink accepts. A valid one Eye cannot render keeps its rule and
 * never matches; an invalid one (another engine's, or out of place) drops only its own selector, silently,
 * as Blink does; `:is()` drops its invalid arguments; `:focus-visible` and range states match what they say.
 */
use mars_eye::{
    ComputedStyle, Document, Node, RenderContext, compute_styles, compute_styles_with_context,
    parse,
};

/** Styles `markup` with one sheet through the public cascade. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.to_owned()]).unwrap()
}

/** The node with `id`. */
fn node<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
}

/** The computed style of the element with `id`. */
fn style<'a>(document: &'a Document, id: &str) -> &'a ComputedStyle {
    &node(document, id).style
}

/** The color of the element with `id`, when a rule set one. */
fn color(document: &Document, id: &str) -> Option<String> {
    style(document, id).color.clone()
}

/** Whether any diagnostic mentions `needle`. */
fn reported(document: &Document, needle: &str) -> bool {
    document.diagnostics.iter().any(|d| d.contains(needle))
}

#[test]
fn a_valid_pseudo_element_rule_parses_and_never_styles_its_element() {
    let document = styled("<p id=a>a</p>", "p::selection{color:red}");
    assert_eq!(color(&document, "a"), None);
    assert!(!reported(&document, "pseudo-element"));
}

#[test]
fn a_webkit_pseudo_element_does_not_drop_the_rest_of_its_list() {
    let sheet = "body::-webkit-scrollbar-thumb, #a::-webkit-slider-thumb, \
                 #a::-webkit-inner-spin-button, #a::-webkit-calendar-picker-indicator, #a{color:red}";
    let document = styled("<input id=a>", sheet);
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert!(!reported(&document, "pseudo-element"));
}

#[test]
fn view_transition_pseudo_elements_are_valid_and_silent() {
    let sheet = "::view-transition-group(*){opacity:0} ::view-transition-old(root){opacity:0} \
                 ::view-transition-image-pair(screen){opacity:0} ::view-transition{opacity:0} \
                 #a{color:red}";
    let document = styled("<p id=a>a</p>", sheet);
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert!(!reported(&document, "pseudo-element"));
}

#[test]
fn a_pseudo_element_may_be_followed_by_a_state_in_its_compound() {
    let sheet = "#a::-webkit-scrollbar-track-piece:end, #a{color:red}";
    let document = styled("<p id=a>a</p>", sheet);
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert!(!reported(&document, "pseudo"));
}

#[test]
fn a_pseudo_element_in_the_middle_of_a_selector_is_invalid_and_silent() {
    let document = styled("<p id=a>a</p>", "#a::selection span, #a{color:red}");
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert!(!reported(&document, "pseudo"));
}

#[test]
fn a_pseudo_element_followed_by_a_class_is_invalid() {
    let document = styled("<p id=a class=x>a</p>", "#a::selection.x{color:red}");
    assert_eq!(color(&document, "a"), None);
}

#[test]
fn two_pseudo_elements_in_one_selector_are_invalid() {
    let document = styled("<p id=a>a</p>", "#a::selection::selection{color:red}");
    assert_eq!(color(&document, "a"), None);
}

#[test]
fn a_pseudo_element_inside_a_selector_function_is_invalid() {
    let document = styled("<p id=a>a</p>", ":is(#a::selection){color:red}");
    assert_eq!(color(&document, "a"), None);
}

#[test]
fn another_engines_pseudo_element_is_invalid_and_leaves_the_list_alone() {
    let sheet = "#a::-moz-focus-inner{color:blue}, #a::-ms-clear{color:blue}, #a{color:red}";
    let document = styled("<input id=a>", sheet);
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
    assert!(!reported(&document, "pseudo"));
}

#[test]
fn another_engines_pseudo_class_is_invalid_and_silent() {
    let document = styled(
        "<input id=a>",
        "#a:-moz-focusring{color:blue} #a:-ms-input-placeholder{color:blue}",
    );
    assert_eq!(color(&document, "a"), None);
    assert!(!reported(&document, "pseudo-class"));
}

#[test]
fn is_drops_an_invalid_argument_and_keeps_the_rest() {
    let document = styled("<p id=a>a</p>", ":is(#a:-moz-focusring, #a){color:red}");
    assert_eq!(color(&document, "a").as_deref(), Some("red"));
}

#[test]
fn not_with_an_invalid_argument_drops_its_whole_selector() {
    let document = styled("<p id=a>a</p>", "p:not(:-moz-focusring){color:red}");
    assert_eq!(color(&document, "a"), None);
}

#[test]
fn an_unknown_pseudo_class_still_masks_its_hiding_rule() {
    let document = styled(
        "<button id=a type=button>a</button>",
        "#a:made-up-state{display:none}",
    );
    assert!(reported(&document, "pseudo-class"));
    assert!(!style(&document, "a").interactive);
}

#[test]
fn valid_window_and_scrollbar_states_parse_without_a_diagnostic() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a:window-inactive, #b:horizontal{color:red}",
    );
    assert_eq!(color(&document, "a"), None);
    assert_eq!(color(&document, "b"), None);
    assert!(!reported(&document, "pseudo-class"));
}

#[test]
fn supports_selector_reports_what_blink_accepts() {
    let document = styled(
        "<p id=a>a</p>",
        "@supports selector(::-webkit-scrollbar) { #a{order:7} } \
         @supports selector(:-moz-focusring) { #a{order:8} }",
    );
    assert_eq!(style(&document, "a").order, 7);
}

#[test]
fn focus_visible_matches_a_focused_text_field() {
    let mut document = parse("<input id=a><button id=b>b</button>").unwrap();
    document.nodes[0].focused = true;
    let sheet = "input:focus-visible{color:red} button:focus-visible{color:blue}";
    let focused =
        compute_styles_with_context(&document, &[sheet.into()], RenderContext::default()).unwrap();
    assert_eq!(color(&focused, "a").as_deref(), Some("red"));
    assert_eq!(color(&focused, "b"), None);
}

#[test]
fn focus_visible_does_not_match_an_unfocused_field() {
    let document = styled("<input id=a>", "input:focus-visible{color:red}");
    assert_eq!(color(&document, "a"), None);
}

/** Eye edits number and date inputs as text and cannot tell when Blink would call one out of
 * range, so a range state stays unsupported and its hiding rule masks rather than failing open. */
#[test]
fn range_states_stay_unsupported() {
    let document = styled(
        "<input id=a type=number min=1 max=5 value=99>",
        "input:out-of-range{color:blue}",
    );
    assert_eq!(color(&document, "a"), None);
    assert!(reported(&document, "pseudo-class"));
}

#[test]
fn a_hover_rule_is_kept_and_never_matches() {
    let document = styled("<p id=a>a</p>", "p:hover{color:red} p:active{color:blue}");
    assert_eq!(color(&document, "a"), None);
    assert!(!reported(&document, "pseudo-class"));
}

/** Blink drops a whole rule when one selector in its list is invalid, so the valid ones do not apply. */
#[test]
fn one_invalid_selector_drops_the_whole_list() {
    let document = styled("<p id=a>a</p>", "p, p::-moz-focus-inner{color:red}");
    assert_eq!(color(&document, "a"), None);
}

/** `::part()` and `::slotted()` select real elements, so a hiding rule on one masks instead of matching nothing. */
#[test]
fn element_backed_pseudo_elements_stay_unsupported() {
    let document = styled("<p id=a>a</p>", "p::part(x){display:none}");
    assert!(reported(&document, "pseudo-element"));
}

/** `::first-line` can hide a page's own text, so out of place it stays unsupported and masks. */
#[test]
fn text_pseudo_elements_stay_unsupported() {
    let document = styled("<p id=a>a</p>", "p::first-line:hover{display:none}");
    assert!(reported(&document, "pseudo-element"));
}
