/**
 * Range and Selection: boundary points over the page's nodes, as `document.createRange`,
 * `new Range()` and `getSelection()` give them. Boundaries hold node wrappers rather than
 * identities, so a range over a script-created node follows it when insertion renumbers it.
 * V9 has no layout or caret, so client rectangles are the nominal boxes `getBoundingClientRect`
 * reports and the selection holds whatever ranges scripts put there.
 */
use super::dom::{self, DOCUMENT, insert, node_of, remove_node, wrap};
use super::{components, host_data};
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData, RangeData};
use crate::v9::vm::heap::Object;
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
type Boundary = (usize, usize);

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
    }
}

/** Allocate a Range host object over its data. */
fn range_object(vm: &mut Vm, data: RangeData) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Range(Box::new(data)))),
        Some(realm().ids.range_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** `new Range()`. */
pub(in crate::v9) fn construct_range(vm: &mut Vm, _: Call) -> Res<JsValue> {
    new_range(vm)
}

/** The data behind a Range, or an Illegal invocation error. */
fn range_data(vm: &mut Vm, value: &JsValue) -> Res<RangeData> {
    match host_data(vm, value) {
        Some(HostData::Range(data)) => Ok(*data),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** Store a Range's data back. */
fn set_range(vm: &mut Vm, value: &JsValue, data: RangeData) {
    if let JsValue::Object(id) = value
        && let Some(Class::Host(host)) = vm.heap.get_mut(*id).map(|object| &mut object.class)
    {
        **host = HostData::Range(Box::new(data));
    }
}

/** A boundary's node identity and offset; a boundary whose node was collected is the document. */
fn boundary(vm: &Vm, node: &JsValue, offset: usize) -> Boundary {
    (node_of(vm, node).unwrap_or(DOCUMENT), offset)
}

/** The length the specification gives a node: text units for text, child count otherwise. */
fn node_length(vm: &mut Vm, node: usize) -> usize {
    if node != DOCUMENT && vm.page.tag(node).starts_with('#') && !is_container(vm, node) {
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
fn children(vm: &mut Vm, node: usize) -> Vec<usize> {
    if node == DOCUMENT {
        vec![super::nodes::HTML]
    } else {
        vm.page.children(node)
    }
}

/** The inclusive ancestors of a node, nearest first, ending at the document. */
fn inclusive_ancestors(vm: &mut Vm, node: usize) -> Vec<usize> {
    let mut chain = vec![node];
    chain.extend(vm.page.ancestors(node));
    if node != DOCUMENT && vm.page.connected(node) {
        chain.push(DOCUMENT);
    }
    chain
}

/** Order two boundary points as the specification's "position" of a boundary point. */
fn compare(vm: &mut Vm, a: Boundary, b: Boundary) -> Ordering {
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
    // Neither contains the other: DOCUMENT_POSITION_FOLLOWING (4) means B comes after A.
    if dom::compare_position(vm, a.0, b.0) as u32 & 4 != 0 {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

/** Set one boundary, collapsing the other past it, as the specification's "set the start". */
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
    if start {
        if compare(vm, point, boundary(vm, &data.end, data.end_offset)) == Ordering::Greater {
            data.end = node.clone();
            data.end_offset = offset;
        }
        data.start = node;
        data.start_offset = offset;
    } else {
        if compare(vm, point, boundary(vm, &data.start, data.start_offset)) == Ordering::Less {
            data.start = node.clone();
            data.start_offset = offset;
        }
        data.end = node;
        data.end_offset = offset;
    }
    Ok(())
}

/** The parent of a node argument and the node's index in it, for the Before/After setters. */
fn parent_point(vm: &mut Vm, call: &Call, after: bool) -> Res<(JsValue, usize)> {
    let node = dom::arg_node(vm, call, 0)?;
    let Some(parent) = vm.page.parent(node) else {
        return Err(vm.dom_exception("InvalidNodeTypeError", "The node has no parent."));
    };
    let index = vm
        .page
        .children(parent)
        .iter()
        .position(|id| *id == node)
        .unwrap_or(0);
    Ok((wrap(vm, parent)?, index + usize::from(after)))
}

/** The deepest node containing both boundaries. */
fn common_ancestor(vm: &mut Vm, data: &RangeData) -> usize {
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
    let data = range_data(vm, &call.this)?;
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
    let mut data = range_data(vm, &call.this)?;
    let name = dom::native_name(&call);
    match name {
        "setStart" | "setEnd" => {
            let offset = vm.to_integer(&call.arg(1))?.max(0.0) as usize;
            set_boundary(vm, &mut data, name == "setStart", call.arg(0), offset)?;
        }
        "setStartBefore" | "setStartAfter" | "setEndBefore" | "setEndAfter" => {
            let (parent, offset) = parent_point(vm, &call, name.ends_with("After"))?;
            set_boundary(vm, &mut data, name.starts_with("setStart"), parent, offset)?;
        }
        "selectNode" => {
            let (parent, offset) = parent_point(vm, &call, false)?;
            data.start = parent.clone();
            data.start_offset = offset;
            data.end = parent;
            data.end_offset = offset + 1;
        }
        "selectNodeContents" => {
            let node = dom::arg_node(vm, &call, 0)?;
            data.start = call.arg(0);
            data.start_offset = 0;
            data.end = call.arg(0);
            data.end_offset = node_length(vm, node);
        }
        "collapse" => collapse(&mut data, call.arg(0).truthy()),
        "cloneRange" => return range_object(vm, data),
        "detach" => return Ok(JsValue::Undefined),
        "toString" => return JsValue::string(&text_of(vm, &data)).map_err(Into::into),
        "compareBoundaryPoints" => return compare_boundary_points(vm, &data, &call),
        "comparePoint" | "isPointInRange" | "intersectsNode" => {
            return point_queries(vm, &data, &call, name);
        }
        "getBoundingClientRect" | "getClientRects" => {
            let node = boundary(vm, &data.start, data.start_offset).0;
            let rect = dom::rect(
                vm,
                if node == DOCUMENT {
                    super::nodes::HTML
                } else {
                    node
                },
            )?;
            if name == "getClientRects" {
                return Ok(JsValue::Object(vm.new_array(vec![rect])?));
            }
            return Ok(rect);
        }
        "createContextualFragment" => return contextual_fragment(vm, &call),
        "insertNode" => insert_node(vm, &mut data, &call)?,
        "deleteContents" => {
            delete_contents(vm, &mut data)?;
        }
        "extractContents" | "cloneContents" => {
            let fragment = extract_contents(vm, &mut data, name == "cloneContents")?;
            set_range(vm, &call.this, data);
            return wrap(vm, fragment);
        }
        "surroundContents" => {
            let parent = dom::arg_node(vm, &call, 0)?;
            let fragment = extract_contents(vm, &mut data, false)?;
            insert_node_at(vm, &mut data, parent)?;
            insert(vm, parent, fragment, None)?;
            let (grandparent, offset) = parent_point_of(vm, parent)?;
            data.start = grandparent.clone();
            data.start_offset = offset;
            data.end = grandparent;
            data.end_offset = offset + 1;
        }
        _ => return Ok(JsValue::Undefined),
    }
    set_range(vm, &call.this, data);
    Ok(JsValue::Undefined)
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

/** A node's parent wrapper and index in it. */
fn parent_point_of(vm: &mut Vm, node: usize) -> Res<(JsValue, usize)> {
    let Some(parent) = vm.page.parent(node) else {
        return Err(vm.dom_exception("InvalidNodeTypeError", "The node has no parent."));
    };
    let index = vm
        .page
        .children(parent)
        .iter()
        .position(|id| *id == node)
        .unwrap_or(0);
    Ok((wrap(vm, parent)?, index))
}

/** `compareBoundaryPoints(how, sourceRange)`: -1, 0 or 1. */
fn compare_boundary_points(vm: &mut Vm, data: &RangeData, call: &Call) -> Res<JsValue> {
    let how = vm.to_integer(&call.arg(0))?;
    let other = range_data(vm, &call.arg(1))?;
    let (this_point, other_point) = match how as i64 {
        0 => (
            boundary(vm, &data.start, data.start_offset),
            boundary(vm, &other.start, other.start_offset),
        ),
        1 => (
            boundary(vm, &data.end, data.end_offset),
            boundary(vm, &other.start, other.start_offset),
        ),
        2 => (
            boundary(vm, &data.end, data.end_offset),
            boundary(vm, &other.end, other.end_offset),
        ),
        3 => (
            boundary(vm, &data.start, data.start_offset),
            boundary(vm, &other.end, other.end_offset),
        ),
        _ => {
            return Err(vm.dom_exception(
                "NotSupportedError",
                "The comparison mode is not one of the four.",
            ));
        }
    };
    Ok(JsValue::Number(
        match compare(vm, this_point, other_point) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        },
    ))
}

/** `comparePoint`, `isPointInRange` and `intersectsNode`. */
fn point_queries(vm: &mut Vm, data: &RangeData, call: &Call, name: &str) -> Res<JsValue> {
    let node = dom::arg_node(vm, call, 0)?;
    let start = boundary(vm, &data.start, data.start_offset);
    let end = boundary(vm, &data.end, data.end_offset);
    if name == "intersectsNode" {
        let Some(parent) = vm.page.parent(node) else {
            return Ok(JsValue::Boolean(node == DOCUMENT));
        };
        let offset = vm
            .page
            .children(parent)
            .iter()
            .position(|id| *id == node)
            .unwrap_or(0);
        let before = compare(vm, (parent, offset), end) == Ordering::Less;
        let after = compare(vm, (parent, offset + 1), start) == Ordering::Greater;
        return Ok(JsValue::Boolean(before && after));
    }
    let offset = vm.to_integer(&call.arg(1))?.max(0.0) as usize;
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

/** `insertNode(node)`: insert at the start boundary, splitting a text node there. */
fn insert_node(vm: &mut Vm, data: &mut RangeData, call: &Call) -> Res<()> {
    let node = dom::arg_node(vm, call, 0)?;
    insert_node_at(vm, data, node)
}

/** Insert a node at the start boundary; the range then also ends after it when collapsed. */
fn insert_node_at(vm: &mut Vm, data: &mut RangeData, node: usize) -> Res<()> {
    let collapsed =
        boundary(vm, &data.start, data.start_offset) == boundary(vm, &data.end, data.end_offset);
    let (container, offset) = boundary(vm, &data.start, data.start_offset);
    let (parent, reference) = if container != DOCUMENT && vm.page.tag(container) == "#text" {
        let tail = split_text(vm, container, offset)?;
        let Some(parent) = vm.page.parent(container) else {
            return Err(vm.dom_exception("HierarchyRequestError", "The text node has no parent."));
        };
        (parent, Some(tail))
    } else if container == DOCUMENT {
        return Err(vm.unsupported_error("inserting directly into the document"));
    } else {
        let reference = vm.page.children(container).get(offset).copied();
        (container, reference)
    };
    let before = vm.page.children(parent).len();
    insert(vm, parent, node, reference)?;
    if collapsed {
        let added = vm.page.children(parent).len().saturating_sub(before);
        let index = reference
            .and_then(|reference| {
                vm.page
                    .children(parent)
                    .iter()
                    .position(|id| *id == reference)
            })
            .unwrap_or_else(|| vm.page.children(parent).len());
        data.end = wrap(vm, parent)?;
        data.end_offset = index;
        data.start = data.end.clone();
        data.start_offset = index.saturating_sub(added);
    }
    Ok(())
}

/** Split a text node at a unit offset, returning the new tail node after it. */
fn split_text(vm: &mut Vm, node: usize, offset: usize) -> Res<usize> {
    let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
    let at = offset.min(units.len());
    let head = String::from_utf16_lossy(&units[..at]);
    let tail = String::from_utf16_lossy(&units[at..]);
    let old = vm.page.text_content(node);
    vm.page
        .set_text(node, &head)
        .map_err(|error| dom::dom_error(vm, error))?;
    vm.page
        .mutations
        .push(components::Mutation::Text { node, old });
    let new = vm
        .page
        .create("#text", Some(tail))
        .map_err(|error| dom::dom_error(vm, error))?;
    if let Some(parent) = vm.page.parent(node) {
        let next = vm
            .page
            .children(parent)
            .into_iter()
            .skip_while(|id| *id != node)
            .nth(1);
        insert(vm, parent, new, next)?;
    }
    Ok(new)
}

/** Remove the contents and collapse to the start. */
fn delete_contents(vm: &mut Vm, data: &mut RangeData) -> Res<()> {
    let fragment = extract_contents(vm, data, false)?;
    for child in vm.page.children(fragment) {
        vm.page
            .remove(child)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    vm.page
        .remove(fragment)
        .map_err(|error| dom::dom_error(vm, error))?;
    Ok(())
}

/**
 * Move (or copy) the contents into a new fragment: whole nodes between the boundaries, and the
 * selected parts of text nodes at either end. ponytail: elements partially inside the range are
 * neither split nor copied, as the specification's partially-contained clones would be.
 */
fn extract_contents(vm: &mut Vm, data: &mut RangeData, copy: bool) -> Res<usize> {
    let fragment = vm
        .page
        .create(components::FRAGMENT, None)
        .map_err(|error| dom::dom_error(vm, error))?;
    let start = boundary(vm, &data.start, data.start_offset);
    let end = boundary(vm, &data.end, data.end_offset);
    if start == end {
        return Ok(fragment);
    }
    if start.0 == end.0 && vm.page.tag(start.0) == "#text" {
        let piece = take_text(vm, start.0, start.1, end.1, copy)?;
        vm.page
            .insert(fragment, piece, None)
            .map_err(|error| dom::dom_error(vm, error))?;
        collapse(data, true);
        return Ok(fragment);
    }
    let root = common_ancestor(vm, data);
    let mut roots = Vec::new();
    for (index, child) in children(vm, root).into_iter().enumerate() {
        if compare(vm, (root, index), start) != Ordering::Less
            && compare(vm, (root, index + 1), end) != Ordering::Greater
        {
            roots.push(child);
        }
    }
    let partial_start = (vm.page.tag(start.0) == "#text" && start.0 != root).then_some(start);
    let partial_end = (vm.page.tag(end.0) == "#text" && end.0 != root).then_some(end);
    if let Some((node, offset)) = partial_start {
        let length = node_length(vm, node);
        let piece = take_text(vm, node, offset, length, copy)?;
        vm.page
            .insert(fragment, piece, None)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    for child in roots {
        let moved = if copy {
            dom::clone_node(vm, child, true)?
        } else {
            child
        };
        if !copy {
            remove_node(vm, child)?;
        }
        vm.page
            .insert(fragment, moved, None)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    if let Some((node, offset)) = partial_end {
        let piece = take_text(vm, node, 0, offset, copy)?;
        vm.page
            .insert(fragment, piece, None)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    if !copy {
        data.start = wrap(vm, root)?;
        data.start_offset = boundary_index(vm, root, start);
        collapse(data, true);
    }
    Ok(fragment)
}

/** The child index of `root` that a boundary point inside it lies at or after. */
fn boundary_index(vm: &mut Vm, root: usize, point: Boundary) -> usize {
    let kids = children(vm, root);
    if point.0 == root {
        return point.1.min(kids.len());
    }
    let chain = inclusive_ancestors(vm, point.0);
    chain
        .iter()
        .find_map(|id| kids.iter().position(|kid| kid == id))
        .unwrap_or(kids.len())
}

/** A new text node holding `[from, to)` of a text node, removed from (or copied out of) it. */
fn take_text(vm: &mut Vm, node: usize, from: usize, to: usize, copy: bool) -> Res<usize> {
    let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
    let from = from.min(units.len());
    let to = to.clamp(from, units.len());
    let piece = String::from_utf16_lossy(&units[from..to]);
    if !copy {
        let mut rest = units[..from].to_vec();
        rest.extend_from_slice(&units[to..]);
        let old = vm.page.text_content(node);
        vm.page
            .set_text(node, &String::from_utf16_lossy(&rest))
            .map_err(|error| dom::dom_error(vm, error))?;
        vm.page
            .mutations
            .push(components::Mutation::Text { node, old });
    }
    vm.page
        .create("#text", Some(piece))
        .map_err(|error| dom::dom_error(vm, error))
}

/** The page's one Selection object, created on first use and kept in the page state. */
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
                let offset = vm.to_integer(&call.arg(1))?.max(0.0) as usize;
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
            let offset = vm.to_integer(&call.arg(1))?.max(0.0) as usize;
            set_boundary(vm, &mut data, false, call.arg(0), offset)?;
            ranges = vec![range_object(vm, data)?];
        }
        "setBaseAndExtent" => {
            let mut data = collapsed_data();
            let anchor = vm.to_integer(&call.arg(1))?.max(0.0) as usize;
            let focus = vm.to_integer(&call.arg(3))?.max(0.0) as usize;
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
                delete_contents(vm, &mut data)?;
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
