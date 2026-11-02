/** Incremental layout replay preserves the full journal's ordering, identity and refusal rules. */
use mars_eye::{
    DomChange, InsertPosition, MutationCursor, ScriptChange, apply_script_changes, parse,
    script_document,
};

/** Empty elements have one identity, making retirement and reuse unambiguous. */
fn insert(id: usize) -> ScriptChange {
    ScriptChange::Dom {
        change: DomChange::InsertHtml {
            target: None,
            position: InsertPosition::BeforeEnd,
            html: "<div></div>".into(),
            first_id: id,
        },
    }
}

/** Splitting a journal at every possible layout boundary produces the same document. */
#[test]
fn split_replay_matches_full_replay() {
    let base = parse("<p id=original>start</p>").unwrap();
    let changes = vec![
        insert(10),
        ScriptChange::Attribute {
            node: 10,
            name: "id".into(),
            value: Some("added".into()),
        },
        ScriptChange::NodeText {
            node: 10,
            text: "content".into(),
        },
        insert(11),
        ScriptChange::Remove { node: 11 },
        insert(12),
    ];
    let expected = apply_script_changes(&base, &changes).unwrap();
    for split in 0..=changes.len() {
        let mut cursor = MutationCursor::new(&base);
        let first = cursor.apply(base.clone(), &[], &changes[..split]).unwrap();
        let actual = cursor.apply(first, &[], &changes[split..]).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
    }
}

/** A node removed before an intermediate layout cannot have its identity reused afterward. */
#[test]
fn removed_identities_stay_retired_across_layout_boundaries() {
    let base = parse("<p>base</p>").unwrap();
    let mut cursor = MutationCursor::new(&base);
    let first = cursor
        .apply(base, &[], &[insert(10), ScriptChange::Remove { node: 10 }])
        .unwrap();
    assert!(cursor.apply(first.clone(), &[], &[insert(10)]).is_err());
    let next = cursor.apply(first, &[], &[insert(11)]).unwrap();
    assert!(
        script_document(&next, "page")
            .unwrap()
            .nodes
            .iter()
            .any(|node| node.local_id == 11)
    );
}

/** Limits apply to the complete task journal, rather than resetting on each layout read. */
#[test]
fn split_replay_keeps_the_whole_journal_limit() {
    let base = parse("<p>base</p>").unwrap();
    let change = ScriptChange::Attribute {
        node: 0,
        name: "class".into(),
        value: Some("a".into()),
    };
    let mut cursor = MutationCursor::new(&base);
    let full = vec![change.clone(); mars_eye::constants::STATEMENTS];
    let first = cursor.apply(base, &[], &full).unwrap();
    assert_eq!(
        cursor.apply(first, &[], &[change]).unwrap_err().code,
        "limit_exceeded"
    );
}

/** Legacy writes cannot silently run after the extended log when a cache boundary moves. */
#[test]
fn legacy_changes_cannot_follow_extended_changes() {
    let base = parse("<p>base</p>").unwrap();
    let mut cursor = MutationCursor::new(&base);
    let first = cursor.apply(base, &[], &[insert(10)]).unwrap();
    let legacy = DomChange::SetAttribute {
        node: 0,
        name: "lang".into(),
        value: "en".into(),
    };
    assert!(cursor.apply(first, &[legacy], &[]).is_err());
}

/** An intermediate read does not bypass the attribute authority boundary. */
#[test]
fn split_replay_rejects_forbidden_attributes() {
    let base = parse("<iframe></iframe>").unwrap();
    let mut cursor = MutationCursor::new(&base);
    let first = cursor.apply(base, &[], &[]).unwrap();
    let forbidden = ScriptChange::Attribute {
        node: 0,
        name: "srcdoc".into(),
        value: Some("unsafe".into()),
    };
    assert!(cursor.apply(first, &[], &[forbidden]).is_err());
}
