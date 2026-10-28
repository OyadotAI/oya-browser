/**
 * Range and Selection: boundary points over the page's nodes, as `document.createRange`,
 * `new Range()` and `getSelection()` give them. Boundaries hold node wrappers rather than
 * identities, so a range over a script-created node follows it when insertion renumbers it.
 * Registered ranges also move with the tree (see range_live); StaticRange objects do not.
 * V9 has no layout or caret, so client rectangles are the nominal boxes `getBoundingClientRect`
 * reports and the selection holds whatever ranges scripts put there.
 */
use super::dom::{self, DOCUMENT, insert, node_of, remove_node, wrap};
use super::{components, host_data, range_contents, range_live};
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData, RangeData};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Vm};
use std::cmp::Ordering;

/** `Range.START_TO_START` and the other `compareBoundaryPoints` modes, in specification order. */
pub(super) const COMPARE_MODES: [(&str, f64); 4] = [
    ("START_TO_START", 0.0),
    ("START_TO_END", 1.0),
    ("END_TO_END", 2.0),
    ("END_TO_START", 3.0),
];

/** A boundary point: a node and an offset into its children or text. */
pub(super) type Boundary = (usize, usize);

/** A new range collapsed at the start of the document. */
pub(in crate::v9) fn new_range(vm: &mut Vm) -> Res<JsValue> {
    range_object(vm, collapsed_data())
}

/** Range data collapsed at the start of the document. */
fn collapsed_data() -> RangeData {
    let document = JsValue::Object(realm().ids.document);
    RangeData {
        start: document.clone(),
        start_offset: 0,
        end: document,
        end_offset: 0,
        is_static: false,
    }
}

/** Allocate a Range host object over its data and register it as live. */
fn range_object(vm: &mut Vm, data: RangeData) -> Res<JsValue> {
    let nodes = range_live::nodes_of(vm, &data);
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Range(Box::new(data)))),
        Some(realm().ids.range_prototype),
    ))?;
    range_live::register(vm, id, nodes);
    Ok(JsValue::Object(id))
}

/** `new Range()`. */
pub(in crate::v9) fn construct_range(vm: &mut Vm, _: Call) -> Res<JsValue> {
    new_range(vm)
}

/**
 * `new StaticRange(init)`: a range whose points stay where the script put them, so it is not
 * registered as live and no mutation moves it.
 */
