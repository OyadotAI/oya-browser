/** Document rules are tested through parse, apply and layout only. */
use mars::constants::{DOCUMENT_DEPTH, HTML_BYTES, NODES, STATIC_TEXT_NODE_BYTES, TEXT_BYTES};
use mars::document::{TextChange, apply, layout, parse};
mod flex_algorithm;
mod flex_geometry;
mod flex_sizing_regressions;
mod flex_style;
mod script;
mod static_css;
mod static_text;

/** Decoded text, source order and native local IDs are deterministic. */
#[test]
fn entities_and_unicode_have_explicit_layout_units() {
    let document = parse("<html><body><h1 id=\"title\">A&amp;é🐙</h1><p id=\"p\">&lt;&gt;&quot;&apos;</p></body></html>").unwrap();
    assert_eq!(document.nodes[0].text, "A&é🐙");
    let geometry = layout(&document);
    assert_eq!(geometry.unit, "unicode_scalar_cells");
    assert_eq!(geometry.rows[0].width, 4);
    assert_eq!(geometry.rows[1].y, 1);
    assert_eq!(document.nodes[1].local_id, 1);
}

/** Module scripts are kept as scripts: V9 runs them, so they carry no fallback diagnostic. */
#[test]
fn module_scripts_are_kept_without_a_diagnostic() {
    let document = parse("<html><body><script type=module></script></body></html>").unwrap();
    assert_eq!(document.scripts.len(), 1);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
}

/** Unsupported markup degrades to inert content with a named diagnostic, never an incomplete page. */
#[test]
fn unsupported_markup_degrades_with_a_diagnostic() {
    for (body, diagnostic) in [
        (
            "<iframe></iframe>",
            Some("Embedded frame (not supported yet)"),
        ),
        (
            "<input onchange='host()'>",
            Some("Inline event handler attributes were removed and never run."),
        ),
        ("<input type=file>", None),
    ] {
        let document = parse(&format!("<html><body>{body}</body></html>")).unwrap();
        assert!(document.scripts.is_empty(), "{body}");
        assert!(
            document.nodes.iter().all(|node| node.handler.is_none()
                && !node.attributes.keys().any(|key| key.starts_with("on"))),
            "{body}"
        );
        if let Some(diagnostic) = diagnostic {
            assert_eq!(document.diagnostics, [diagnostic], "{body}");
        }
    }
}

/** Inline and external executable scripts preserve source order and data blocks stay inert. */
#[test]
fn classic_scripts_are_collected_in_document_order() {
    let document = parse("<script>let a=1;</script><script type=application/json>{}</script><script src='/a.js'></script><script>let b=2;</script>").unwrap();
    assert_eq!(document.scripts.len(), 3);
    assert_eq!(document.scripts[0].source, "let a=1;");
    assert_eq!(document.scripts[1].src.as_deref(), Some("/a.js"));
    assert_eq!(document.scripts[2].source, "let b=2;");
}

/** Every proposed target is validated before any replacement is applied. */
#[test]
fn invalid_script_target_prevents_every_change() {
    let original = parse("<html><body><p id=\"a\">original</p></body></html>").unwrap();
    let changes = [
        TextChange {
            html_id: "a".into(),
            text: "changed".into(),
        },
        TextChange {
            html_id: "missing".into(),
            text: "wrong".into(),
        },
    ];
    assert_eq!(
        apply(&original, &changes).unwrap_err().code,
        "invalid_target"
    );
    assert_eq!(original.nodes[0].text, "original");
}

