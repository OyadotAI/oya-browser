/** Document snapshots share computed values while edits and decoding retain independent ownership. */
use mars_eye::{ComputedStyle, Document, SharedStyle, compute_styles, parse};

/** A candidate's native style edits cannot change the published document or another node. */
#[test]
fn editing_a_snapshot_detaches_only_the_changed_style() {
    let document = compute_styles(&parse("<p>one</p><p>two</p>").unwrap(), &[]).unwrap();
    let mut candidate = document.clone();
    assert!(std::ptr::eq(
        &*document.nodes[0].style,
        &*candidate.nodes[0].style
    ));
    candidate.nodes[0].style.color = Some("red".into());
    candidate.nodes[0].style.margin[0] = 40.0;
    assert_ne!(document.nodes[0].style, candidate.nodes[0].style);
    assert_ne!(document.nodes[0].style.margin[0], 40.0);
    assert!(std::ptr::eq(
        &*document.nodes[1].style,
        &*candidate.nodes[1].style
    ));
}

/** Reusing a cascade retains its style allocation and still protects that cache from later edits. */
#[test]
fn unchanged_restyle_shares_values_without_sharing_edits() {
    let document = parse("<p>one</p>").unwrap();
    let first = compute_styles(&document, &[]).unwrap();
    let mut second = compute_styles(&document, &[]).unwrap();
    assert!(std::ptr::eq(
        &*first.nodes[0].style,
        &*second.nodes[0].style
    ));
    second.nodes[0].style.color = Some("red".into());
    let third = compute_styles(&document, &[]).unwrap();
    assert_eq!(first.nodes[0].style, third.nodes[0].style);
    assert_ne!(second.nodes[0].style, third.nodes[0].style);
}

/** Sharing is private storage: the style object and document wire forms remain value based. */
#[test]
fn styles_keep_the_existing_wire_representation() {
    let plain = ComputedStyle {
        color: Some("red".into()),
        ..Default::default()
    };
    let shared = SharedStyle::from(plain.clone());
    assert_eq!(
        serde_json::to_value(&shared).unwrap(),
        serde_json::to_value(&plain).unwrap()
    );
    let mut document = parse("<p>one</p><p>two</p>").unwrap();
    document.nodes[0].style = shared;
    let wire = serde_json::to_value(&document).unwrap();
    assert!(wire["nodes"][1].get("style").is_none());
    let decoded: Document = serde_json::from_value(wire).unwrap();
    assert_eq!(document.nodes[0].style, decoded.nodes[0].style);
    assert_eq!(document.nodes[1].style, decoded.nodes[1].style);
}
