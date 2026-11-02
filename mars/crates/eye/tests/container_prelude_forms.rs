/**
 * `@container` preludes Blink accepts. Every form parses without a diagnostic. A condition Eye cannot
 * evaluate (`scroll-state()`, a standard property) is Unknown per element, so its rules do not apply
 * and its hiding rules mask, while named, size, range and style queries evaluate as Blink does.
 */
use mars_eye::{ComputedStyle, Document, compute_styles, parse};

/** Styles `markup` with one sheet through the public cascade. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.to_owned()]).unwrap()
}

/** The computed style of the element with `id`. */
fn style<'a>(document: &'a Document, id: &str) -> &'a ComputedStyle {
    &document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .style
}

/** Every prelude form Blink accepts, each applied to a paragraph. */
const FORMS: [&str; 17] = [
    "(min-width: 890px)",
    "(width >= 130px)",
    "(744px <= width < 1440px)",
    "style(--gallery-navbar)",
    "style(--x: 1) and (min-width: 2px)",
    "card (min-width: 1px)",
    "card style(--a)",
    "not style(--a)",
    "(min-width: 1px) or (max-width: 2px)",
    "(orientation: portrait)",
    "(inline-size > 30em)",
    "(aspect-ratio: 16 / 9)",
    "scroll-state(stuck: top)",
    "(max-width:16rem)",
    "list-select-root (max-width:16rem)",
    "(block-size > 1px)",
    "style(--gallery-navbar: foo bar) and (not (width > 1px))",
];

#[test]
fn every_prelude_form_parses_without_a_container_diagnostic() {
    for form in FORMS {
        let document = styled(
            "<p id=a>a</p>",
            &format!("@container {form} {{ p {{ order: 9 }} }}"),
        );
        assert!(
            document
                .diagnostics
                .iter()
                .all(|d| !d.contains("@container")),
            "{form} was reported: {:?}",
            document.diagnostics
        );
    }
}

#[test]
fn a_scroll_state_query_applies_nothing_and_is_not_a_container_name() {
    let document = styled(
        "<p id=a>a</p>",
        "@container scroll-state(stuck: top) { p { order: 5 } }",
    );
    assert_eq!(style(&document, "a").order, 0);
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("@container"))
    );
}

#[test]
fn a_hiding_rule_under_scroll_state_masks_its_target() {
    let document = styled(
        "<button id=a type=button>a</button>",
        "@container scroll-state(stuck: top) { #a { display: none } }",
    );
    let a = style(&document, "a");
    assert!(!a.hidden);
    assert!(!a.interactive);
}

#[test]
fn a_range_query_reads_the_size_container() {
    let sheet = "#c{container-type:inline-size;width:500px}\
                 @container (400px <= width < 600px){p{order:4}}";
    let document = styled("<div id=c><p id=p>x</p></div>", sheet);
    assert_eq!(style(&document, "p").order, 4);
}

#[test]
fn a_range_query_outside_the_size_stays_false() {
    let sheet = "#c{container-type:inline-size;width:500px}\
                 @container (600px <= width){p{order:4}}";
    let document = styled("<div id=c><p id=p>x</p></div>", sheet);
    assert_eq!(style(&document, "p").order, 0);
}

#[test]
fn a_not_style_query_holds_when_the_property_is_absent() {
    let document = styled(
        "<div><p id=p>x</p></div>",
        "@container not style(--a){p{order:6}}",
    );
    assert_eq!(style(&document, "p").order, 6);
}

#[test]
fn a_named_container_query_ignores_containers_with_other_names() {
    let sheet = "#c{container-type:inline-size;container-name:card;width:500px}\
                 @container other (min-width: 1px){p{order:3}}";
    let document = styled("<div id=c><p id=p>x</p></div>", sheet);
    assert_eq!(style(&document, "p").order, 0);
}
