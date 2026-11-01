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

/** Controls and options in the scale fixtures below, named so the fixtures state their sizes. */
const INPUTS: usize = 1000;
const SELECTS: usize = 50;
const OPTIONS_PER_SELECT: usize = 20;
const FORMS: usize = 40;
const SCALE_OPTIONS: usize = 5000;
const SCALE_FILLER: usize = 20_000;

/** Controls are nodes, so a page past 256 of them parses and every one submits like any other. */
#[test]
fn a_page_with_more_than_256_controls_parses_and_each_control_submits() {
    let mut html = String::from("<form action=\"/s\">");
    for index in 0..INPUTS {
        html.push_str(&format!("<input name=\"i{index}\">"));
    }
    for index in 0..SELECTS {
        html.push_str(&format!("<select name=\"s{index}\">"));
        for option in 0..OPTIONS_PER_SELECT {
            html.push_str(&format!("<option value=\"{option}\">o{option}</option>"));
        }
        html.push_str("</select>");
    }
    html.push_str("</form>");
    let document = parse(&html).unwrap();
    let submitted = submission(&document, form_index(&document), None).unwrap();
    for index in 0..INPUTS {
        assert!(
            submitted.encoded.contains(&format!("i{index}=")),
            "i{index}"
        );
    }
    for index in 0..SELECTS {
        let name = format!("s{index}");
        let select = document
            .nodes
            .iter()
            .find(|n| n.attributes.get("name") == Some(&name))
            .unwrap();
        let last = select_options_of(&document, select.local_id)
            .last()
            .unwrap()
            .local_id;
        let edited = mars_eye::select(&document, select.local_id, last).unwrap();
        assert!(edited.nodes[last].selected, "{name}");
    }
}

/** A page past 32 forms parses too; forms are nodes, and the node budget already bounds them. */
#[test]
fn a_page_with_more_than_32_forms_parses() {
    let html: String = (0..FORMS)
        .map(|index| format!("<form action=\"/f{index}\"><input name=\"n{index}\"></form>"))
        .collect();
    let document = parse(&html).unwrap();
    assert_eq!(
        document.nodes.iter().filter(|n| n.tag == "form").count(),
        FORMS
    );
}

/** The first form in the document, the one the submission test submits. */
fn form_index(document: &Document) -> usize {
    document
        .nodes
        .iter()
        .find(|n| n.tag == "form")
        .unwrap()
        .local_id
}

/** A select's options in source order, through the public scoping rule. */
fn select_options_of(document: &Document, select: usize) -> Vec<&Node> {
    mars_eye::select_options(document, select)
}

/** Options are grouped per select in one pass, so 5000 options under one select (with the
node budget filled around them) still parse and select in time linear in the page. */
#[test]
fn five_thousand_options_under_one_select_scale_linearly() {
    let mut html = String::from("<select name=\"big\">");
    for option in 0..SCALE_OPTIONS {
        html.push_str(&format!("<option>choice {option}</option>"));
    }
    html.push_str("</select>");
    html.push_str(&"<p>filler</p>".repeat(SCALE_FILLER));
    let started = std::time::Instant::now();
    let document = parse(&html).unwrap();
    let select = document
        .nodes
        .iter()
        .find(|n| n.tag == "select")
        .unwrap()
        .local_id;
    let last = select_options_of(&document, select)
        .last()
        .unwrap()
        .local_id;
    let edited = mars_eye::select(&document, select, last).unwrap();
    let elapsed = started.elapsed();
    assert!(edited.nodes[last].selected);
    assert!(elapsed < SCALE_BUDGET, "{elapsed:?}");
}

/** Generous for a debug build; a quadratic option scan takes many times longer at this size. */
const SCALE_BUDGET: std::time::Duration = std::time::Duration::from_secs(4);

/** Identities that differ from vector positions (a node removed after parse, so every later
position shifts) still address the right controls in submission and option selection. */
#[test]
fn identities_that_differ_from_positions_address_the_right_controls() {
    let mut document = parse(
        "<i></i><form action=\"/s\"><input name=\"a\" value=\"1\"><select name=\"c\">\
         <option value=\"x\">x</option><option value=\"y\" selected>y</option></select>\
         <input type=\"checkbox\" name=\"k\" value=\"v\" checked></form>",
    )
    .unwrap();
    let leaf = document.nodes.iter().position(|n| n.tag == "i").unwrap();
    document.nodes.remove(leaf);
    assert_ne!(document.nodes[leaf].local_id, leaf);
    let form = document
        .nodes
        .iter()
        .find(|n| n.tag == "form")
        .unwrap()
        .local_id;
    assert_eq!(
        submission(&document, form, None).unwrap().encoded,
        "a=1&c=y&k=v"
    );
    let select = document
        .nodes
        .iter()
        .find(|n| n.tag == "select")
        .unwrap()
        .local_id;
    let option = document
        .nodes
        .iter()
        .find(|n| n.tag == "option" && n.attributes.get("value").map(String::as_str) == Some("x"))
        .unwrap()
        .local_id;
    let picked = mars_eye::select(&document, select, option).unwrap();
    assert_eq!(
        submission(&picked, form, None).unwrap().encoded,
        "a=1&c=x&k=v"
    );
}

/** Controls that name their form by attribute resolve owners through the index, so 20 forms of
500 controls validate and submit in time linear in the page, not forms times nodes. */
#[test]
fn twenty_forms_of_five_hundred_controls_validate_and_submit_quickly() {
    let mut html = String::new();
    for form in 0..FORM_COUNT {
        html.push_str(&format!("<form id=\"f{form}\" action=\"/s{form}\"></form>"));
    }
    for form in 0..FORM_COUNT {
        for control in 0..CONTROLS_PER_FORM {
            html.push_str(&format!(
                "<input form=\"f{form}\" name=\"n{control}\" value=\"v{control}\" required>"
            ));
        }
    }
    let started = std::time::Instant::now();
    let document = parse(&html).unwrap();
    let forms: Vec<usize> = document
        .nodes
        .iter()
        .filter(|n| n.tag == "form")
        .map(|n| n.local_id)
        .collect();
    for form in forms {
        let submitted = submission(&document, form, None).unwrap();
        assert!(submitted.encoded.contains("n499=v499"));
    }
    let elapsed = started.elapsed();
    assert!(elapsed < SCALE_BUDGET, "{elapsed:?}");
}

/** Forms in the scaling fixture. */
const FORM_COUNT: usize = 20;
/** Controls in each form of the scaling fixture. */
const CONTROLS_PER_FORM: usize = 500;
