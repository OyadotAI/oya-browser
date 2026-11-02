/** Script selection crosses the document bridge without changing authored defaults. */
use mars_eye::{Document, ScriptChange, apply_script_changes, parse};

fn id(document: &Document, name: &str) -> usize {
    document
        .nodes
        .iter()
        .find(|n| n.html_id == name)
        .unwrap()
        .local_id
}

#[test]
fn value_assignment_selects_first_duplicate_inside_optgroup_even_when_disabled() {
    let before = parse("<select id=s><option id=a selected value=a>A</option><optgroup><option id=b disabled value=b>B</option><option id=c value=b>C</option></optgroup></select>").unwrap();
    let after = apply_script_changes(
        &before,
        &[ScriptChange::Value {
            node: id(&before, "s"),
            value: "b".into(),
        }],
    )
    .unwrap();
    let selected: Vec<_> = after
        .nodes
        .iter()
        .filter(|n| n.selected)
        .map(|n| n.html_id.as_str())
        .collect();
    assert_eq!(selected, ["b"]);
    assert!(
        after
            .nodes
            .iter()
            .find(|n| n.html_id == "a")
            .unwrap()
            .attributes
            .contains_key("selected")
    );
    let empty = apply_script_changes(
        &after,
        &[ScriptChange::Value {
            node: id(&before, "s"),
            value: "absent".into(),
        }],
    )
    .unwrap();
    assert!(!empty.nodes.iter().any(|n| n.selected));
}

#[test]
fn foreign_duplicate_and_excess_selection_is_rejected_atomically() {
    let before = parse("<select id=s><option id=a>A</option><option id=b>B</option></select><select><option id=c>C</option></select>").unwrap();
    for selected in [
        vec![id(&before, "c")],
        vec![id(&before, "a"), id(&before, "a")],
        vec![id(&before, "a"), id(&before, "b")],
    ] {
        assert!(
            apply_script_changes(
                &before,
                &[ScriptChange::Selection {
                    node: id(&before, "s"),
                    selected
                }]
            )
            .is_err()
        );
        assert!(
            before
                .nodes
                .iter()
                .find(|n| n.html_id == "a")
                .unwrap()
                .selected
        );
    }
}

/** Exact selection includes disabled options, but successful form entries exclude them. */
#[test]
fn selected_optgroup_values_and_empty_selection_match_submission() {
    let before = parse("<form id=f><select id=s name=choice multiple><optgroup><option id=a value=a>A</option></optgroup><optgroup disabled><option id=b value=b>B</option></optgroup><option id=c>C text</option></select></form>").unwrap();
    let after = apply_script_changes(
        &before,
        &[ScriptChange::Selection {
            node: id(&before, "s"),
            selected: vec![id(&before, "a"), id(&before, "b"), id(&before, "c")],
        }],
    )
    .unwrap();
    assert_eq!(
        mars_eye::submission(&after, id(&after, "f"), None)
            .unwrap()
            .encoded,
        "choice=a&choice=C+text"
    );
    assert!(mars_eye::select(&before, id(&before, "s"), id(&before, "b")).is_err());
    let empty = apply_script_changes(
        &after,
        &[ScriptChange::Selection {
            node: id(&after, "s"),
            selected: vec![],
        }],
    )
    .unwrap();
    assert_eq!(
        mars_eye::submission(&empty, id(&empty, "f"), None)
            .unwrap()
            .encoded,
        ""
    );
}