pub(in crate::v9) fn construct_static_range(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let init = call.arg(0);
    if !matches!(init, JsValue::Object(_)) {
        return Err(vm.type_error("Failed to construct 'StaticRange': The init must be an object."));
    }
    let (start, start_offset) = static_point(vm, &init, "startContainer", "startOffset")?;
    let (end, end_offset) = static_point(vm, &init, "endContainer", "endOffset")?;
    let data = RangeData {
        start,
        start_offset,
        end,
        end_offset,
        is_static: true,
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Range(Box::new(data)))),
        Some(realm().ids.static_range_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** One boundary of a StaticRange init: its container must be a node, its offset an integer. */
fn static_point(
    vm: &mut Vm,
    init: &JsValue,
    container: &str,
    offset: &str,
) -> Res<(JsValue, usize)> {
    let node = vm.get(init, &Key::from(container))?;
    if node_of(vm, &node).is_none() {
        return Err(vm.type_error(&format!(
            "Failed to construct 'StaticRange': {container} is not a Node."
        )));
    }
    let value = vm.get(init, &Key::from(offset))?;
    let number = vm.to_integer(&value)?;
    Ok((node, number.max(0.0) as usize))
}

/** The data behind a Range or StaticRange, or an Illegal invocation error. */
pub(super) fn any_range_data(vm: &mut Vm, value: &JsValue) -> Res<RangeData> {
    match host_data(vm, value) {
        Some(HostData::Range(data)) => Ok(*data),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** The data behind a Range, refusing a StaticRange: its methods exist only on Range. */
pub(super) fn range_data(vm: &mut Vm, value: &JsValue) -> Res<RangeData> {
    let data = any_range_data(vm, value)?;
    if data.is_static {
        return Err(vm.type_error("Illegal invocation"));
    }
    Ok(data)
}

/** Store a Range's data back. */
pub(super) fn set_range(vm: &mut Vm, value: &JsValue, data: RangeData) {
    let Some(id) = value.object() else {
        return;
    };
    let nodes = range_live::nodes_of(vm, &data);
    let Some(Class::Host(host)) = vm.heap.get_mut(id).map(|object| &mut object.class) else {
        return;
    };
    **host = HostData::Range(Box::new(data));
    range_live::relink(vm, id, nodes);
}

/** A boundary's node identity and offset; a boundary whose node was collected is the document. */
pub(super) fn boundary(vm: &Vm, node: &JsValue, offset: usize) -> Boundary {
    (node_of(vm, node).unwrap_or(DOCUMENT), offset)
}

/** Whether a node holds character data (text, comment or processing instruction). */
pub(super) fn is_character_data(vm: &mut Vm, node: usize) -> bool {
    node != DOCUMENT && vm.page.tag(node).starts_with('#') && !is_container(vm, node)
}

/** The length the specification gives a node: text units for character data, child count otherwise. */
pub(super) fn node_length(vm: &mut Vm, node: usize) -> usize {
    if is_character_data(vm, node) {
        vm.page.text_content(node).encode_utf16().count()
    } else {
        children(vm, node).len()
    }
}

/** Fragments and shadow roots have children despite their `#` names. */
fn is_container(vm: &mut Vm, node: usize) -> bool {
    let tag = vm.page.tag(node);
    tag == components::FRAGMENT || tag == components::SHADOW_ROOT
}

/** A node's children, with the document's virtual root as its child. */
pub(super) fn children(vm: &mut Vm, node: usize) -> Vec<usize> {
    if node == DOCUMENT {
        vec![super::nodes::HTML]
    } else {
        vm.page.children(node)
    }
}

/** The index of `node` among `parent`'s children, or 0 when it is not one of them. */
pub(super) fn index_in(vm: &mut Vm, parent: usize, node: usize) -> usize {
    children(vm, parent)
        .iter()
        .position(|child| *child == node)
        .unwrap_or(0)
}

/** The inclusive ancestors of a node, nearest first, ending at the document. */
pub(super) fn inclusive_ancestors(vm: &mut Vm, node: usize) -> Vec<usize> {
    let mut chain = vec![node];
    chain.extend(vm.page.ancestors(node));
    if node != DOCUMENT && vm.page.connected(node) {
        chain.push(DOCUMENT);
    }
    chain
}

/** The root of a node's tree: the document when it is connected, else its topmost ancestor. */
pub(super) fn root_of(vm: &mut Vm, node: usize) -> usize {
    inclusive_ancestors(vm, node)
        .last()
        .copied()
        .unwrap_or(node)
}

/** Order two boundary points as the specification's "position" of a boundary point. */
pub(super) fn compare(vm: &mut Vm, a: Boundary, b: Boundary) -> Ordering {
    if a.0 == b.0 {
        return a.1.cmp(&b.1);
    }
    let chain_b = inclusive_ancestors(vm, b.0);
    if let Some(at) = chain_b.iter().position(|id| *id == a.0) {
        // A's node contains B's: compare A's offset with the child on the way down to B.
        let child = chain_b[at - 1];
        let index = children(vm, a.0).iter().position(|id| *id == child);
        return if index.is_some_and(|index| index < a.1) {
            Ordering::Greater
        } else {
            Ordering::Less
        };
    }
    let chain_a = inclusive_ancestors(vm, a.0);
    if let Some(at) = chain_a.iter().position(|id| *id == b.0) {
        let child = chain_a[at - 1];
        let index = children(vm, b.0).iter().position(|id| *id == child);
        return if index.is_some_and(|index| index < b.1) {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if let Some(order) = order_under_common_ancestor(vm, &chain_a, &chain_b) {
        return order;
    }
    // Different trees have no tree order; DOCUMENT_POSITION_FOLLOWING (4) means B comes after A.
    if dom::compare_position(vm, a.0, b.0) as u32 & 4 != 0 {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

/**
 * Tree order of two nodes that neither contains: the children of their deepest common ancestor
 * that lead to each one decide it. None when the two chains share no ancestor.
 */
fn order_under_common_ancestor(
    vm: &mut Vm,
    chain_a: &[usize],
    chain_b: &[usize],
) -> Option<Ordering> {
    let common = *chain_a.iter().find(|id| chain_b.contains(id))?;
    let at_a = chain_a.iter().position(|id| *id == common)?;
    let at_b = chain_b.iter().position(|id| *id == common)?;
    let branch_a = chain_a.get(at_a.checked_sub(1)?)?;
    let branch_b = chain_b.get(at_b.checked_sub(1)?)?;
    let kids = children(vm, common);
    let index_a = kids.iter().position(|kid| kid == branch_a)?;
    let index_b = kids.iter().position(|kid| kid == branch_b)?;
    Some(index_a.cmp(&index_b))
}

/**
 * Set one boundary as the specification's "set the start" or "set the end" does. A point in
 * another tree collapses the range onto it, and a point past the other end drags that end along.
 */
fn set_boundary(
    vm: &mut Vm,
    data: &mut RangeData,
    start: bool,
    node: JsValue,
    offset: usize,
) -> Res<()> {
    let Some(id) = node_of(vm, &node) else {
        return Err(vm.type_error("parameter 1 is not of type 'Node'"));
    };
    if offset > node_length(vm, id) {
        return Err(vm.dom_exception(
            "IndexSizeError",
            "The offset is larger than the node's length.",
        ));
    }
    let point = (id, offset);
    let other = if start {
        boundary(vm, &data.end, data.end_offset)
    } else {
        boundary(vm, &data.start, data.start_offset)
    };
    if root_of(vm, id) != root_of(vm, other.0) {
        return Ok(collapse_range_on(data, node, offset));
    }
    let order = compare(vm, point, other);
    if start {
        if order == Ordering::Greater {
            data.end = node.clone();
            data.end_offset = offset;
        }
        data.start = node;
        data.start_offset = offset;
    } else {
        if order == Ordering::Less {
            data.start = node.clone();
            data.start_offset = offset;
        }
        data.end = node;
        data.end_offset = offset;
    }
    Ok(())
}

/** Set both boundary points of a range to one point. */
fn collapse_range_on(data: &mut RangeData, node: JsValue, offset: usize) {
    data.start = node.clone();
    data.start_offset = offset;
    data.end = node;
    data.end_offset = offset;
}

/** The parent of a node argument and the node's index in it, for the Before/After setters. */
fn parent_point(vm: &mut Vm, call: &Call, after: bool) -> Res<(JsValue, usize)> {
    let node = dom::arg_node(vm, call, 0)?;
    let Some(parent) = vm.page.parent(node) else {
        return Err(vm.dom_exception("InvalidNodeTypeError", "The node has no parent."));
    };
    let index = index_in(vm, parent, node);
    Ok((wrap(vm, parent)?, index + usize::from(after)))
}

/** The deepest node containing both boundaries. */
pub(super) fn common_ancestor(vm: &mut Vm, data: &RangeData) -> usize {
    let start = boundary(vm, &data.start, data.start_offset).0;
    let end = boundary(vm, &data.end, data.end_offset).0;
    let chain = inclusive_ancestors(vm, end);
    inclusive_ancestors(vm, start)
        .into_iter()
        .find(|id| chain.contains(id))
        .unwrap_or(DOCUMENT)
}

/** Range accessors. */
pub(in crate::v9) fn range_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = any_range_data(vm, &call.this)?;
    Ok(match dom::native_name(&call) {
        "startContainer" => data.start,
        "startOffset" => JsValue::Number(data.start_offset as f64),
        "endContainer" => data.end,
        "endOffset" => JsValue::Number(data.end_offset as f64),
        "collapsed" => JsValue::Boolean(
            boundary(vm, &data.start, data.start_offset)
                == boundary(vm, &data.end, data.end_offset),
        ),
        "commonAncestorContainer" => {
            let ancestor = common_ancestor(vm, &data);
            wrap(vm, ancestor)?
        }
        _ => JsValue::Undefined,
    })
}

/** Range methods; the ones that change the document run the reactions they queue. */
pub(in crate::v9) fn range_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| range_method_steps(vm, call))
}

/** The range_method steps themselves. */
fn range_method_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = dom::native_name(&call);
    if name == "insertNode" {
        let node = dom::arg_node(vm, &call, 0)?;
        insert_node_at(vm, &call.this, node)?;
        return Ok(JsValue::Undefined);
    }
    let mut data = range_data(vm, &call.this)?;
    match name {
        "setStart" | "setEnd" => {
            let offset = dom::unsigned_long(vm, &call.arg(1))?;
            set_boundary(vm, &mut data, name == "setStart", call.arg(0), offset)?;
        }
        "setStartBefore" | "setStartAfter" | "setEndBefore" | "setEndAfter" => {
            let (parent, offset) = parent_point(vm, &call, name.ends_with("After"))?;
            set_boundary(vm, &mut data, name.starts_with("setStart"), parent, offset)?;
        }
        "selectNode" => {
            let (parent, offset) = parent_point(vm, &call, false)?;
            collapse_range_on(&mut data, parent.clone(), offset);
            data.end = parent;
            data.end_offset = offset + 1;
        }
        "selectNodeContents" => {
            let node = dom::arg_node(vm, &call, 0)?;
            let length = node_length(vm, node);
            collapse_range_on(&mut data, call.arg(0), 0);
            data.end_offset = length;
        }
        "collapse" => collapse(&mut data, call.arg(0).truthy()),
        "cloneRange" => return range_object(vm, data),
        "detach" => return Ok(JsValue::Undefined),
        "toString" => return JsValue::string(&text_of(vm, &data)).map_err(Into::into),
        "compareBoundaryPoints" => return compare_boundary_points(vm, &data, &call),
        "comparePoint" | "isPointInRange" | "intersectsNode" => {
            return point_queries(vm, &data, &call, name);
        }
        "getBoundingClientRect" | "getClientRects" => return client_rects(vm, &data, name),
        "createContextualFragment" => return contextual_fragment(vm, &call),
        "deleteContents" => range_contents::delete_contents(vm, &mut data)?,
        "extractContents" | "cloneContents" => {
            let fragment =
                range_contents::extract_contents(vm, &mut data, name == "cloneContents")?;
            set_range(vm, &call.this, data);
            return wrap(vm, fragment);
        }
        "surroundContents" => {
            surround_contents(vm, &call.this, &call, &mut data)?;
            return Ok(JsValue::Undefined);
        }
        _ => return Ok(JsValue::Undefined),
    }
    set_range(vm, &call.this, data);
    Ok(JsValue::Undefined)
}

/** getBoundingClientRect and getClientRects: the nominal box of the start node. */
fn client_rects(vm: &mut Vm, data: &RangeData, name: &str) -> Res<JsValue> {
    let node = boundary(vm, &data.start, data.start_offset).0;
    let target = if node == DOCUMENT {
        super::nodes::HTML
    } else {
        node
    };
    let rect = dom::rect(vm, target)?;
    if name == "getClientRects" {
        return Ok(JsValue::Object(vm.new_array(vec![rect])?));
    }
    Ok(rect)
}

/** Collapse a range to one of its ends. */
fn collapse(data: &mut RangeData, to_start: bool) {
    if to_start {
        data.end = data.start.clone();
        data.end_offset = data.start_offset;
    } else {
        data.start = data.end.clone();
        data.start_offset = data.end_offset;
    }
}

/** `compareBoundaryPoints(how, sourceRange)`: -1, 0 or 1, for ranges in the same tree. */
fn compare_boundary_points(vm: &mut Vm, data: &RangeData, call: &Call) -> Res<JsValue> {
    let how = vm.to_integer(&call.arg(0))? as i64;
    let other = range_data(vm, &call.arg(1))?;
    let (this_point, other_point) = compared_points(vm, data, &other, how)?;
    if root_of(vm, this_point.0) != root_of(vm, other_point.0) {
        return Err(vm.dom_exception(
            "WrongDocumentError",
            "The two ranges are not in the same tree.",
        ));
    }
    Ok(JsValue::Number(
        match compare(vm, this_point, other_point) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        },
    ))
}

/** The two points a comparison mode names: this range's and the source's, start or end each. */
fn compared_points(
    vm: &mut Vm,
    data: &RangeData,
    other: &RangeData,
    how: i64,
) -> Res<(Boundary, Boundary)> {
    if how < 0 || how as usize >= COMPARE_MODES.len() {
        return Err(vm.dom_exception(
            "NotSupportedError",
            "The comparison mode is not one of the four.",
        ));
    }
    let this_end = matches!(how, 1 | 2);
    let other_end = matches!(how, 2 | 3);
    Ok((
        range_point(vm, data, this_end),
        range_point(vm, other, other_end),
    ))
}

/** A range's start or end as a boundary point. */
fn range_point(vm: &Vm, data: &RangeData, end: bool) -> Boundary {
    if end {
        boundary(vm, &data.end, data.end_offset)
    } else {
        boundary(vm, &data.start, data.start_offset)
    }
}

/** `comparePoint`, `isPointInRange` and `intersectsNode`: the queries against a range's span. */
fn point_queries(vm: &mut Vm, data: &RangeData, call: &Call, name: &str) -> Res<JsValue> {
    let node = dom::arg_node(vm, call, 0)?;
    if name == "intersectsNode" {
        return Ok(JsValue::Boolean(intersects_node(vm, data, node)));
    }
    let start = range_point(vm, data, false);
    let end = range_point(vm, data, true);
    if root_of(vm, node) != root_of(vm, start.0) {
        if name == "comparePoint" {
            return Err(vm.dom_exception(
                "WrongDocumentError",
                "The point is not in the range's tree.",
            ));
        }
        return Ok(JsValue::Boolean(false));
    }
    let offset = dom::unsigned_long(vm, &call.arg(1))?;
    if offset > node_length(vm, node) {
        return Err(vm.dom_exception(
            "IndexSizeError",
            "The offset is larger than the node's length.",
        ));
    }
    let point = (node, offset);
    let position = if compare(vm, point, start) == Ordering::Less {
        -1.0
    } else if compare(vm, point, end) == Ordering::Greater {
        1.0
    } else {
        0.0
    };
    Ok(if name == "comparePoint" {
        JsValue::Number(position)
    } else {
        JsValue::Boolean(position == 0.0)
    })
}

/**
 * intersectsNode: a node meets the range when its span starts before the range ends and ends
 * after it starts. A node with no parent meets the range only as the range's own root.
 */
fn intersects_node(vm: &mut Vm, data: &RangeData, node: usize) -> bool {
    let start = range_point(vm, data, false);
    let end = range_point(vm, data, true);
    if root_of(vm, node) != root_of(vm, start.0) {
        return false;
    }
    let Some(parent) = vm.page.parent(node) else {
        return true;
    };
    let offset = index_in(vm, parent, node);
    compare(vm, (parent, offset), end) == Ordering::Less
        && compare(vm, (parent, offset + 1), start) == Ordering::Greater
}

/** The text between the boundaries, in document order. */
fn text_of(vm: &mut Vm, data: &RangeData) -> String {
    let start = boundary(vm, &data.start, data.start_offset);
    let end = boundary(vm, &data.end, data.end_offset);
    let root = common_ancestor(vm, data);
    let mut out = Vec::new();
    for node in text_nodes_under(vm, root) {
        let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
        let (lo, hi) = match (node == start.0, node == end.0) {
            (true, true) => (start.1, end.1),
            (true, false) => (start.1, units.len()),
            (false, true) => (0, end.1),
            (false, false) => {
                let inside = compare(vm, (node, 0), start) != Ordering::Less
                    && compare(vm, (node, units.len()), end) != Ordering::Greater;
                if !inside {
                    continue;
                }
                (0, units.len())
            }
        };
        out.extend_from_slice(
            &units[lo.min(units.len())..hi.min(units.len()).max(lo.min(units.len()))],
        );
    }
    String::from_utf16_lossy(&out)
}

/** A node's text nodes (itself included), in document order. */
fn text_nodes_under(vm: &mut Vm, root: usize) -> Vec<usize> {
    let mut nodes = vec![root];
    if root == DOCUMENT {
        nodes = vec![super::nodes::HTML];
        nodes.extend(vm.page.descendants(super::nodes::HTML));
    } else {
        nodes.extend(vm.page.descendants(root));
    }
    nodes
        .into_iter()
        .filter(|node| vm.page.tag(*node) == "#text")
        .collect()
}

/** `createContextualFragment(html)`: a fragment holding the parsed markup. */
fn contextual_fragment(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    // WebIDL: the markup argument is required, so a call without it is a TypeError, not an empty fragment.
    if call.args.is_empty() {
        return Err(vm.type_error("1 argument required, but only 0 present."));
    }
    let html = vm.to_text(&call.arg(0))?;
    let roots = super::html::parse_detached(&mut vm.page, &html)
        .map_err(|error| dom::dom_error(vm, error))?;
    let fragment = vm
        .page
        .create(components::FRAGMENT, None)
        .map_err(|error| dom::dom_error(vm, error))?;
    for root in roots {
        vm.page
            .insert(fragment, root, None)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    vm.page
        .mutations
        .push(components::Mutation::Created { root: fragment });
    wrap(vm, fragment)
}

/**
 * insertNode: the node goes in at the range's start, splitting a text start there. A collapsed
 * range then ends after the new node. The start is never set here: the insertion's live-range
 * rules have already placed it.
 */
fn insert_node_at(vm: &mut Vm, this: &JsValue, node: usize) -> Res<()> {
    let data = range_data(vm, this)?;
    let collapsed =
        boundary(vm, &data.start, data.start_offset) == boundary(vm, &data.end, data.end_offset);
    let (container, offset) = boundary(vm, &data.start, data.start_offset);
    let (parent, reference) = insertion_point(vm, container, offset)?;
    let count = if dom::node_type(vm, node) == 11 {
        children(vm, node).len()
    } else {
        1
    };
    let siblings = children(vm, parent);
    let index = reference
        .and_then(|reference| siblings.iter().position(|id| *id == reference))
        .unwrap_or(siblings.len());
    insert(vm, parent, node, reference)?;
    if collapsed {
        let mut data = range_data(vm, this)?;
        data.end = wrap(vm, parent)?;
        data.end_offset = index + count;
        set_range(vm, this, data);
    }
    Ok(())
}

/**
 * Where insertNode puts a node for a start point: before the tail a text start splits off, or
 * before the child at the offset. A text start with no parent, or a comment or processing
 * instruction, cannot take a node.
 */
fn insertion_point(vm: &mut Vm, container: usize, offset: usize) -> Res<(usize, Option<usize>)> {
    if container == DOCUMENT {
        return Err(vm.unsupported_error("inserting directly into the document"));
    }
    if !is_character_data(vm, container) {
        return Ok((container, children(vm, container).get(offset).copied()));
    }
    let tag = vm.page.tag(container);
    let Some(parent) = vm.page.parent(container).filter(|_| tag == "#text") else {
        return Err(vm.dom_exception(
            "HierarchyRequestError",
            "The start node cannot take a node.",
        ));
    };
    let tail = dom::split_text_node(vm, container, offset)?;
    Ok((parent, Some(tail)))
}

/**
 * surroundContents(newParent): the range's contents go into newParent, and newParent takes
 * their place, so the range ends up selecting it. Only a range that partly selects character
 * data may do this; a node the range partly selects otherwise is an error.
 */
fn surround_contents(vm: &mut Vm, this: &JsValue, call: &Call, data: &mut RangeData) -> Res<()> {
    let parent = dom::arg_node(vm, call, 0)?;
    if parent == DOCUMENT || vm.page.tag(parent) == components::FRAGMENT {
        return Err(vm.dom_exception(
            "InvalidNodeTypeError",
            "The new parent cannot be a document or a fragment.",
        ));
    }
    if has_partial_non_text(vm, data) {
        return Err(vm.dom_exception(
            "InvalidStateError",
            "The Range has partially selected a non-Text node.",
        ));
    }
    let fragment = range_contents::extract_contents(vm, data, false)?;
    set_range(vm, this, data.clone());
    for child in children(vm, parent) {
        remove_node(vm, child)?;
    }
    insert_node_at(vm, this, parent)?;
    insert(vm, parent, fragment, None)
}

/** Whether the range partly selects a node (inside one boundary's ancestors but not the other's) that is not character data. */
fn has_partial_non_text(vm: &mut Vm, data: &RangeData) -> bool {
    let start_node = boundary(vm, &data.start, data.start_offset).0;
    let end_node = boundary(vm, &data.end, data.end_offset).0;
    let start_chain = inclusive_ancestors(vm, start_node);
    let end_chain = inclusive_ancestors(vm, end_node);
    let partial: Vec<usize> = start_chain
        .iter()
        .chain(end_chain.iter())
        .filter(|&&node| !(start_chain.contains(&node) && end_chain.contains(&node)))
        .copied()
        .collect();
    partial.into_iter().any(|node| !is_character_data(vm, node))
}

/** The Selection object, created on first use and kept in the page state. */
pub(in crate::v9) fn selection(vm: &mut Vm) -> Res<JsValue> {
    if let Some(id) = vm.page.state.selection
        && vm.heap.get(id).is_some()
    {
        return Ok(JsValue::Object(id));
    }
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Selection(Vec::new()))),
        Some(realm().ids.selection_prototype),
    ))?;
    vm.page.state.selection = Some(id);
    Ok(JsValue::Object(id))
}

