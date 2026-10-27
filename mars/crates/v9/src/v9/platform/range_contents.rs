/**
 * Range contents as the DOM standard runs extractContents, cloneContents and deleteContents.
 * Nodes wholly inside the range move (or are copied) whole. An element the range only partly
 * covers is copied shallowly, and its selected part is extracted into that copy, so the result
 * keeps the markup around the selection. Character data at an edge gives up its selected units.
 * Every position the result needs is read before the first mutation, as the standard reads it.
 */
use super::components::FRAGMENT;
use super::dom::{self, replace_data};
use super::range::{
    Boundary, boundary, children, common_ancestor, compare, inclusive_ancestors, index_in,
    is_character_data, node_length,
};
use crate::v9::vm::data::RangeData;
use crate::v9::vm::{Res, Vm};
use std::cmp::Ordering;

/** deleteContents: the contents are extracted and the fragment that holds them is dropped. */
pub(super) fn delete_contents(vm: &mut Vm, data: &mut RangeData) -> Res<()> {
    let fragment = extract_contents(vm, data, false)?;
    for child in vm.page.children(fragment) {
        vm.page
            .remove(child)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    vm.page
        .remove(fragment)
        .map_err(|error| dom::dom_error(vm, error))
}

/** extractContents, or cloneContents when `copy`: the range's contents in a new fragment. */
pub(super) fn extract_contents(vm: &mut Vm, data: &mut RangeData, copy: bool) -> Res<usize> {
    let fragment = create_fragment(vm)?;
    let start = boundary(vm, &data.start, data.start_offset);
    let end = boundary(vm, &data.end, data.end_offset);
    if start == end {
        return Ok(fragment);
    }
    if start.0 == end.0 && is_character_data(vm, start.0) {
        let piece = take_units(vm, start.0, start.1, end.1, copy)?;
        dom::insert(vm, fragment, piece, None)?;
        if !copy {
            collapse_to(vm, data, start)?;
        }
        return Ok(fragment);
    }
    let root = common_ancestor(vm, data);
    let collapse = (!copy).then(|| new_position(vm, root, start));
    fill_fragment(vm, fragment, root, start, end, copy)?;
    if let Some(position) = collapse {
        collapse_to(vm, data, position)?;
    }
    Ok(fragment)
}

/** The node and offset a range collapses to after its contents are extracted from `root`. */
fn new_position(vm: &mut Vm, root: usize, start: Boundary) -> Boundary {
    if start.0 == root {
        return start;
    }
    let child = inclusive_ancestors(vm, start.0)
        .into_iter()
        .find(|id| children(vm, root).contains(id));
    let index = child.map_or(0, |child| index_in(vm, root, child));
    (root, index + 1)
}

/** Move (or copy) the parts of `root`'s children that the boundary points select into `fragment`. */
fn fill_fragment(
    vm: &mut Vm,
    fragment: usize,
    root: usize,
    start: Boundary,
    end: Boundary,
    copy: bool,
) -> Res<()> {
    let first = partial_child(vm, root, start);
    let last = partial_child(vm, root, end);
    let contained = contained_children(vm, root, start, end);
    if let Some(node) = first {
        let part = start_part(vm, start, node, copy)?;
        dom::insert(vm, fragment, part, None)?;
    }
    for child in contained {
        let moved = if copy {
            dom::clone_node(vm, child, true)?
        } else {
            child
        };
        dom::insert(vm, fragment, moved, None)?;
    }
    if let Some(node) = last {
        let part = end_part(vm, end, node, copy)?;
        dom::insert(vm, fragment, part, None)?;
    }
    Ok(())
}

/** The child of `root` a boundary sits inside, when that child is only partly selected. */
fn partial_child(vm: &mut Vm, root: usize, point: Boundary) -> Option<usize> {
    if point.0 == root {
        return None;
    }
    let kids = children(vm, root);
    inclusive_ancestors(vm, point.0)
        .into_iter()
        .find(|id| kids.contains(id))
}

/** The children of `root` lying wholly between the two boundary points, in order. */
fn contained_children(vm: &mut Vm, root: usize, start: Boundary, end: Boundary) -> Vec<usize> {
    let mut out = Vec::new();
    for (index, child) in children(vm, root).into_iter().enumerate() {
        let after_start = compare(vm, (root, index), start) != Ordering::Less;
        let before_end = compare(vm, (root, index + 1), end) != Ordering::Greater;
        if after_start && before_end {
            out.push(child);
        }
    }
    out
}

/** The part of the partly selected `node` that starts at the range's start point. */
fn start_part(vm: &mut Vm, start: Boundary, node: usize, copy: bool) -> Res<usize> {
    if is_character_data(vm, node) {
        let length = node_length(vm, node);
        return take_units(vm, node, start.1, length, copy);
    }
    let copy_node = dom::clone_node(vm, node, false)?;
    let length = node_length(vm, node);
    let mut sub = sub_range(vm, (start.0, start.1), (node, length))?;
    let inner = extract_contents(vm, &mut sub, copy)?;
    dom::insert(vm, copy_node, inner, None)?;
    Ok(copy_node)
}

/** The part of the partly selected `node` that ends at the range's end point. */
fn end_part(vm: &mut Vm, end: Boundary, node: usize, copy: bool) -> Res<usize> {
    if is_character_data(vm, node) {
        return take_units(vm, node, 0, end.1, copy);
    }
    let copy_node = dom::clone_node(vm, node, false)?;
    let mut sub = sub_range(vm, (node, 0), (end.0, end.1))?;
    let inner = extract_contents(vm, &mut sub, copy)?;
    dom::insert(vm, copy_node, inner, None)?;
    Ok(copy_node)
}

/** A detached range over two boundary points, for extracting part of a partly selected node. */
fn sub_range(vm: &mut Vm, start: Boundary, end: Boundary) -> Res<RangeData> {
    Ok(RangeData {
        start: dom::wrap(vm, start.0)?,
        start_offset: start.1,
        end: dom::wrap(vm, end.0)?,
        end_offset: end.1,
    })
}

/**
 * A new character data node holding units `[from, to)` of `node`. Unless copying, those units
 * leave `node` through replaceData, which moves the live points inside them.
 */
fn take_units(vm: &mut Vm, node: usize, from: usize, to: usize, copy: bool) -> Res<usize> {
    let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
    let from = from.min(units.len());
    let to = to.clamp(from, units.len());
    let piece = String::from_utf16_lossy(&units[from..to]);
    let tag = vm.page.tag(node);
    if !copy {
        replace_data(vm, node, from, to - from, "")?;
    }
    vm.page
        .create(&tag, Some(piece))
        .map_err(|error| dom::dom_error(vm, error))
}

/** Set a range's start and end to one point. */
fn collapse_to(vm: &mut Vm, data: &mut RangeData, point: Boundary) -> Res<()> {
    let value = dom::wrap(vm, point.0)?;
    data.start = value.clone();
    data.start_offset = point.1;
    data.end = value;
    data.end_offset = point.1;
    Ok(())
}

/** A new empty document fragment. */
fn create_fragment(vm: &mut Vm) -> Res<usize> {
    vm.page
        .create(FRAGMENT, None)
        .map_err(|error| dom::dom_error(vm, error))
}
