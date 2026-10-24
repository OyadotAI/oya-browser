/** Form markup outside Eye's implemented controls falls back the way HTML's controls do. */
use mars_eye::{Document, Node, parse, set_value, submission};

/** The checkout fixture, parsed once per test. */
fn checkout() -> Document {
    parse(include_str!("../../../fixtures/compat/forms-types.html")).unwrap()
}

/** The input with this name. */
fn named<'a>(document: &'a Document, name: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.attributes.get("name").map(String::as_str) == Some(name))
        .unwrap_or_else(|| panic!("no {name}"))
}

/** Number and date inputs are edited as text fields. */
#[test]
fn typed_inputs_edit_as_text() {
    let document = checkout();
    for name in ["qty", "when"] {
        let input = named(&document, name);
        assert_eq!(input.input_type(), "text", "{name}");
        let edited = set_value(&document, input.local_id, "2").unwrap();
        assert_eq!(edited.nodes[input.local_id].value, "2");
    }
    assert!(document.diagnostics.iter().any(|d| d.contains("number")));
}

/** Unknown input types behave as text; known types are matched case-insensitively. */
#[test]
fn unknown_input_types_are_text() {
    let document = checkout();
    assert_eq!(named(&document, "fancy").input_type(), "text");
    assert_eq!(named(&document, "email").input_type(), "email");
}

/** Image inputs submit and reset inputs are inert buttons. */
#[test]
fn image_and_reset_inputs_fall_back() {
    let document = checkout();
    let image = document
        .nodes
        .iter()
        .find(|n| n.attributes.get("alt").is_some_and(|a| a == "Go"))
        .unwrap();
    assert_eq!(image.input_type(), "submit");
    let reset = document.nodes.iter().find(|n| n.value == "Clear").unwrap();
    assert_eq!(reset.input_type(), "button");
}

/** Color inputs stay visible but cannot be edited. */
#[test]
fn unimplemented_controls_are_static() {
    let document = checkout();
    let color = named(&document, "color");
    assert_eq!(color.input_type(), "color");
    assert!(set_value(&document, color.local_id, "#000000").is_err());
}

/** A multiple select keeps every authored selection and submits each. */
#[test]
fn multiple_select_submits_every_selection() {
    let document = parse(
        "<form id=f><select name=t multiple><option selected>Cheese<option>Olives<option selected>Basil</select></form>",
    )
    .unwrap();
    let form = document
        .nodes
        .iter()
        .find(|n| n.tag == "form")
        .unwrap()
        .local_id;
    assert_eq!(
        submission(&document, form, None).unwrap().encoded,
        "t=Cheese&t=Basil"
    );
}

/** Datalist, output, progress and meter parse as static content. */
#[test]
fn static_form_elements_parse() {
    let document = checkout();
    for tag in ["datalist", "output", "progress", "meter"] {
        assert!(document.nodes.iter().any(|n| n.tag == tag), "{tag}");
    }
    assert_eq!(
        document
            .nodes
            .iter()
            .find(|n| n.tag == "output")
            .unwrap()
            .text,
        "$10"
    );
}

/** Pattern constraints are noted rather than refusing the form. */
#[test]
fn pattern_attributes_do_not_refuse() {
    let document = checkout();
    assert_eq!(named(&document, "phone").attributes["pattern"], "[0-9]+");
    assert!(document.diagnostics.iter().any(|d| d.contains("pattern")));
}

/** A nested form start tag is ignored, so its controls belong to the outer form. */
#[test]
fn nested_form_start_tags_are_ignored() {
    let document = parse("<form id=a action=/a><form id=b action=/b><input name=q value=1></form><input name=r value=2></form>").unwrap();
    assert_eq!(document.nodes.iter().filter(|n| n.tag == "form").count(), 1);
    let form = document
        .nodes
        .iter()
        .find(|n| n.tag == "form")
        .unwrap()
        .local_id;
    assert_eq!(
        submission(&document, form, None).unwrap().encoded,
        "q=1&r=2"
    );
}

/** A submit button keeps its supported handler inside the form. */
#[test]
fn form_buttons_keep_working() {
    let document = checkout();
    let button = document.nodes.iter().find(|n| n.tag == "button").unwrap();
    assert_eq!(button.handler.as_deref(), Some("track()"));
    assert_eq!(button.input_type(), "submit");
}

/** A label wrapping more text than a control name allows is capped, not refused. */
#[test]
fn long_control_labels_are_capped() {
    let long = "word ".repeat(2000);
    let document = parse(&format!("<label>{long}<input id=i></label>")).unwrap();
    let input = document.nodes.iter().find(|n| n.tag == "input").unwrap();
    assert!(mars_eye::label(&document, input).len() <= mars_eye::constants::TEXT_BYTES);
}
