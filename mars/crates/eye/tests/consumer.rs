/** Standalone Eye consumers parse, style and render without Mars or sibling engines. */
use mars_eye::{apply, compute_styles, parse, render, script_document};

/** Real document text reaches a bounded display list through only Eye's public facade. */
#[test]
fn parses_and_renders_html_without_the_browser_shell() {
    let document = parse("<html><head><title>Eye</title></head><body><h1 id='title'>Hello Eye</h1><p>Readable text.</p></body></html>").unwrap();
    let document = compute_styles(&document, &[]).unwrap();
    let display = render(&document, 640.0).unwrap();
    assert_eq!(document.title, "Eye");
    assert_eq!(display.unit, "logical_points");
    assert_eq!(display.width, 640.0);
    assert!(
        display
            .items
            .iter()
            .any(|item| item.lines.iter().any(|line| line.text == "Hello Eye"))
    );
    assert!(display.items.iter().all(|item| item.x.is_finite()
        && item.y.is_finite()
        && item.width.is_finite()
        && item.height.is_finite()));
}

/** Unsupported CSS is dropped with a value-free diagnostic instead of refusing the page. */
#[test]
fn drops_unsupported_css_with_a_value_free_diagnostic() {
    let document = parse("<p style='offset-path:none;color:red'>Secret-free text</p>").unwrap();
    let document = compute_styles(&document, &[]).unwrap();
    assert_eq!(document.nodes[0].style.color.as_deref(), Some("red"));
    assert!(
        document
            .diagnostics
            .iter()
            .any(|d| d.contains("offset-path"))
    );
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("Secret-free"))
    );
}

/** Eye reexports the exact shared projection type without conversion or duplicate validators. */
#[test]
fn shares_projection_identity_with_core_and_applies_its_proposals() {
    let document = parse("<p id='out'>Before<span id='child'> child</span></p><input id='secret' type='password' value='hidden'>").unwrap();
    let mut projection: mars_core::dom::ScriptDocument =
        script_document(&document, "consumer-page").unwrap();
    projection.validate().unwrap();
    let secret = projection.get_by_id("secret").unwrap();
    assert!(
        projection
            .nodes
            .iter()
            .find(|node| node.local_id == secret)
            .unwrap()
            .value
            .is_none()
    );
    let target = projection.get_by_id("out").unwrap();
    let change: mars_core::dom::TextChange = projection.set_text(target, "After").unwrap();
    let changed = apply(&document, &[change]).unwrap();
    assert_eq!(
        changed
            .nodes
            .iter()
            .find(|node| node.html_id == "out")
            .unwrap()
            .text,
        "After"
    );
    assert!(changed.nodes.iter().all(|node| node.html_id != "child"));
    assert!(document.nodes.iter().any(|node| node.html_id == "child"));
}

/** The standalone renderer still validates caller-supplied viewport bounds. */
#[test]
fn rejects_invalid_viewports_before_layout() {
    let document = parse("<p>Hello Eye</p>").unwrap();
    assert_eq!(
        render(&document, f64::NAN).unwrap_err().code,
        "invalid_request"
    );
}
