/** Flex geometry is observed through the same public display list used by controls and agents. */
use mars::document::{DisplayItem, DisplayList, Document, compute_styles, parse, render};

/** Empty fixed-size blocks make allocation expectations independent of platform font metrics. */
fn styled(body: &str, css: &str) -> Document {
    compute_styles(&parse(body).unwrap(), &[css.into()]).unwrap()
}

/** Tests identify the real display item through its stable document-local node identity. */
fn item<'a>(document: &Document, display: &'a DisplayList, id: &str) -> &'a DisplayItem {
    let local = document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
        .local_id;
    display
        .items
        .iter()
        .find(|item| item.local_id == local)
        .unwrap()
}

/** Binary64 rounding must not hide a materially different layout decision. */
fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}

#[path = "flex_geometry/allocation.rs"]
mod allocation;
#[path = "flex_geometry/flow.rs"]
mod flow;
#[path = "flex_geometry/validation.rs"]
mod validation;
