/** Shared inert DOM contracts reject malformed candidates independently of every engine. */
use mars_core::{
    constants::{DOCUMENT_DEPTH, ID_BYTES, TEXT_BYTES},
    dom::{DomChange, InsertPosition, ScriptDocument, ScriptNode},
};
use std::collections::BTreeMap;

/** Construct ordinary inert nodes with no native handles or capability grants. */
fn node(local_id: usize, parent: Option<usize>) -> ScriptNode {
    ScriptNode {
        selected: None,
        local_id,
        html_id: format!("node-{local_id}"),
        parent,
        value: None,
        text_writable: true,
        click_listener: false,
        inline_handler: None,
        tag: "p".into(),
        attributes: BTreeMap::new(),
    }
}

/** A body with `0 > 1` and a sibling `2`, in preorder. */
fn tree() -> ScriptDocument {
    ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: vec![node(0, None), node(1, Some(0)), node(2, None)],
    }
}

/** Vec order of identities, the order both engines must agree on. */
fn order(document: &ScriptDocument) -> Vec<(usize, Option<usize>)> {
    document
        .nodes
        .iter()
        .map(|n| (n.local_id, n.parent))
        .collect()
}

/** Every projection needs a bounded nonempty page identity before interpreter admission. */
#[test]
fn rejects_empty_and_oversized_projection_identities() {
    for identity in [String::new(), "x".repeat(ID_BYTES + 1)] {
        let document = ScriptDocument {
            identity,
            charset: "UTF-8".into(),
            nodes: vec![node(0, None)],
        };
        assert_eq!(document.validate().unwrap_err().code, "invalid_request");
    }
}

/** Parent references must resolve to earlier nodes, ruling out cycles and forward references. */
#[test]
fn rejects_forward_parent_references() {
    let document = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: vec![node(0, Some(1)), node(1, None)],
    };
    assert_eq!(document.validate().unwrap_err().code, "invalid_request");
}

/** Projection identities cannot alias two distinct source nodes. */
#[test]
fn rejects_duplicate_local_identities() {
    let document = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: vec![node(0, None), node(0, None)],
    };
    assert_eq!(document.validate().unwrap_err().code, "invalid_request");
}

/** The last accepted ancestor chain and first rejected chain retain their existing bound. */
#[test]
fn enforces_projection_depth_at_the_shared_boundary() {
    let mut document = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: (0..DOCUMENT_DEPTH)
            .map(|id| node(id, id.checked_sub(1)))
            .collect(),
    };
    document.validate().unwrap();
    document
        .nodes
        .push(node(DOCUMENT_DEPTH, Some(DOCUMENT_DEPTH - 1)));
    assert_eq!(document.validate().unwrap_err().code, "limit_exceeded");
}

/** A fragment's depth counts from where it is placed: one level under the deepest allowed node
is refused, one level under its parent is accepted, and a refusal changes nothing. */
#[test]
fn insert_counts_fragment_depth_from_the_insertion_point() {
    let chain = || ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: (0..DOCUMENT_DEPTH)
            .map(|id| node(id, id.checked_sub(1)))
            .collect(),
    };
    let mut document = chain();
    let deepest = DOCUMENT_DEPTH - 1;
    let error = document
        .insert(
            Some(deepest),
            InsertPosition::BeforeEnd,
            vec![node(DOCUMENT_DEPTH, None)],
        )
        .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(order(&document), order(&chain()));
    document
        .insert(
            Some(deepest),
            InsertPosition::AfterEnd,
            vec![node(DOCUMENT_DEPTH, None)],
        )
        .unwrap();
    assert_eq!(document.nodes.len(), DOCUMENT_DEPTH + 1);
    let nested = vec![
        node(DOCUMENT_DEPTH + 1, None),
        node(DOCUMENT_DEPTH + 2, Some(DOCUMENT_DEPTH + 1)),
    ];
    let error = document
        .insert(Some(deepest - 1), InsertPosition::BeforeEnd, nested)
        .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
}

/** Input values use the same UTF-8 byte budget before any engine receives the projection. */
#[test]
fn enforces_projection_input_text_at_the_shared_boundary() {
    let mut document = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: vec![node(0, None)],
    };
    document.nodes[0].value = Some("x".repeat(TEXT_BYTES));
    document.validate().unwrap();
    document.nodes[0].value.as_mut().unwrap().push('x');
    assert_eq!(document.validate().unwrap_err().code, "limit_exceeded");
}

/** A candidate replacement removes descendants without recycling surviving identities. */
#[test]
fn emits_a_text_proposal_and_updates_only_the_inert_tree() {
    let mut document = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: vec![node(0, None), node(1, Some(0)), node(2, None)],
    };
    document.validate().unwrap();
    let change = document.set_text(0, "replacement").unwrap();
    assert_eq!(change.html_id, "node-0");
    assert_eq!(change.text, "replacement");
    assert_eq!(
        document
            .nodes
            .iter()
            .map(|node| node.local_id)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    document.validate().unwrap();
}

