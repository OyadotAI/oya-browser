/** CSS Overflow 5 scroll markers: a scroll container with `scroll-marker-group: before | after`
gets a `::scroll-marker-group` box as its previous or next sibling, holding one `::scroll-marker`
box for each element with marker content whose scroll container it is. Eye builds no pseudo
boxes, so layout runs on a copy of the document with these boxes as nodes; their items take
their originating element's identity back once painting is resolved (`reclaim`).
ponytail: `:target-current`, scroll buttons and marker activation are not implemented. */
use super::flow::{DisplayItem, Node};
use crate::document::{Document, Overflow, Position};
use std::collections::BTreeMap;

/** The tag of a synthetic group box, and of the items it paints. */
const GROUP_TAG: &str = "::scroll-marker-group";
/** The tag of a synthetic marker box. */
const MARKER_TAG: &str = "::scroll-marker";
/** The tag of a synthetic scroll button box. */
const BUTTON_TAG: &str = "::scroll-button";
/** The tag synthetic boxes lay out under: a plain element, since layout treats `::` tags as
generated boxes with no children of their own. */
const LAYOUT_TAG: &str = "div";

/** Synthetic identity to its originating element and pseudo tag. */
type Owners = BTreeMap<usize, (usize, &'static str)>;

/** The document with marker groups as nodes, and each synthetic identity's originating element. */
pub(super) struct Marked {
    /// The copy layout and painting run on.
    pub document: Document,
    /// Synthetic identity to the element it stands for (the scroller for a group, the
    /// originating element for a marker) and its pseudo tag.
    pub owners: Owners,
}

/** Whether a style makes its box a scroll container. */
fn scrolls(style: &crate::document::ComputedStyle) -> bool {
    !matches!(style.overflow_x, Overflow::Visible | Overflow::Clip)
        || !matches!(style.overflow_y, Overflow::Visible | Overflow::Clip)
}

/** The scroll container a marker belongs to: the nearest scrolling ancestor along the
containing-block chain, so an absolute box skips non-positioned ancestors and a fixed one
reaches the viewport (scroll-marker-005). None when there is none. */
fn marker_scroller(document: &Document, positions: &[Option<usize>], node: &Node) -> Option<usize> {
    let mut escaping = node.style.position;
    let mut current = node.parent;
    while let Some(local) = current {
        let ancestor = &document.nodes[positions.get(local).copied().flatten()?];
        let style = &ancestor.style;
        let contains = match escaping {
            Position::Absolute => {
                style.position != Position::Static || super::positioned::contains_fixed(style)
            }
            Position::Fixed => super::positioned::contains_fixed(style),
            _ => true,
        };
        if contains {
            if scrolls(style) {
                return Some(local);
            }
            escaping = style.position;
        }
        current = ancestor.parent;
    }
    None
}

/** A synthetic box node standing for a pseudo-element of `owner`. */
fn synthetic(owner: &Node, local_id: usize, parent: Option<usize>) -> Node {
    let mut node = owner.clone();
    node.tag = LAYOUT_TAG.into();
    node.local_id = local_id;
    node.parent = parent;
    node.attributes = Default::default();
    node.href = None;
    node.image_src = None;
    node.image_size = None;
    node.table_cell = None;
    node.text = String::new();
    // Pseudo boxes have no source white space around them.
    node.space_after = false;
    node.space_leading = false;
    node.leading_space = String::new();
    node
}

/** The group and marker nodes of one scroller, numbered from `next`. */
fn group_nodes(
    scroller: &Node,
    markers: &[&Node],
    next: &mut usize,
    owners: &mut Owners,
) -> Vec<Node> {
    let Some(style) = scroller.style.marker_group.as_deref() else {
        return Vec::new();
    };
    let group_id = *next;
    *next += 1;
    owners.insert(group_id, (scroller.local_id, GROUP_TAG));
    let mut group = synthetic(scroller, group_id, scroller.parent);
    group.style = style.clone().into();
    let mut nodes = vec![group];
    for owner in markers {
        let Some(marker) = owner.style.scroll_marker.as_deref() else {
            continue;
        };
        let mut node = synthetic(owner, *next, Some(group_id));
        node.text = marker.content.clone();
        node.style = (*marker.style).clone().into();
        owners.insert(*next, (owner.local_id, MARKER_TAG));
        *next += 1;
        nodes.push(node);
    }
    nodes
}

/** The `::scroll-button()` nodes of `owner`, numbered from `next`: its next siblings. */
fn button_nodes(owner: &Node, next: &mut usize, owners: &mut Owners) -> Vec<Node> {
    let mut nodes = Vec::new();
    for button in &owner.style.scroll_buttons {
        let mut node = synthetic(owner, *next, owner.parent);
        node.text = button.content.clone();
        node.style = (*button.style).clone().into();
        owners.insert(*next, (owner.local_id, BUTTON_TAG));
        *next += 1;
        nodes.push(node);
    }
    nodes
}

/** The index just past `index`'s subtree, nodes being in document order. */
fn subtree_end(document: &Document, index: usize) -> usize {
    let mut inside = std::collections::BTreeSet::from([document.nodes[index].local_id]);
    let mut end = index + 1;
    while let Some(node) = document.nodes.get(end) {
        if !node.parent.is_some_and(|parent| inside.contains(&parent)) {
            break;
        }
        inside.insert(node.local_id);
        end += 1;
    }
    end
}

/** The copy of `document` with every marker group and scroll button placed, or None when no
element asks for one. Buttons follow their element, then an `after` group. */
pub(super) fn expand(document: &Document) -> Option<Marked> {
    let has_group = |node: &Node| node.style.marker_group.is_some() && scrolls(&node.style);
    let generates = |node: &Node| has_group(node) || !node.style.scroll_buttons.is_empty();
    if !document.nodes.iter().any(generates) {
        return None;
    }
    let positions = super::sizing::positions(document);
    let mut markers: BTreeMap<usize, Vec<&Node>> = BTreeMap::new();
    for node in document
        .nodes
        .iter()
        .filter(|node| node.style.scroll_marker.is_some())
    {
        if let Some(scroller) = marker_scroller(document, &positions, node) {
            markers.entry(scroller).or_default().push(node);
        }
    }
    let mut next = document.nodes.iter().map(|node| node.local_id).max()? + 1;
    let mut owners = BTreeMap::new();
    let mut inserts: BTreeMap<usize, Vec<Node>> = BTreeMap::new();
    for (index, scroller) in document
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| generates(n))
    {
        let end = subtree_end(document, index);
        let buttons = button_nodes(scroller, &mut next, &mut owners);
        inserts.entry(end).or_default().extend(buttons);
        if !has_group(scroller) {
            continue;
        }
        let own = markers
            .get(&scroller.local_id)
            .map_or(&[][..], Vec::as_slice);
        let nodes = group_nodes(scroller, own, &mut next, &mut owners);
        let at = if scroller.style.marker_group_side == Some(true) {
            end
        } else {
            index
        };
        inserts.entry(at).or_default().extend(nodes);
    }
    let mut copy = document.clone();
    copy.nodes = Vec::with_capacity(document.nodes.len() + owners.len());
    for (index, node) in document.nodes.iter().enumerate() {
        copy.nodes
            .extend(inserts.remove(&index).unwrap_or_default());
        copy.nodes.push(node.clone());
    }
    copy.nodes.extend(inserts.into_values().flatten());
    Some(Marked {
        document: copy,
        owners,
    })
}

/** Gives every synthetic item its originating element's identity back, keeping its pseudo tag,
so hosts and agents see only real nodes. */
pub(super) fn reclaim(marked: Option<&Marked>, items: &mut [DisplayItem]) {
    let Some(marked) = marked else {
        return;
    };
    for item in items {
        if let Some((owner, tag)) = marked.owners.get(&item.local_id) {
            item.local_id = *owner;
            item.tag = (*tag).into();
        }
    }
}
