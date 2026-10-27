/**
 * Live range maintenance: every Range object and Selection range keeps its boundary points as
 * the DOM standard's "live range" rules say when the tree changes. The DOM's mutation paths call
 * these hooks just before or after they change the tree, with the parent, index and node the
 * standard names, so a point always follows the node it sat in. Ranges are registered when made
 * and held weakly: the page's sweep drops one that the heap has collected.
 *
 * Each hook reads only the ranges linked under the nodes it affects (range_index). A removal also
 * walks the removed subtree, which is the standard's own cost: the points inside it must be found.
 */
use super::dom::wrap;
use super::range::{Boundary, boundary, children, range_data, set_range};
use super::range_index::Nodes;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::RangeData;
use crate::v9::vm::{Res, Vm};
use std::collections::BTreeSet;

/** The two containers of a range's data, as the node index keys them. */
pub(super) fn nodes_of(vm: &Vm, data: &RangeData) -> Nodes {
    (
        boundary(vm, &data.start, data.start_offset).0,
        boundary(vm, &data.end, data.end_offset).0,
    )
}

/** Track a Range object, linked under its containers, so later mutations move its points. */
pub(super) fn register(vm: &mut Vm, id: ObjectId, nodes: Nodes) {
    vm.page.state.live_ranges.place(id, nodes);
}

/** A range's points were rewritten: re-link it under its new containers if it is registered. */
pub(super) fn relink(vm: &mut Vm, id: ObjectId, nodes: Nodes) {
    if vm.page.state.live_ranges.contains(id) {
        vm.page.state.live_ranges.place(id, nodes);
    }
}

/** A node was renumbered by an insertion: the ranges linked under it follow the new identity. */
pub(super) fn renumber(vm: &mut Vm, old: usize, new: usize) {
    vm.page.state.live_ranges.renumber(old, new);
}

/** A node left `parent`, where it sat at `index`: points inside it move to the parent's place. */
pub(super) fn removed(vm: &mut Vm, parent: usize, index: usize, node: usize) -> Res<()> {
    ensure_indexed(vm);
    if vm.page.state.live_ranges.is_empty() {
        return Ok(());
    }
    let subtree = subtree_of(vm, node);
    let mut nodes: Vec<usize> = subtree.iter().copied().collect();
    nodes.push(parent);
    move_at(vm, &nodes, |_, point| {
        removal_point(point, &subtree, parent, index)
    })
}

/** Where one point goes when a subtree leaves `parent` at `index`: inside the subtree it moves to the gap. */
fn removal_point(
    point: Boundary,
    subtree: &BTreeSet<usize>,
    parent: usize,
    index: usize,
) -> Boundary {
    if subtree.contains(&point.0) {
        (parent, index)
    } else if point.0 == parent && point.1 > index {
        (parent, point.1 - 1)
    } else {
        point
    }
}

/** `count` nodes entered `parent` at `index`: points after that index in the parent move on. */
pub(super) fn inserted(vm: &mut Vm, parent: usize, index: usize, count: usize) -> Res<()> {
    move_at(vm, &[parent], |_, point| {
        if point.0 == parent && point.1 > index {
            (parent, point.1 + count)
        } else {
            point
        }
    })
}

/** Character data of `node` lost `count` units at `offset` and gained `new_len` units there. */
pub(super) fn replaced(
    vm: &mut Vm,
    node: usize,
    offset: usize,
    count: usize,
    new_len: usize,
) -> Res<()> {
    move_at(vm, &[node], |_, point| {
        replaced_point(point, node, offset, count, new_len)
    })
}

/** Where one point goes when character data is replaced: inside the span it snaps to the start. */
fn replaced_point(
    point: Boundary,
    node: usize,
    offset: usize,
    count: usize,
    new_len: usize,
) -> Boundary {
    if point.0 != node || point.1 <= offset {
        point
    } else if point.1 <= offset + count {
        (node, offset)
    } else {
        (node, point.1 + new_len - count)
    }
}