/** Every insert position lands where insertAdjacentHTML puts it, reparenting only the root. */
#[test]
fn insert_positions_match_insert_adjacent_html() {
    let cases = [
        (
            InsertPosition::BeforeBegin,
            vec![(9, None), (10, Some(9)), (0, None), (1, Some(0)), (2, None)],
        ),
        (
            InsertPosition::AfterBegin,
            vec![
                (0, None),
                (9, Some(0)),
                (10, Some(9)),
                (1, Some(0)),
                (2, None),
            ],
        ),
        (
            InsertPosition::BeforeEnd,
            vec![
                (0, None),
                (1, Some(0)),
                (9, Some(0)),
                (10, Some(9)),
                (2, None),
            ],
        ),
        (
            InsertPosition::AfterEnd,
            vec![(0, None), (1, Some(0)), (9, None), (10, Some(9)), (2, None)],
        ),
    ];
    for (position, expected) in cases {
        let mut document = tree();
        document
            .insert(Some(0), position, vec![node(9, None), node(10, Some(9))])
            .unwrap();
        assert_eq!(order(&document), expected, "{position:?}");
    }
}

/** The body wrapper accepts only first-child and last-child insertion. */
#[test]
fn body_insertion_refuses_sibling_positions() {
    let mut document = tree();
    document
        .insert(None, InsertPosition::AfterBegin, vec![node(9, None)])
        .unwrap();
    assert_eq!(order(&document)[0], (9, None));
    for position in [InsertPosition::BeforeBegin, InsertPosition::AfterEnd] {
        let error = tree()
            .insert(None, position, vec![node(9, None)])
            .unwrap_err();
        assert_eq!(error.code, "unsupported_feature");
    }
}

/** Inserted identities cannot collide, exceed NODES or point outside the fragment. */
#[test]
fn insert_refuses_bad_identities_and_leaves_the_projection_unchanged() {
    for inserted in [
        vec![node(1, None)],
        vec![node(9, None), node(9, None)],
        vec![node(mars_core::constants::NODES, None)],
        vec![node(9, Some(0))],
    ] {
        let mut document = tree();
        assert!(
            document
                .insert(Some(0), InsertPosition::AfterEnd, inserted)
                .is_err()
        );
        assert_eq!(order(&document), order(&tree()));
    }
}

/** Moving keeps the subtree together and its identities; prepend to body is AfterBegin. */
#[test]
fn relocate_moves_whole_subtrees() {
    let mut document = tree();
    let change = document
        .relocate(2, None, InsertPosition::AfterBegin)
        .unwrap();
    assert!(matches!(
        change,
        DomChange::Move {
            node: 2,
            target: None,
            ..
        }
    ));
    assert_eq!(order(&document), vec![(2, None), (0, None), (1, Some(0))]);
    let mut document = tree();
    document
        .relocate(0, Some(2), InsertPosition::BeforeEnd)
        .unwrap();
    assert_eq!(
        order(&document),
        vec![(2, None), (0, Some(2)), (1, Some(0))]
    );
}

/** A node cannot move into itself or its own descendants. */
#[test]
fn relocate_refuses_moves_into_own_subtree() {
    for target in [0, 1] {
        let mut document = tree();
        assert!(
            document
                .relocate(0, Some(target), InsertPosition::BeforeEnd)
                .is_err()
        );
        assert_eq!(order(&document), order(&tree()));
    }
}

/** Only lang and dir are writable from scripts. */
#[test]
fn set_attribute_refuses_names_outside_the_allowlist() {
    let mut document = tree();
    for name in ["id", "href", "src", "style", "onclick", "LANG"] {
        assert_eq!(
            document.set_attribute(1, name, "x").unwrap_err().code,
            "unsupported_feature"
        );
    }
    document.set_attribute(1, "lang", "fr").unwrap();
    assert_eq!(document.nodes[1].attributes["lang"], "fr");
}

/** The digest is stable for equal structure and changes with order, parent, tag or attributes. */
#[test]
fn digest_is_stable_and_sensitive_to_structure() {
    assert_eq!(tree().digest(), tree().digest());
    let base = tree().digest();
    let mut moved = tree();
    moved.relocate(2, None, InsertPosition::AfterBegin).unwrap();
    let mut tagged = tree();
    tagged.nodes[2].tag = "div".into();
    let mut attributed = tree();
    attributed.set_attribute(2, "lang", "en").unwrap();
    for other in [moved, tagged, attributed] {
        assert_ne!(other.digest(), base);
    }
}

/** Subtrees must be contiguous: a child cannot reappear after an unrelated sibling. */
#[test]
fn rejects_non_contiguous_subtrees() {
    let document = ScriptDocument {
        identity: "page".into(),
        charset: "UTF-8".into(),
        nodes: vec![node(0, None), node(2, None), node(1, Some(0))],
    };
    assert_eq!(document.validate().unwrap_err().code, "invalid_request");
}

/** Text proposals keep their wire shape inside the structural log. */
#[test]
fn text_changes_serialize_as_tagged_dom_changes() {
    let change: DomChange = mars_core::dom::TextChange {
        html_id: "a".into(),
        text: "b".into(),
    }
    .into();
    assert_eq!(
        serde_json::to_string(&change).unwrap(),
        r#"{"op":"text","html_id":"a","text":"b"}"#
    );
}
