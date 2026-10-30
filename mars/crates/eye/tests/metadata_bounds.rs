/** Parser admission and restored-document validation share complete metadata byte accounting. */
use mars_core::constants::{METADATA_BYTES, METADATA_ELEMENTS};
use mars_eye::{Document, MetadataElement, parse, validate_content};

/** Crossing the former count cap retains every small metadata element in source order. */
#[test]
fn small_metadata_records_are_not_cut_at_256() {
    let source = (0..300)
        .map(|index| format!("<meta id=m{index} content={index}>"))
        .collect::<String>();
    let document = parse(&source).unwrap();
    assert_eq!(document.metadata.len(), 300);
    assert_eq!(document.metadata.last().unwrap().attributes["id"], "m299");
    assert!(
        !document
            .diagnostics
            .iter()
            .any(|message| message.contains("Metadata past"))
    );
}

/** A record exceeding the real aggregate cap is omitted, never replaced by empty text. */
#[test]
fn oversized_data_block_is_diagnosed_and_later_small_metadata_survives() {
    let source = format!(
        "<script type=application/json id=big>{}</script><meta id=after>",
        "x".repeat(METADATA_BYTES)
    );
    let document = parse(&source).unwrap();
    assert_eq!(document.metadata.len(), 1);
    assert_eq!(document.metadata[0].attributes["id"], "after");
    assert!(
        document
            .diagnostics
            .iter()
            .any(|message| message.contains("Metadata past"))
    );
}

/** Deserialization cannot bypass count or byte admission with empty-valued long attribute names. */
#[test]
fn restored_metadata_is_validated_before_projection() {
    let encoded = serde_json::to_vec(&parse("<meta id=ok>").unwrap()).unwrap();
    let mut document: Document = serde_json::from_slice(&encoded).unwrap();
    document.metadata[0]
        .attributes
        .insert("x".repeat(METADATA_BYTES), String::new());
    assert!(validate_content(&document).is_err());
    document.metadata = vec![
        MetadataElement {
            tag: "meta".into(),
            ..Default::default()
        };
        METADATA_ELEMENTS + 1
    ];
    assert!(validate_content(&document).is_err());
}

/** Executable-script attributes share the allowance rather than getting a separate unchecked pool. */
#[test]
fn restored_script_attributes_and_metadata_share_bytes() {
    let mut document = parse("<meta><script></script>").unwrap();
    document.metadata[0].text = "x".repeat(METADATA_BYTES / 2);
    document.scripts[0]
        .attributes
        .insert("data-config".into(), "y".repeat(METADATA_BYTES / 2));
    assert!(validate_content(&document).is_err());
}
