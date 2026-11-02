/** Keyword sizing belongs to one layout and must survive nested orthogonal measurements. */
use mars_eye::{DisplayList, Document, compute_styles, parse, render};

/** Public layout exercises both the keyword prepass and final placement without a browser. */
fn layout(markup: &str) -> (Document, DisplayList) {
    let document = compute_styles(&parse(markup).unwrap(), &["body{margin:0}".into()]).unwrap();
    let display = render(&document, 800.0).unwrap();
    (document, display)
}

/** Find the actual border-box width, not a private cached intermediate. */
fn width(document: &Document, display: &DisplayList, id: &str) -> f64 {
    let node = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap();
    display
        .items
        .iter()
        .find(|item| item.local_id == node.local_id)
        .unwrap()
        .width
}

/** Identical identities across successive documents must never reuse earlier intrinsic constraints. */
#[test]
fn previous_document_keyword_widths_do_not_constrain_the_next_prepass() {
    let markup = |size| {
        format!(
            "<div id='outer' style='display:flex;width:max-content'><div style='flex:0 0 10px;max-width:max-content'><i style='display:block;width:{size}px;height:10px'></i></div></div>"
        )
    };
    let _ = layout(&markup(1));
    let (document, display) = layout(&markup(100));
    assert_eq!(width(&document, &display, "outer"), 100.0);
    let _ = layout(&markup(300));
    let (document, display) = layout(&markup(100));
    assert_eq!(width(&document, &display, "outer"), 100.0);
}

/** A parent must see a completed child's max-content constraint during its own intrinsic measurement. */
#[test]
fn parent_prepass_uses_current_descendant_keyword_maximums() {
    let (document, display) = layout(
        "<div id='outer' style='display:flex;width:max-content'><div style='flex:0 0 200px;max-width:max-content'><i style='display:block;width:100px;height:10px'></i></div></div>",
    );
    assert_eq!(width(&document, &display, "outer"), 100.0);
}

/** Orthogonal layout uses a temporary document; returning to the parent restores its keyword lookup. */
#[test]
fn vertical_subtree_does_not_erase_later_sibling_keyword_minimums() {
    let (document, display) = layout(
        "<div id='before' style='width:1px;min-width:max-content'><i style='display:block;width:100px;height:10px'></i></div><div style='writing-mode:vertical-rl;height:100px;font:20px/1 monospace'>ab cd</div><div id='after' style='width:1px;min-width:max-content'><i style='display:block;width:100px;height:10px'></i></div>",
    );
    assert_eq!(width(&document, &display, "before"), 100.0);
    assert_eq!(width(&document, &display, "after"), 100.0);
}
