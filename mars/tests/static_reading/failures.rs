/** Failed bounded candidates preserve the current context and committed page. */
use super::*;

/** Larger static capacity does not grant an unbounded measurement budget: layout stops at the
work bound, the page commits with a display list marked truncated, every published box is
finite, and the render context stays. */
#[test]
fn exhausted_layout_work_truncates_the_display_and_keeps_the_context() {
    let good = fixture("<p id=out>Committed</p>");
    // Deeply nested flex columns re-measure their text at every level, past the work bound.
    let paragraph = format!("<p>{}</p>", "word ".repeat(3200));
    let column = "<div style='display:flex;flex-direction:column'>".repeat(24);
    let bad = fixture(&format!(
        "{column}{}{}",
        paragraph.repeat(48),
        "</div>".repeat(24)
    ));
    let mut runtime = runtime(&[&good.url, &bad.url]);
    runtime
        .set_render_context(RenderContext {
            width: 640.0,
            height: 400.0,
            appearance: Appearance::Dark,
        })
        .unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":good.url})).unwrap();
    let context = runtime.snapshot().render_context;
    let big = call(&mut runtime, "page.navigate", json!({"url":bad.url})).unwrap();
    assert_ne!(big["page_id"], page["page_id"]);
    let snapshot = runtime.snapshot();
    let display = snapshot.display.as_ref().unwrap();
    assert!(display.truncated);
    assert!(display.height.is_finite());
    assert!(
        display
            .items
            .iter()
            .all(|item| item.y.is_finite() && item.height.is_finite())
    );
    assert_eq!(snapshot.render_context, context);
    assert_eq!(snapshot.inspection, big);
    assert_eq!(call(&mut runtime, "page.inspect", json!({})).unwrap(), big);
}

/** A failing script is skipped and named, and the styled document still commits. */
#[test]
fn synthetic_unsupported_script_keeps_the_previous_page() {
    let good = fixture("<p id=out>Committed</p>");
    let bad = fixture(
        "<style>body{font:16px/1.6 system-ui}</style><p>Candidate</p><script>unsupported();</script>",
    );
    let mut runtime = runtime(&[&good.url, &bad.url]);
    call(&mut runtime, "page.navigate", json!({"url":good.url})).unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":bad.url})).unwrap();
    assert_eq!(page["degraded"], json!(["scripts"]));
    assert!(
        page["diagnostics"]
            .to_string()
            .contains("Script 1 (inline) did not run")
    );
}