/** The ranges a Selection holds. */
fn selection_ranges(vm: &mut Vm, value: &JsValue) -> Res<Vec<JsValue>> {
    match host_data(vm, value) {
        Some(HostData::Selection(ranges)) => Ok(ranges),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** Store a Selection's ranges back. */
fn set_selection(vm: &mut Vm, value: &JsValue, ranges: Vec<JsValue>) {
    if let JsValue::Object(id) = value
        && let Some(Class::Host(host)) = vm.heap.get_mut(*id).map(|object| &mut object.class)
    {
        **host = HostData::Selection(ranges);
    }
}

/** Selection accessors: the anchor is the first range's start and the focus its end. */
pub(in crate::v9) fn selection_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let ranges = selection_ranges(vm, &call.this)?;
    let range = match ranges.first() {
        Some(range) => Some(range_data(vm, range)?),
        None => None,
    };
    Ok(match dom::native_name(&call) {
        "rangeCount" => JsValue::Number(ranges.len() as f64),
        "type" => JsValue::string(match &range {
            None => "None",
            Some(data)
                if boundary(vm, &data.start, data.start_offset)
                    == boundary(vm, &data.end, data.end_offset) =>
            {
                "Caret"
            }
            Some(_) => "Range",
        })?,
        "isCollapsed" => JsValue::Boolean(range.as_ref().is_none_or(|data| {
            boundary(vm, &data.start, data.start_offset) == boundary(vm, &data.end, data.end_offset)
        })),
        "anchorNode" | "baseNode" => range.map_or(JsValue::Null, |data| data.start),
        "anchorOffset" | "baseOffset" => {
            JsValue::Number(range.map_or(0.0, |data| data.start_offset as f64))
        }
        "focusNode" | "extentNode" => range.map_or(JsValue::Null, |data| data.end),
        "focusOffset" | "extentOffset" => {
            JsValue::Number(range.map_or(0.0, |data| data.end_offset as f64))
        }
        "direction" => JsValue::string(if range.is_some() { "forward" } else { "none" })?,
        _ => JsValue::Undefined,
    })
}

/** Selection methods. */
pub(in crate::v9) fn selection_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| selection_method_steps(vm, call))
}

