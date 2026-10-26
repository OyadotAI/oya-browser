/**
 * `::before` and `::after` of the root and body. Those elements are formatting wrappers with no
 * node of their own, so their generated boxes become top-level stand-in nodes of a document copy
 * (as `canvases` adds bitmap sizes): the root's `::before` boxes first, then the body's, and the
 * `::after` boxes last. The body's lay out in the body's flow; the root's are marked so layout
 * places them in the root's box, above and below the body (`flow::layout_once`).
 */
use crate::document::{Document, Node, style::Generated};

/// The identifier prefix that marks a root generated box's stand-in.
const ROOT_ID: &str = "@root";

/**
 * A copy of `document` with a stand-in node per root or body generated box, each with fresh
 * identities past the document's; None when neither generates any.
 */
pub(super) fn with_root_generated(document: &Document) -> Option<Document> {
    let formatting = &document.formatting;
    let (root, body) = (&formatting.html.style, &formatting.body.style);
    if root.generated.is_empty() && body.generated.is_empty() {
        return None;
    }
    let side = |after: bool| {
        let pick = move |generated: &&Generated| generated.after == after;
        let root = root
            .generated
            .iter()
            .filter(pick)
            .map(|g| (g.clone(), true));
        let body = body
            .generated
            .iter()
            .filter(pick)
            .map(|g| (g.clone(), false));
        if after {
            body.chain(root).collect::<Vec<_>>()
        } else {
            root.chain(body).collect()
        }
    };
    let mut next = document
        .nodes
        .iter()
        .map(|node| node.local_id + 1)
        .max()
        .unwrap_or(0);
    let mut stand_in = |(generated, root): (Generated, bool)| {
        next += 1;
        node(next - 1, generated, root)
    };
    let before: Vec<Node> = side(false).into_iter().filter_map(&mut stand_in).collect();
    let after: Vec<Node> = side(true).into_iter().filter_map(&mut stand_in).collect();
    let mut copy = document.clone();
    copy.nodes = before
        .into_iter()
        .chain(document.nodes.iter().cloned())
        .chain(after)
        .collect();
    Some(copy)
}

/** Whether `node` stands in for a root `::before` or `::after` box. */
pub(super) fn is_root_box(node: &Node) -> bool {
    node.parent.is_none() && node.tag.starts_with("::") && node.html_id.starts_with(ROOT_ID)
}

/** A top-level node carrying one generated box's text and style; its `::` tag marks it
 * generated, so it has no children. Absolutely positioned boxes are left out, as for elements'
 * generated boxes laid out in flow. */
fn node(local_id: usize, generated: Generated, root: bool) -> Option<Node> {
    use crate::document::Position;
    if matches!(
        generated.style.position,
        Position::Absolute | Position::Fixed
    ) {
        return None;
    }
    let tag = if generated.after {
        "::after"
    } else {
        "::before"
    };
    let id = if root {
        format!("{ROOT_ID}{local_id}")
    } else {
        format!("@{local_id}")
    };
    let mut node: Node = serde_json::from_value(serde_json::json!({
        "local_id": local_id, "html_id": id, "tag": tag, "text": generated.content,
    }))
    .ok()?;
    node.style = *generated.style;
    node.style.generated.clear();
    Some(node)
}
