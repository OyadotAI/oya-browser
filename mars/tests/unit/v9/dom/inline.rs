/** Inline input arguments use current projected tree semantics while legacy APIs keep literal input data. */
use super::*;

/** A removed input makes the later argument throw after the earlier argument's effect; the error is logged. */
#[test]
fn inline_input_arguments_observe_removal() {
    let document = document(
        "<div id=parent><input id=name value=Ada></div><p id=out>initial</p><button id=button type=button onclick=\"receive(remove(),document.getElementById('name').value)\">Click</button>",
    );
    let source = "function remove(){document.getElementById('parent').textContent='removed';}function receive(a,b){document.getElementById('out').textContent=b;}";
    let clicked = click(source, load(source, &document).state, &document);
    assert_eq!(clicked.changes.len(), 1);
    assert_eq!(clicked.changes[0].text().unwrap(), "removed");
    assert!(clicked.effects.console[0].text.contains("TypeError"));
}

/** A duplicate id reads its first match, whose absent value property writes as empty text. */
#[test]
fn inline_input_arguments_read_the_first_duplicate_match() {
    let document = document(
        "<p id=name>first</p><input id=name value=Ada><p id=out>initial</p><button id=button type=button onclick=\"receive(document.getElementById('name').value)\">Click</button>",
    );
    let source = "function receive(value){document.getElementById('out').textContent=value;}";
    let clicked = click(source, load(source, &document).state, &document);
    assert_eq!(clicked.changes[0].text().unwrap(), "");
}

/** Supported inline reads still use freshly edited nonsecret values rather than the initial source attribute. */
#[test]
fn inline_input_arguments_read_current_permitted_projected_values() {
    let mut document = document(
        "<input id=name value=Ada><p id=out>initial</p><button id=button type=button onclick=\"receive(document.getElementById('name').value)\">Click</button>",
    );
    let source = "function receive(value){document.getElementById('out').textContent=value;}";
    let mut state = load(source, &document).state;
    for value in ["Lin", "Rust 🐙", ""] {
        document
            .nodes
            .iter_mut()
            .find(|node| node.html_id == "name")
            .unwrap()
            .value = Some(value.into());
        let clicked = click(source, state, &document);
        assert_eq!(clicked.changes[0].text().unwrap(), value);
        state = clicked.state;
    }
}

/** Source-only clients retain their documented explicit input map without needing a fabricated document. */
#[test]
fn source_only_inline_input_arguments_keep_explicit_literal_input_semantics() {
    let source = "function receive(value){document.getElementById('out').textContent=value;}";
    let loaded = mars::v9::load(source, &[]).unwrap();
    let clicked = event(
        source,
        "receive(document.getElementById('name').value)",
        loaded.state,
        &[mars::v9::InputValue {
            html_id: "name".into(),
            value: "literal Rust 🐙".into(),
        }],
    )
    .unwrap();
    assert_eq!(clicked.changes[0].text().unwrap(), "literal Rust 🐙");
}