/** The selection_method steps themselves. */
fn selection_method_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut ranges = selection_ranges(vm, &call.this)?;
    let name = dom::native_name(&call);
    match name {
        "getRangeAt" => {
            let index = vm.to_integer(&call.arg(0))?;
            return match ranges.get(index.max(0.0) as usize) {
                Some(range) if index >= 0.0 => Ok(range.clone()),
                _ => Err(vm.dom_exception(
                    "IndexSizeError",
                    "The index is not a range in the selection.",
                )),
            };
        }
        "addRange" => {
            range_data(vm, &call.arg(0))?;
            if ranges.is_empty() {
                ranges.push(call.arg(0));
            }
        }
        "removeRange" => ranges.retain(|range| *range != call.arg(0)),
        "removeAllRanges" | "empty" => ranges.clear(),
        "collapse" | "setPosition" => {
            if call.arg(0).nullish() {
                ranges.clear();
            } else {
                let offset = dom::unsigned_long(vm, &call.arg(1))?;
                let mut data = collapsed_data();
                set_boundary(vm, &mut data, true, call.arg(0), offset)?;
                collapse(&mut data, true);
                ranges = vec![range_object(vm, data)?];
            }
        }
        "collapseToStart" | "collapseToEnd" => {
            let Some(range) = ranges.first().cloned() else {
                return Err(vm.dom_exception("InvalidStateError", "The selection has no range."));
            };
            let mut data = range_data(vm, &range)?;
            collapse(&mut data, name == "collapseToStart");
            ranges = vec![range_object(vm, data)?];
        }
        "extend" => {
            let Some(range) = ranges.first().cloned() else {
                return Err(vm.dom_exception("InvalidStateError", "The selection has no range."));
            };
            let mut data = range_data(vm, &range)?;
            let offset = dom::unsigned_long(vm, &call.arg(1))?;
            set_boundary(vm, &mut data, false, call.arg(0), offset)?;
            ranges = vec![range_object(vm, data)?];
        }
        "setBaseAndExtent" => {
            let mut data = collapsed_data();
            let anchor = dom::unsigned_long(vm, &call.arg(1))?;
            let focus = dom::unsigned_long(vm, &call.arg(3))?;
            set_boundary(vm, &mut data, true, call.arg(0), anchor)?;
            set_boundary(vm, &mut data, false, call.arg(2), focus)?;
            ranges = vec![range_object(vm, data)?];
        }
        "selectAllChildren" => {
            let node = dom::arg_node(vm, &call, 0)?;
            let length = node_length(vm, node);
            let mut data = collapsed_data();
            data.start = call.arg(0);
            data.end = call.arg(0);
            data.end_offset = length;
            ranges = vec![range_object(vm, data)?];
        }
        "deleteFromDocument" => {
            if let Some(range) = ranges.first().cloned() {
                let mut data = range_data(vm, &range)?;
                range_contents::delete_contents(vm, &mut data)?;
                set_range(vm, &range, data);
            }
        }
        "containsNode" => {
            let Some(range) = ranges.first().cloned() else {
                return Ok(JsValue::Boolean(false));
            };
            let data = range_data(vm, &range)?;
            return point_queries(vm, &data, &call, "intersectsNode");
        }
        "toString" => {
            let text = match ranges.first().cloned() {
                Some(range) => {
                    let data = range_data(vm, &range)?;
                    text_of(vm, &data)
                }
                None => String::new(),
            };
            return JsValue::string(&text).map_err(Into::into);
        }
        _ => return Ok(JsValue::Undefined),
    }
    set_selection(vm, &call.this, ranges);
    Ok(JsValue::Undefined)
}

#[cfg(test)]
mod tests {
    use super::COMPARE_MODES;

    /** The comparison modes keep the specification's numbering. */
    #[test]
    fn compare_modes_follow_the_specification_order() {
        let values: Vec<f64> = COMPARE_MODES.iter().map(|(_, value)| *value).collect();
        assert_eq!(values, vec![0.0, 1.0, 2.0, 3.0]);
    }
}
