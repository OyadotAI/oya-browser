/**
 * `@property` registrations (CSS Properties and Values API; Blink's PropertyRegistration): a
 * registered custom property falls back to its initial value, and a non-inherited one reads
 * only its own element's declaration. Tailwind 4 relies on both (`--tw-border-style: solid`).
 */
use mars_eye::{Document, compute_styles, parse};

/** The color of `#t` in `<div class=p><p id=t>` styled with `sheet`. */
fn color(sheet: &str) -> Option<String> {
    let markup = "<div class=p><p id=t>x</p></div>";
    let document: Document = compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.color.clone()
}

#[test]
fn an_undeclared_registered_property_takes_its_initial_value() {
    let sheet = "@property --c { syntax: '<color>'; inherits: false; initial-value: green }\
                 p { color: var(--c, red) }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn a_non_inherited_registered_property_ignores_the_parents_value() {
    let sheet = "@property --c { syntax: '<color>'; inherits: false; initial-value: green }\
                 .p { --c: red } p { color: var(--c) }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn an_inherited_registered_property_still_inherits() {
    let sheet = "@property --c { syntax: '<color>'; inherits: true; initial-value: red }\
                 .p { --c: green } p { color: var(--c) }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn a_registration_without_inherits_is_invalid() {
    let sheet =
        "@property --c { syntax: '<color>'; initial-value: red } p { color: var(--c, green) }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn the_universal_syntax_needs_no_initial_value() {
    let sheet = "@property --c { syntax: '*'; inherits: false } p { color: var(--c, green) }";
    assert_eq!(color(sheet).as_deref(), Some("green"));
}

#[test]
fn unregistered_properties_inherit_as_before() {
    assert_eq!(
        color(".p { --c: green } p { color: var(--c) }").as_deref(),
        Some("green")
    );
}
