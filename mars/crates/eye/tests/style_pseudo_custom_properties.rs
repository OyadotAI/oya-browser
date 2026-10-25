/**
 * A `::before`/`::after` box is an element of its own for custom properties, as in Blink: its
 * own `--*` declarations feed `var()` in its `content` and other values, and it inherits the
 * originating element's. Tailwind 4 writes `content: var(--tw-content); --tw-content: ""`.
 */
use mars_eye::{compute_styles, parse};

/** The `::after` content of `#t` styled with `sheet`, if a box is generated. */
fn after(sheet: &str) -> Option<String> {
    let document = compute_styles(&parse("<p id=t>x</p>").unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style
        .generated
        .iter()
        .find(|generated| generated.after)
        .map(|generated| generated.content.clone())
}

#[test]
fn content_reads_a_custom_property_declared_on_the_pseudo_element() {
    assert_eq!(
        after(r#"#t::after { content: var(--c); --c: "hi" }"#),
        Some("hi".into())
    );
}

#[test]
fn content_reads_a_custom_property_inherited_from_the_element() {
    assert_eq!(
        after(r#"#t { --c: "up" } #t::after { content: var(--c) }"#),
        Some("up".into())
    );
}

#[test]
fn an_empty_string_through_var_still_generates_a_box() {
    assert_eq!(
        after(r#"#t::after { --c: ""; content: var(--c) }"#),
        Some(String::new())
    );
}

#[test]
fn an_unresolvable_var_generates_no_box() {
    assert_eq!(after("#t::after { content: var(--missing) }"), None);
}

#[test]
fn other_values_in_the_pseudo_element_read_its_own_properties() {
    let sheet = r#"#t::after { content: ""; --w: 30px; width: var(--w); display: block }"#;
    let document = compute_styles(&parse("<p id=t>x</p>").unwrap(), &[sheet.into()]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    assert_eq!(node.style.generated[0].style.width, Some(30.0));
}
