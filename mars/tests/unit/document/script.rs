/** Script projections derive bounded capabilities from the actual owned document tree. */
use mars::{
    constants::{ID_BYTES, TEXT_BYTES},
    document::{ScriptDocument, TextChange, apply, compute_styles, parse, render, script_document},
};

/** Parsing ensures tests exercise author IDs and real native target policy together. */
fn projection(html: &str) -> ScriptDocument {
    let parsed = parse(html).unwrap();
    let styled = compute_styles(&parsed, &[]).unwrap();
    script_document(&styled, "page-a").unwrap()
}

/** Missing author IDs remain invisible; duplicate author IDs select the first current element. */
#[test]
fn lookup_uses_actual_nonempty_author_ids_in_tree_order() {
    let document =
        projection("<p>anonymous</p><p>also anonymous</p><p id=same>first</p><p id=same>last</p>");
    assert_eq!(document.get_by_id(""), None);
    assert_eq!(document.get_by_id("missing"), None);
    assert_eq!(document.get_by_id("@0"), None);
    assert_eq!(document.get_by_id("same"), Some(2));
    assert!(document.nodes[2].text_writable);
    assert!(!document.nodes[3].text_writable);
}

/** Password values never cross into V9 even when scripts know the element identifier; hidden
values are the page's own and scripts read them, as in Chrome. */
#[test]
fn projection_exposes_only_nonsecret_input_and_textarea_values() {
    let document = projection(
        "<input id=public value='Rust 🐙'><input id=password type=password value=secret><input id=hidden type=hidden value=private><textarea id=area>notes</textarea><p id=out>text</p>",
    );
    assert_eq!(document.nodes[0].value.as_deref(), Some("Rust 🐙"));
    assert_eq!(document.nodes[1].value, None);
    assert_eq!(document.nodes[2].value.as_deref(), Some("private"));
    assert_eq!(document.nodes[3].value.as_deref(), Some("notes"));
    assert_eq!(document.nodes[4].value, None);
}

/** Listener support never converts native submit, command or inaccessible controls into script buttons. */
#[test]
fn only_visible_enabled_explicit_ordinary_buttons_accept_click_listeners() {
    for attributes in [
        "",
        "type=submit",
        "type=reset",
        "type=button disabled",
        "type=button hidden",
        "type=button style='display:none'",
        "type=button style='pointer-events:none'",
        "type=button command=show",
        "type=button commandfor=panel",
        "type=button popovertarget=panel",
        "type=button popovertargetaction=show",
    ] {
        let document = projection(&format!("<button id=button {attributes}>Click</button>"));
        assert!(!document.nodes[0].click_listener, "{attributes}");
    }
    assert!(projection("<button type=BUTTON>Click</button>").nodes[0].click_listener);
}

/** Immediate projection replacement removes descendants without renumbering surviving siblings. */
#[test]
fn projected_text_changes_match_actual_tree_removal_and_stable_ids() {
    let original = parse("<div id=parent><p id=child>nested</p></div><p id=tail>tail</p>").unwrap();
    let mut document = script_document(&original, "page-a").unwrap();
    let change = document.set_text(0, "changed").unwrap();
    assert_eq!(document.get_by_id("child"), None);
    assert_eq!(document.get_by_id("tail"), Some(2));
    assert!(document.set_text(1, "detached").is_err());
    let changed = apply(&original, &[change]).unwrap();
    let expected = script_document(&changed, "page-a").unwrap();
    assert_eq!(
        serde_json::to_value(document).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

/** A source ID colliding with an earlier synthetic native target cannot redirect a write. */
#[test]
fn author_ids_colliding_with_synthetic_native_ids_are_not_text_writable() {
    let mut document = projection("<p>anonymous</p><p id='@0'>author</p>");
    assert_eq!(document.get_by_id("@0"), Some(1));
    assert!(!document.nodes[1].text_writable);
    assert_eq!(
        document.set_text(1, "wrong target").unwrap_err().code,
        "unsupported_feature"
    );
}

/** Hostile tree identities and ancestry fail before any script can traverse the snapshot. */
#[test]
fn malformed_projection_identity_duplicates_parents_and_text_are_rejected() {
    let original = projection("<div id=a><p id=b>nested</p></div>");
    for case in 0..6 {
        let mut document = original.clone();
        match case {
            0 => document.identity.clear(),
            1 => document.identity = "x".repeat(ID_BYTES + 1),
            2 => document.nodes[1].local_id = document.nodes[0].local_id,
            3 => document.nodes[0].parent = Some(1),
            4 => document.nodes[1].html_id = "x".repeat(mars::constants::HTML_ID_BYTES + 1),
            _ => document.nodes[0].value = Some("x".repeat(TEXT_BYTES + 1)),
        }
        assert!(document.validate().is_err(), "case {case}");
    }
}

/** Descendant deletion before later tables must retain stable header references during paint. */
#[test]
fn table_rendering_after_early_descendant_removal_uses_stable_ids() {
    let original = parse("<div id=early><p id=gone>remove</p></div><table><tr><th id=heading>Header</th></tr><tr><td id=value>Data</td></tr></table>").unwrap();
    let changed = apply(
        &original,
        &[TextChange {
            html_id: "early".into(),
            text: "replacement".into(),
        }],
    )
    .unwrap();
    let cell = changed
        .nodes
        .iter()
        .find(|node| node.html_id == "value")
        .unwrap();
    let header = changed
        .nodes
        .iter()
        .find(|node| node.html_id == "heading")
        .unwrap();
    assert_eq!(
        cell.table_cell.as_ref().unwrap().headers,
        vec![header.local_id]
    );
    assert!(render(&changed, 800.0).is_ok());
}
