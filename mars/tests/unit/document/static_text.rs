/** Static reading keeps complete source identities while byte, control and measured-work limits stay independent. */
use mars::{
    constants::{STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES, TEXT_BYTES},
    document::{
        TextChange, apply, compute_styles, label, parse, render, script_document, set_value,
        validate_content,
    },
};

/// The diagnostic a page records when text past a per-node bound is cut.
const TRUNCATED: &str = "Text longer than the per-node bound was truncated.";
/// Full-size paragraphs that exactly fill the aggregate stored-text allowance.
const PARAGRAPHS: usize = STATIC_TEXT_TOTAL_BYTES / STATIC_TEXT_NODE_BYTES;
const _: () = assert!(PARAGRAPHS * STATIC_TEXT_NODE_BYTES == STATIC_TEXT_TOTAL_BYTES);

/** A long readable paragraph remains one element with its complete original UTF-8 contents. */
#[test]
fn long_static_paragraph_keeps_complete_text_and_one_identity() {
    let sentence = "Readable beginning café 🐙 middle and final sentence. ";
    let text = sentence.repeat(TEXT_BYTES / sentence.len() + 1);
    assert!(text.len() > TEXT_BYTES);
    let document = parse(&format!("<p id=paragraph>{text}</p>")).unwrap();
    assert_eq!(document.nodes.len(), 1);
    assert_eq!(document.nodes[0].local_id, 0);
    assert_eq!(document.nodes[0].text, text);
    let projection = script_document(&document, "static-page").unwrap();
    assert!(projection.nodes[0].value.is_none());
}

/** Static UTF-8 bytes have an exact per-node cap; the next byte is cut at a character boundary. */
#[test]
fn static_node_limit_keeps_exact_utf8_bytes_and_truncates_past_it() {
    let text = "é".repeat(STATIC_TEXT_NODE_BYTES / "é".len());
    assert_eq!(
        parse(&format!("<p>{text}</p>")).unwrap().nodes[0].text,
        text
    );
    assert_truncated(&format!("<p>{text}x</p>"), &text);
    let shifted = format!("x{}", "é".repeat(STATIC_TEXT_NODE_BYTES / "é".len() - 1));
    assert_truncated(&format!("<p>x{text}</p>"), &shifted);
}

/** One paragraph keeps exactly `expected` and the page records the truncation. */
fn assert_truncated(source: &str, expected: &str) {
    let document = parse(source).unwrap();
    assert_eq!(document.nodes.len(), 1);
    assert_eq!(document.nodes[0].text, expected);
    assert!(document.diagnostics.contains(&TRUNCATED.to_owned()));
}

/** Many individually valid paragraphs cannot evade the total stored-text byte budget. */
#[test]
fn aggregate_static_limit_counts_each_stored_node_once() {
    let full = format!("<p>{}</p>", "x".repeat(STATIC_TEXT_NODE_BYTES)).repeat(PARAGRAPHS);
    let document = parse(&full).unwrap();
    assert_eq!(
        document
            .nodes
            .iter()
            .map(|node| node.text.len())
            .sum::<usize>(),
        STATIC_TEXT_TOTAL_BYTES
    );
    assert_eq!(
        parse(&format!("{full}<p>x</p>")).unwrap_err().code,
        "limit_exceeded"
    );
}

/** A comment inside a paragraph splits its text as the DOM does: the element's own text, a
`#comment` node and a `#text` node. Each text node keeps the per-node bound, so a comment cannot
stretch one node past it, and the cut is recorded. */
#[test]
fn comments_split_text_into_separately_bounded_nodes() {
    let full = "x".repeat(STATIC_TEXT_NODE_BYTES);
    let document = parse(&format!("<p>{full}<!-- split -->{full}</p>")).unwrap();
    let tags: Vec<&str> = document
        .nodes
        .iter()
        .map(|node| node.tag.as_str())
        .collect();
    assert_eq!(tags, ["p", "#comment", "#text"]);
    assert_eq!(document.nodes[0].text, full);
    assert_eq!(document.nodes[1].value, " split ");
    assert_eq!(document.nodes[2].text, full);
    assert!(!document.diagnostics.contains(&TRUNCATED.to_owned()));
    let over = parse(&format!("<p>{full}<!-- split -->{full}x</p>")).unwrap();
    assert_eq!(over.nodes[2].text, full);
    assert!(over.diagnostics.contains(&TRUNCATED.to_owned()));
}

/** Numeric references can expand UTF-8 bytes and still stop at the decoded node allowance. */
#[test]
fn expanded_static_entities_obey_decoded_bytes() {
    let entities = "&#x1f419;".repeat(STATIC_TEXT_NODE_BYTES / "🐙".len());
    let decoded = "🐙".repeat(STATIC_TEXT_NODE_BYTES / "🐙".len());
    let document = parse(&format!("<p>{entities}</p>")).unwrap();
    assert_eq!(document.nodes[0].text, decoded);
    assert_truncated(&format!("<p>{entities}x</p>"), &decoded);
}