/**
 * splitText moved the data after `offset` of `node` into `created`, which sits after it in
 * `parent` (at `index`, when there is one). Points past the split follow the data; points in the
 * parent just after `node` step past the new node.
 */
pub(super) fn split(
    vm: &mut Vm,
    node: usize,
    offset: usize,
    created: usize,
    parent: Option<usize>,
    index: usize,
) -> Res<()> {
    let nodes: Vec<usize> = std::iter::once(node).chain(parent).collect();
    move_at(vm, &nodes, |_, point| {
        if point.0 == node && point.1 > offset {
            (created, point.1 - offset)
        } else if Some(point.0) == parent && point.1 == index + 1 {
            (point.0, point.1 + 1)
        } else {
            point
        }
    })
}

/**
 * normalize folds `child`, the text node at `index` in `parent`, into `first`, whose text had
 * `length` units. Points in `child` move to `first` past its text; points at `child`'s place in
 * the parent move to the end of `first`.
 */
pub(super) fn merged(
    vm: &mut Vm,
    first: usize,
    child: usize,
    parent: Option<usize>,
    index: usize,
    length: usize,
) -> Res<()> {
    let nodes: Vec<usize> = std::iter::once(child).chain(parent).collect();
    move_at(vm, &nodes, |_, point| {
        if point.0 == child {
            (first, length + point.1)
        } else if Some(point.0) == parent && point.1 == index {
            (first, length)
        } else {
            point
        }
    })
}

/** The node and every node under it, found by walking the tree: the cost a removal walk has. */
fn subtree_of(vm: &mut Vm, root: usize) -> BTreeSet<usize> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if seen.insert(node) {
            stack.extend(children(vm, node));
        }
    }
    seen
}

/** Links the ranges of a registry loaded in the old bare-id form, from the heap, once. */
fn ensure_indexed(vm: &mut Vm) {
    if vm.page.state.live_ranges.is_indexed() {
        return;
    }
    for id in vm.page.state.live_ranges.registered() {
        if let Ok(data) = range_data(vm, &JsValue::Object(id)) {
            let nodes = nodes_of(vm, &data);
            vm.page.state.live_ranges.place(id, nodes);
        }
    }
    vm.page.state.live_ranges.mark_indexed();
}

/** Apply `adjust` to the points of every range linked under any of `nodes`. */
fn move_at<F>(vm: &mut Vm, nodes: &[usize], adjust: F) -> Res<()>
where
    F: Fn(&mut Vm, Boundary) -> Boundary,
{
    ensure_indexed(vm);
    let affected: BTreeSet<ObjectId> = nodes
        .iter()
        .flat_map(|node| vm.page.state.live_ranges.ranges_at(*node))
        .collect();
    for id in affected {
        move_range(vm, id, &adjust)?;
    }
    Ok(())
}

/**
 * Apply `adjust` to both points of one range and store it only if a point moved, so an untouched
 * range keeps its wrappers. Storing re-links the range under its new containers.
 */
fn move_range<F>(vm: &mut Vm, id: ObjectId, adjust: &F) -> Res<()>
where
    F: Fn(&mut Vm, Boundary) -> Boundary,
{
    let handle = JsValue::Object(id);
    let Ok(mut data) = range_data(vm, &handle) else {
        return Ok(());
    };
    let start = boundary(vm, &data.start, data.start_offset);
    let end = boundary(vm, &data.end, data.end_offset);
    let new_start = adjust(vm, start);
    let new_end = adjust(vm, end);
    if new_start == start && new_end == end {
        return Ok(());
    }
    if new_start != start {
        data.start = wrap(vm, new_start.0)?;
        data.start_offset = new_start.1;
    }
    if new_end != end {
        data.end = wrap(vm, new_end.0)?;
        data.end_offset = new_end.1;
    }
    set_range(vm, &handle, data);
    Ok(())
}
