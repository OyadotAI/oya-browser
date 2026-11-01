/** Pointer targeting shared by the agent channel and the native window: which painted box a point
lands on, and which element a click there reaches, following Chrome's model. Both paths call these
functions so a click means the same thing whichever way it arrives. */
use super::render::{DisplayItem, DisplayList};
use super::{Document, Node};
use crate::constants::DOCUMENT_DEPTH;
use mars_core::geometry::{contains_quad, contains_rect, quad};

/// Input types that are buttons or choices, not text a person types into.
const NOT_TEXT: &[&str] = &[
    "checkbox", "radio", "submit", "button", "reset", "file", "image", "hidden", "color", "range",
];
/// Elements that can take a click's focus or default action, nearest first.
const CONTROL_TAGS: &[&str] = &["label", "input", "textarea", "a", "button"];

/**
 * The topmost painted box under a document point whose node takes pointer events. Items are in
 * paint order, so the last containing item is on top. `top` is the viewport's scroll offset, which
 * moves pinned boxes. `resolve` maps an item to its node and the base ID of the frame that owns it.
 * Opacity does not decide: a transparent box is still hit, as in Chrome, which is how an invisible
 * field sits over its visible label. A `pointer-events:none` box lets the point fall through.
 */
pub fn pointer_target<'a>(
    display: &DisplayList,
    point: [f64; 2],
    top: f64,
    resolve: impl Fn(&DisplayItem) -> Option<(usize, &'a Node)>,
) -> Option<(usize, &'a Node)> {
    display
        .items
        .iter()
        .rev()
        .filter(|item| item_contains(item, point, top))
        .find_map(|item| resolve(item).filter(|(_, node)| takes_pointer(node)))
}

/**
 * Whether a point lies inside an item's transformed box and inside its clip, as painting draws it.
 * Pinned boxes are tested where they are drawn for this scroll offset, not where layout put them.
 */
pub fn item_contains(item: &DisplayItem, point: [f64; 2], top: f64) -> bool {
    let shift = item.pin.map_or(0.0, |pin| pin.shift(top));
    let point = [point[0], point[1] - shift];
    let rect = [item.x, item.y, item.width, item.height];
    let outline = item.quad.unwrap_or_else(|| quad(rect, item.transform));
    contains_quad(&outline, point)
        && item
            .clip
            .is_none_or(|clip| contains_rect([clip.x, clip.y, clip.width, clip.height], point))
}

/**
 * Whether a node can be a click's target. `pointer-events:none` and `inert` clear the flag through
 * inheritance, `visibility:hidden` and display:none make the node hidden. A disabled control still
 * takes the click, which then does nothing, so a click never falls through to a link beneath it.
 */
pub fn takes_pointer(node: &Node) -> bool {
    node.style.interactive && !node.hidden()
}

/** Whether a node is a field a person types into. */
pub fn is_text_field(node: &Node) -> bool {
    node.tag == "textarea" || node.tag == "input" && !NOT_TEXT.contains(&node.input_type())
}

/**
 * The text field a click on this node focuses rather than activates: the node itself, or the field
 * its label names when the click lands inside a label, as Chrome focuses a label's control. Returns
 * None when the nearest control is a link or a button, which the click activates instead.
 */
pub fn focus_field<'a>(document: &'a Document, hit: &Node) -> Option<&'a Node> {
    let control = nearest_control(document, hit)?;
    let field = match control.tag.as_str() {
        "label" => labeled_control(document, control)?,
        _ => control,
    };
    (is_text_field(field) && usable(field)).then_some(field)
}

/** Whether a field a click resolved to may take focus: a label can name a field that is hidden,
inert, pointer-events:none or disabled, and focusing that one would type into something the
person cannot see or use. */
fn usable(field: &Node) -> bool {
    takes_pointer(field) && !field.attributes.contains_key("disabled")
}

/**
 * The control a label names: the element its `for` attribute points at, else the first field
 * nested inside it. Scripts and the native click path both read labels through this one rule.
 */
pub fn labeled_control<'a>(document: &'a Document, label: &Node) -> Option<&'a Node> {
    let named = label.attributes.get("for");
    document.nodes.iter().find(|candidate| {
        named == Some(&candidate.html_id)
            || candidate.parent == Some(label.local_id)
                && ["input", "select", "textarea"].contains(&candidate.tag.as_str())
    })
}

/** The nearest element from `hit` upward that can take a click's focus or default action. */
fn nearest_control<'a>(document: &'a Document, hit: &Node) -> Option<&'a Node> {
    let mut id = Some(hit.local_id);
    for _ in 0..DOCUMENT_DEPTH {
        let node = document
            .nodes
            .iter()
            .find(|candidate| Some(candidate.local_id) == id)?;
        if CONTROL_TAGS.contains(&node.tag.as_str()) {
            return Some(node);
        }
        id = node.parent;
    }
    None
}
