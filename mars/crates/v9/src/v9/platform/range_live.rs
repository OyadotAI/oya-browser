/**
 * Live range maintenance: every Range object and Selection range keeps its boundary points as
 * the DOM standard's "live range" rules say when the tree changes. The DOM's mutation paths call
 * these hooks just before or after they change the tree, with the parent, index and node the
 * standard names, so a point always follows the node it sat in. Ranges are registered when made
 * and held weakly: the page's sweep drops one that the heap has collected.
 *
 * ponytail: each hook walks every registered range, so a mutation costs O(live ranges). The
 * standard requires that walk; a per-node index of ranges would cut it if pages held many ranges.
 */
use super::dom::wrap;
use super::range::{Boundary, boundary, range_data, set_range};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::{Res, Vm};

/** Track a new Range object so that later mutations move its boundary points. */
pub(super) fn register(vm: &mut Vm, id: ObjectId) {
    vm.page.state.live_ranges.push(id);
}

/** A node left `parent`, where it sat at `index`: points inside it move to the parent's place. */
pub(super) fn removed(vm: &mut Vm, parent: usize, index: usize, node: usize) -> Res<()> {
    move_points(vm, |vm, point| {
        removal_point(vm, point, parent, index, node)
    })
}

/** Where one point goes when `node` is removed from `parent` at `index`. */
fn removal_point(
    vm: &mut Vm,
    point: Boundary,
    parent: usize,
    index: usize,
    node: usize,
) -> Boundary {
    if point.0 == node || vm.page.ancestors(point.0).contains(&node) {
        (parent, index)
    } else if point.0 == parent && point.1 > index {
        (parent, point.1 - 1)
    } else {
        point
    }
}

/** `count` nodes entered `parent` at `index`: points after that index in the parent move on. */
pub(super) fn inserted(vm: &mut Vm, parent: usize, index: usize, count: usize) -> Res<()> {
    move_points(vm, |_, point| {
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
    move_points(vm, |_, point| {
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
    move_points(vm, |_, point| {
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
    move_points(vm, |_, point| {
        if point.0 == child {
            (first, length + point.1)
        } else if Some(point.0) == parent && point.1 == index {
            (first, length)
        } else {
            point
        }
    })
}

/** The live ranges still registered, after dropping any whose objects are gone. */
fn registered(vm: &mut Vm) -> Vec<ObjectId> {
    let heap = &vm.heap;
    vm.page
        .state
        .live_ranges
        .retain(|id| heap.get(*id).is_some());
    vm.page.state.live_ranges.clone()
}

/**
 * Apply `adjust` to both boundary points of every registered range and store the results. A
 * point is rewritten only when it changes, so an untouched range keeps its wrappers.
 */
fn move_points<F>(vm: &mut Vm, adjust: F) -> Res<()>
where
    F: Fn(&mut Vm, Boundary) -> Boundary,
{
    for id in registered(vm) {
        let handle = JsValue::Object(id);
        let Ok(mut data) = range_data(vm, &handle) else {
            continue;
        };
        let start = boundary(vm, &data.start, data.start_offset);
        let end = boundary(vm, &data.end, data.end_offset);
        let new_start = adjust(vm, start);
        let new_end = adjust(vm, end);
        if new_start != start {
            data.start = wrap(vm, new_start.0)?;
            data.start_offset = new_start.1;
        }
        if new_end != end {
            data.end = wrap(vm, new_end.0)?;
            data.end_offset = new_end.1;
        }
        set_range(vm, &handle, data);
    }
    Ok(())
}