/** The larger static allowance never reaches titles, attributes, control data or control names. */
#[test]
fn titles_attributes_and_native_control_data_keep_the_editable_limit() {
    let large = "x".repeat(TEXT_BYTES + 1);
    let title = parse(&format!("<title>{large}</title><p>body</p>")).unwrap();
    assert_eq!(title.title.len(), TEXT_BYTES);
    assert!(title.diagnostics.contains(&TRUNCATED.to_owned()));
    for source in [
        format!("<p aria-label='{large}'>body</p>"),
        format!("<input value='{large}'>"),
        format!("<input type=password value='{large}'>"),
    ] {
        let document = parse(&source).unwrap();
        let node = &document.nodes[0];
        assert!(node.value.is_empty() && !node.attributes.contains_key("aria-label"));
        assert!(!node.attributes.contains_key("value"), "{source}");
    }
    let textarea = parse(&format!("<textarea>{large}</textarea>")).unwrap();
    assert_eq!(textarea.nodes[0].value.len(), TEXT_BYTES);
    // A span inside a button is reading text with the static allowance; the button's own name
    // still never exceeds the editable bound.
    for (source, control, name) in [
        (format!("<button>{large}</button>"), 0, TEXT_BYTES),
        (
            format!("<select><option>{large}</option></select>"),
            1,
            TEXT_BYTES,
        ),
        (format!("<button><span>{large}</span></button>"), 0, 0),
        (
            format!("<label for=entry>{large}</label><input id=entry>"),
            1,
            TEXT_BYTES,
        ),
    ] {
        let document = parse(&source).unwrap();
        let node = &document.nodes[control];
        assert!(node.text.len() <= TEXT_BYTES, "{source}");
        assert_eq!(label(&document, node).len(), name, "{source}");
        assert_eq!(document.labels()[control].len(), name, "{source}");
    }
}

/** Replacing already-long static content never raises the independent script/native-write allowance. */
#[test]
fn static_capacity_does_not_expand_text_or_value_mutations() {
    let text = "x".repeat(TEXT_BYTES + 1);
    let document = parse(&format!("<p id=long>{text}</p><input id=entry>")).unwrap();
    assert_eq!(
        apply(
            &document,
            &[TextChange {
                html_id: "long".into(),
                text: text.clone()
            }]
        )
        .unwrap_err()
        .code,
        "limit_exceeded"
    );
    assert_eq!(
        set_value(&document, 1, &text).unwrap_err().code,
        "limit_exceeded"
    );
    let mut projection = script_document(&document, "static-page").unwrap();
    assert_eq!(
        projection.set_text(0, &text).unwrap_err().code,
        "limit_exceeded"
    );
    assert_eq!(document.nodes[0].text, text);
}

/** Forged documents cannot bypass semantic validation by skipping source parsing. */
#[test]
fn forged_static_overflow_is_refused_before_style_render_or_projection() {
    let mut document = parse("<p id=paragraph>valid</p>").unwrap();
    document.nodes[0].text = "x".repeat(STATIC_TEXT_NODE_BYTES + 1);
    assert!(validate_content(&document).is_err());
    assert!(compute_styles(&document, &[]).is_err());
    assert!(render(&document, 640.0).is_err());
    assert!(script_document(&document, "static-page").is_err());
    assert!(apply(&document, &[]).is_err());
}

/** A malicious parent cycle is rejected before descendant-derived native label traversal starts. */
#[test]
fn forged_cycles_fail_before_native_label_recursion() {
    let mut document = parse("<button><span>caption</span></button>").unwrap();
    document.nodes[0].parent = Some(1);
    assert_eq!(
        validate_content(&document).unwrap_err().code,
        "invalid_request"
    );
}

/** UTF-8 and combining scalars remain complete across narrow long-word wrapping without fake nodes. */
#[test]
fn narrow_long_word_wrapping_preserves_unicode_and_source_identity() {
    let word = "e\u{301}🐙".repeat(320);
    let document = compute_styles(&parse(&format!("<p>{word}</p>")).unwrap(), &[]).unwrap();
    let display = render(&document, 280.0).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == 0)
        .unwrap();
    assert!(item.lines.len() > 1);
    assert_eq!(
        item.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<String>(),
        word
    );
    assert_eq!(document.nodes.len(), 1);
}

/** Exhausted native shaping work is an explicit error rather than partial display or dropped text. */
#[test]
fn excessive_measured_prefix_work_fails_without_changing_the_document() {
    let word = "W".repeat(STATIC_TEXT_NODE_BYTES);
    let column = "<div style='display:flex;flex-direction:column'>".repeat(3);
    let paragraphs = format!("<p>{word}</p>").repeat(PARAGRAPHS);
    let source = format!("{column}{paragraphs}{}", "</div>".repeat(3));
    let document = compute_styles(&parse(&source).unwrap(), &[]).unwrap();
    let error = render(&document, 280.0).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    // At the raised node size the measured extent can run out before the work budget does.
    assert!(
        error.message.contains("layout work") || error.message.contains("layout extent"),
        "{error:?}"
    );
    assert!(
        document
            .nodes
            .iter()
            .filter(|node| node.tag == "p")
            .all(|node| node.text == word)
    );
}

/** A deserialized candidate must obey the same aggregate budget as independently parsed paragraphs. */
#[test]
fn forged_aggregate_overflow_cannot_enter_public_document_operations() {
    let mut document = parse(&"<p></p>".repeat(PARAGRAPHS + 1)).unwrap();
    for node in &mut document.nodes {
        node.text = "x".repeat(STATIC_TEXT_NODE_BYTES);
    }
    assert!(validate_content(&document).is_err());
    assert!(compute_styles(&document, &[]).is_err());
    assert!(render(&document, 640.0).is_err());
    assert!(script_document(&document, "static-page").is_err());
    assert!(apply(&document, &[]).is_err());
}