/** Source bytes and decoded/replaced text each enforce their own budget. */
#[test]
fn document_and_text_bounds_are_enforced() {
    assert_eq!(
        parse(&" ".repeat(HTML_BYTES + 1)).unwrap_err().code,
        "limit_exceeded"
    );
    let truncated = parse(&format!(
        "<html><body><p id=\"a\">{}</p></body></html>",
        "x".repeat(STATIC_TEXT_NODE_BYTES + 1)
    ))
    .unwrap();
    assert_eq!(truncated.nodes[0].text.len(), STATIC_TEXT_NODE_BYTES);
    assert_eq!(
        truncated.diagnostics,
        ["Text longer than the per-node bound was truncated."]
    );
    let original = parse("<html><body><p id=\"a\">x</p></body></html>").unwrap();
    assert_eq!(
        apply(
            &original,
            &[TextChange {
                html_id: "a".into(),
                text: "x".repeat(TEXT_BYTES + 1)
            }]
        )
        .unwrap_err()
        .code,
        "limit_exceeded"
    );
}

/** Many small nodes cannot bypass the independent node-count budget. */
#[test]
fn node_count_has_an_independent_bound() {
    let body: String = (0..=NODES)
        .map(|id| format!("<p id=\"n{id}\">x</p>"))
        .collect();
    assert_eq!(
        parse(&format!("<html><body>{body}</body></html>"))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}

/** A valid batch preserves identity while updating geometry. */
#[test]
fn native_mutation_changes_layout_without_reassigning_identity() {
    let original = parse("<html><body><p id=\"a\">x</p></body></html>").unwrap();
    let changed = apply(
        &original,
        &[TextChange {
            html_id: "a".into(),
            text: "longer".into(),
        }],
    )
    .unwrap();
    assert_eq!(layout(&changed).rows[0].width, "longer".len());
    assert_eq!(changed.nodes[0].local_id, original.nodes[0].local_id);
}
/** Optional author IDs and bounded nested containers keep deterministic native identities. */
#[test]
fn nested_documents_generate_stable_native_local_ids() {
    let document=parse("<!doctype html><html><head><title>Own &amp; native</title></head><body><div><h1>Hello</h1><p id=\"body\">Text <span>inside</span></p><input placeholder=\"Name\" disabled></div></body></html>").unwrap();
    assert_eq!(document.title, "Own & native");
    assert_eq!(document.nodes[0].html_id, "@0");
    assert_eq!(document.nodes[1].parent, Some(0));
    let input = document.nodes.last().unwrap();
    assert!(input.disabled);
    assert!(mars::document::set_value(&document, input.local_id, "changed").is_err());
    let deep = format!(
        "<html><body>{}x{}</body></html>",
        "<div>".repeat(33),
        "</div>".repeat(33)
    );
    let flattened = parse(&deep).unwrap();
    assert_eq!(
        flattened.diagnostics,
        ["Markup nested past the depth bound was flattened."]
    );
    assert!(flattened.nodes.iter().enumerate().all(|(index, node)| {
        let depth = std::iter::successors(node.parent, |id| flattened.nodes[*id].parent).count();
        node.local_id == index && depth < DOCUMENT_DEPTH
    }));
    assert!(flattened.nodes.iter().any(|node| node.text == "x"));
    let again = parse(&deep).unwrap();
    assert!(
        flattened
            .nodes
            .iter()
            .zip(&again.nodes)
            .all(|(a, b)| a.local_id == b.local_id && a.parent == b.parent)
    );
}

/** textContent removes descendants atomically while leaving surviving identities unchanged. */
#[test]
fn text_replacement_cannot_leave_descendant_controls_in_the_document() {
    let original=parse("<html><body><div id=\"parent\"><input id=\"child\"><p>Nested</p></div><p id=\"sibling\">Keep</p></body></html>").unwrap();
    let changed = apply(
        &original,
        &[TextChange {
            html_id: "parent".into(),
            text: "replacement".into(),
        }],
    )
    .unwrap();
    assert_eq!(changed.nodes.len(), 2);
    assert_eq!(changed.nodes[0].text, "replacement");
    assert_eq!(
        changed.nodes[1].local_id,
        original.nodes.last().unwrap().local_id
    );
    let failed = apply(
        &original,
        &[
            TextChange {
                html_id: "parent".into(),
                text: "replacement".into(),
            },
            TextChange {
                html_id: "child".into(),
                text: "gone".into(),
            },
        ],
    );
    assert!(failed.is_err());
    assert_eq!(original.nodes.len(), 4);
}
