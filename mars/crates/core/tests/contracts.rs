/** Charset labels, local-id lookups, script modes and the validator's scale share one contract. */
use mars_core::{
    constants::{DOCUMENT_DEPTH, NODES},
    dom::{ScriptDocument, ScriptMode, ScriptNode, local_position},
    http::{UTF_8, WINDOWS_1252, encoding_for_label},
};

/** Every WHATWG spelling resolves to its encoding regardless of case and surrounding space. */
#[test]
fn charset_labels_resolve_by_encoding() {
    assert_eq!(encoding_for_label("utf-8"), Some(UTF_8));
    assert_eq!(encoding_for_label("unicode-1-1-utf-8"), Some(UTF_8));
    assert_eq!(encoding_for_label(" \tUTF8\n"), Some(UTF_8));
    assert_eq!(encoding_for_label("ISO-8859-1"), Some(WINDOWS_1252));
    assert_eq!(encoding_for_label("us-ascii"), Some(WINDOWS_1252));
    assert_eq!(encoding_for_label(" Latin1 "), Some(WINDOWS_1252));
    assert_eq!(encoding_for_label("shift_jis"), Some("Shift_JIS"));
    assert_eq!(encoding_for_label("x-user-defined"), Some("x-user-defined"));
    assert_eq!(encoding_for_label("nope"), None);
    assert_eq!(encoding_for_label(""), None);
}

/** Unmoved ids hit their index, moved ids fall back to the first match, missing ids are None. */
#[test]
fn local_position_matches_a_linear_find() {
    let ids = [0, 1, 2, 3];
    assert_eq!(local_position(&ids, 2, |id| *id), Some(2));
    let moved = [0, 7, 1, 7];
    assert_eq!(local_position(&moved, 7, |id| *id), Some(1));
    assert_eq!(local_position(&moved, 1, |id| *id), Some(2));
    assert_eq!(local_position(&moved, 9, |id| *id), None);
}

/** Declaration order is execution order, and the wire names are lowercase. */
#[test]
fn script_modes_order_and_serialize() {
    assert_eq!(ScriptMode::default(), ScriptMode::Blocking);
    assert!(ScriptMode::Blocking < ScriptMode::Defer && ScriptMode::Defer < ScriptMode::Async);
    for (mode, name) in [
        (ScriptMode::Blocking, "\"blocking\""),
        (ScriptMode::Defer, "\"defer\""),
        (ScriptMode::Async, "\"async\""),
    ] {
        assert_eq!(serde_json::to_string(&mode).unwrap(), name);
        assert_eq!(serde_json::from_str::<ScriptMode>(name).unwrap(), mode);
    }
    assert!(serde_json::from_str::<ScriptMode>("\"Defer\"").is_err());
}

/** A projected node with only the fields validation needs. */
fn node(local_id: usize, parent: Option<usize>) -> ScriptNode {
    ScriptNode {
        local_id,
        html_id: String::new(),
        parent,
        value: None,
        text_writable: false,
        click_listener: false,
        inline_handler: None,
        tag: "div".into(),
        attributes: Default::default(),
    }
}

/** A document whose nodes are given in preorder. */
fn document(nodes: Vec<ScriptNode>) -> ScriptDocument {
    ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes,
    }
}

/** A maximum-size projection validates, and one node more is refused with the limit. */
#[test]
fn validation_accepts_nodes_and_refuses_one_more() {
    let chain = |count: usize| {
        (0..count)
            .map(|id| node(id, (id % DOCUMENT_DEPTH != 0).then(|| id - 1)))
            .collect::<Vec<_>>()
    };
    document(chain(NODES)).validate().unwrap();
    let error = document(chain(NODES + 1)).validate().unwrap_err();
    assert!(format!("{error:?}").contains("document nodes"), "{error:?}");
}

/** Depth counts ancestors through the parent's recorded count, refusing the 32nd ancestor. */
#[test]
fn validation_refuses_documents_past_the_depth_limit() {
    let deep = |count: usize| (0..count).map(|id| node(id, id.checked_sub(1))).collect();
    document(deep(DOCUMENT_DEPTH)).validate().unwrap();
    let error = document(deep(DOCUMENT_DEPTH + 1)).validate().unwrap_err();
    assert!(format!("{error:?}").contains("document depth"), "{error:?}");
}
