/** Nested sizing probes consume one shared budget instead of multiplying recursive layout work. */
use mars::{
    constants::{STATIC_TEXT_NODE_BYTES, STATIC_TEXT_TOTAL_BYTES, TEXT_BYTES},
    document::{compute_styles, parse, render},
};

/** Repeated measurements use real bounded text and nested auto-height columns. */
fn nested_columns(levels: usize, paragraphs: usize, length: usize) -> String {
    let mut html = "<div style='display:flex;flex-direction:column'>".repeat(levels);
    let word = "a".repeat(length);
    html.push_str(&format!("<p>{word}</p>").repeat(paragraphs));
    html.push_str(&"</div>".repeat(levels));
    html
}

/**
 * Deep valid trees run out of the shared work budget before unbounded repeated native shaping;
 * the page then renders truncated, without the leaves the budget never reached, instead of failing.
 */
#[test]
fn nested_flex_measurement_and_final_passes_share_the_same_work_budget() {
    let paragraphs = STATIC_TEXT_TOTAL_BYTES / STATIC_TEXT_NODE_BYTES;
    let document = compute_styles(
        &parse(&nested_columns(30, paragraphs, STATIC_TEXT_NODE_BYTES)).unwrap(),
        &[],
    )
    .unwrap();
    let display = render(&document, 640.0).unwrap();
    assert!(display.truncated);
    assert!(display.items.iter().filter(|item| item.tag == "p").count() < paragraphs);
}

/** A smaller equivalent workload still renders and retains both leaf elements. */
#[test]
fn bounded_nested_flex_measurement_keeps_real_leaf_content() {
    let document = compute_styles(&parse(&nested_columns(3, 2, TEXT_BYTES)).unwrap(), &[]).unwrap();
    let display = render(&document, 640.0).unwrap();
    assert_eq!(
        display.items.iter().filter(|item| item.tag == "p").count(),
        2
    );
    assert!(display.height > 0.0);
}
