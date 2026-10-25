/** Script-driven state reaching style: html/body attribute writes, focus for `:focus` and
`:focus-within`, and the `revert` / `revert-layer` rollback in the cascade. */
use mars_eye::{Document, Node, ScriptChange, apply_script_changes, compute_styles, parse};

/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
}
/** The color of the node with `id` after styling `document` with `sheet`. */
fn color(document: &Document, sheet: &str, id: &str) -> Option<String> {
    let styled = compute_styles(document, &[sheet.into()]).unwrap();
    by_id(&styled, id).style.color.clone()
}

#[test]
fn a_script_removing_an_html_class_reaches_the_document_element() {
    let document = parse("<html class='reftest-wait dark'><p id=a>a</p></html>").unwrap();
    let change = ScriptChange::RootAttribute {
        body: false,
        name: "class".into(),
        value: Some("dark".into()),
    };
    let after = apply_script_changes(&document, &[change]).unwrap();
    assert_eq!(after.formatting.html.attributes["class"], "dark");
    let sheet = ":root.reftest-wait p{color:red} :root.dark p{color:green}";
    assert_eq!(color(&after, sheet, "a").as_deref(), Some("green"));
}

#[test]
fn a_script_writing_a_body_attribute_reaches_body_selectors() {
    let document = parse("<body><p id=a>a</p></body>").unwrap();
    let change = ScriptChange::RootAttribute {
        body: true,
        name: "class".into(),
        value: Some("on".into()),
    };
    let after = apply_script_changes(&document, &[change]).unwrap();
    assert_eq!(
        color(&after, "body.on p{color:green}", "a").as_deref(),
        Some("green")
    );
}

#[test]
fn root_attribute_writes_refuse_event_handlers() {
    let document = parse("<p>a</p>").unwrap();
    let change = ScriptChange::RootAttribute {
        body: false,
        name: "onload".into(),
        value: Some("x()".into()),
    };
    assert!(apply_script_changes(&document, &[change]).is_err());
}

/** `document` with focus moved to the node whose id is `id`. */
fn focused(document: &Document, id: &str) -> Document {
    let node = Some(by_id(document, id).local_id);
    apply_script_changes(document, &[ScriptChange::Focus { node }]).unwrap()
}

#[test]
fn focus_matches_the_focused_element_only() {
    let document = parse("<div id=a tabindex=1>a</div><div id=b>b</div>").unwrap();
    let after = focused(&document, "a");
    let sheet = "div{color:blue} div:focus{color:green}";
    assert_eq!(color(&after, sheet, "a").as_deref(), Some("green"));
    assert_eq!(color(&after, sheet, "b").as_deref(), Some("blue"));
    assert_eq!(color(&document, sheet, "a").as_deref(), Some("blue"));
}

#[test]
fn focus_within_matches_the_focused_element_and_its_ancestors() {
    let document =
        parse("<div id=outer><div id=inner><input id=field></div></div><p id=other>o</p>").unwrap();
    let after = focused(&document, "field");
    let sheet = "*{color:blue} :focus-within{color:green}";
    for id in ["outer", "inner", "field"] {
        assert_eq!(color(&after, sheet, id).as_deref(), Some("green"), "{id}");
    }
    assert_eq!(color(&after, sheet, "other").as_deref(), Some("blue"));
}

#[test]
fn blur_clears_focus_and_focus_within() {
    let document = parse("<div id=outer><input id=field></div>").unwrap();
    let after = focused(&document, "field");
    let blurred = apply_script_changes(&after, &[ScriptChange::Focus { node: None }]).unwrap();
    let sheet = "div{color:blue} div:focus-within{color:green}";
    assert_eq!(color(&blurred, sheet, "outer").as_deref(), Some("blue"));
}

#[test]
fn revert_layer_rolls_back_to_the_previous_layer() {
    let document = parse("<p id=a>a</p>").unwrap();
    let sheet = "@layer one { p{color:green} } @layer two { p{color:red; color:revert-layer} }";
    assert_eq!(color(&document, sheet, "a").as_deref(), Some("green"));
}

#[test]
fn revert_layer_in_unlayered_styles_rolls_back_to_the_last_layer() {
    let document = parse("<p id=a>a</p>").unwrap();
    let sheet = "@layer base { p{color:green} } p{color:revert-layer}";
    assert_eq!(color(&document, sheet, "a").as_deref(), Some("green"));
}

#[test]
fn revert_layer_through_every_layer_reaches_the_parent_value() {
    let document = parse("<div style='color:green'><p id=a>a</p></div>").unwrap();
    let sheet = "@layer x { p{color:revert-layer} } p{color:revert-layer}";
    assert_eq!(color(&document, sheet, "a").as_deref(), Some("green"));
}

#[test]
fn revert_rolls_back_the_whole_author_origin() {
    let document = parse("<div style='color:green'><p id=a>a</p></div>").unwrap();
    let sheet = "@layer x { p{color:red} } p{color:blue} p.z, p{color:revert}";
    assert_eq!(color(&document, sheet, "a").as_deref(), Some("green"));
}
