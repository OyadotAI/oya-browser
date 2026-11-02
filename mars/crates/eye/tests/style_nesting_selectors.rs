/**
 * CSS nesting and complex selector arguments, after Blink's nesting resolution (`&` is the parent
 * list as `:is()`, nested declarations keep source order) and SelectorChecker's sub-selector
 * matching for `:is()`, `:where()` and `:not()`. Cases follow the CSS Nesting and Selectors 4
 * specs and WPT's css-nesting tests.
 */
use mars_eye::{Document, compute_styles, parse};

/** The color of `#t` after styling `markup` with `sheet`. */
fn color(markup: &str, sheet: &str) -> Option<String> {
    let document: Document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.color.clone()
}
/** `.a > .c > p#t.b`: an outer `.a`, an inner `.c`, and the subject. */
const TREE: &str = "<div class=a><div class=c><p id=t class=b>x</p></div></div>";

#[test]
fn ampersand_child_combinator_targets_the_parents_children() {
    let sheet = ".c { display: grid; & > * { color: green } }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}

#[test]
fn a_nested_selector_without_ampersand_is_a_descendant_of_the_parent() {
    assert_eq!(
        color(TREE, ".a { .b { color: green } }").as_deref(),
        Some("green")
    );
    assert_eq!(color(TREE, ".b { .a { color: red } }"), None);
}

#[test]
fn a_relative_nested_selector_starts_from_the_parent() {
    assert_eq!(
        color(TREE, ".c { > .b { color: green } }").as_deref(),
        Some("green")
    );
    assert_eq!(color(TREE, ".a { > .b { color: red } }"), None);
}

#[test]
fn a_trailing_ampersand_matches_the_whole_parent_selector_as_is() {
    // `.c :is(.a .b)`: `.a` is above `.c`, which a textual `.c .a .b` would miss.
    assert_eq!(
        color(TREE, ".a .b { .c & { color: green } }").as_deref(),
        Some("green")
    );
}

#[test]
fn ampersand_takes_the_highest_specificity_of_the_parent_list() {
    // `:is(#nope, .a) .b` weighs (1,1,0), beating `.a .c .b` at (0,3,0).
    let sheet = "#nope, .a { & .b { color: green } } .a .c .b { color: red }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}

#[test]
fn declarations_after_a_nested_rule_apply_after_it() {
    let sheet = ".b { color: red; & { color: blue } color: green }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}

#[test]
fn conditional_rules_nested_in_a_style_rule_apply_to_the_parent() {
    let sheet = ".b { @media (min-width: 1px) { color: green } }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}

#[test]
fn deep_nesting_composes_parents() {
    let sheet = ".a { .c { & .b { color: green } } }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}

#[test]
fn is_with_a_complex_argument_matches_from_the_element_itself() {
    assert_eq!(
        color(TREE, "p:is(.a .b) { color: green }").as_deref(),
        Some("green")
    );
    assert_eq!(
        color(TREE, ".c :is(.a .b) { color: green }").as_deref(),
        Some("green")
    );
    assert_eq!(color(TREE, "p:is(.c .a .b) { color: red }"), None);
}

#[test]
fn not_with_a_complex_argument_excludes_its_matches() {
    assert_eq!(color(TREE, "p:not(.a .b) { color: red }"), None);
    assert_eq!(
        color(TREE, "p:not(.x .b) { color: green }").as_deref(),
        Some("green")
    );
}

#[test]
fn where_with_a_complex_argument_has_no_specificity() {
    let sheet = ":where(.a .c) p { color: red } p { color: green }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}

#[test]
fn a_forgiving_list_drops_other_engines_prefixed_pseudos() {
    let sheet = ":is(.b, .x::-moz-focus-inner) { color: green }";
    assert_eq!(color(TREE, sheet).as_deref(), Some("green"));
}
