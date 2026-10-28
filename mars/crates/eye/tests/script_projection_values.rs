/** What page scripts see of form values: a hidden input's value is the page's own data and is
visible, as in Chrome; a password field's never is. Google builds its search URL from hidden
inputs, and empty ones made Google answer "unusual traffic". */
use mars_eye::{parse, script_document};

/** The projected value of the element with `id`. */
fn projected(markup: &str, id: &str) -> (Option<String>, Option<String>) {
    let document = parse(markup).unwrap();
    let projection = script_document(&document, "page-1").unwrap();
    let node = projection.nodes.iter().find(|node| node.html_id == id).unwrap();
    (node.value.clone(), node.attributes.get("value").cloned())
}

/** Hidden inputs keep their value and value attribute. */
#[test]
fn hidden_input_values_reach_scripts() {
    let (value, attribute) = projected("<form><input type=hidden id=h name=ei value=abc></form>", "h");
    assert_eq!(value.as_deref(), Some("abc"));
    assert_eq!(attribute.as_deref(), Some("abc"));
}

/** Password inputs keep neither. */
#[test]
fn password_values_never_reach_scripts() {
    let (value, attribute) = projected("<form><input type=password id=p value=pw></form>", "p");
    assert_eq!(value, None);
    assert_eq!(attribute, None);
}
