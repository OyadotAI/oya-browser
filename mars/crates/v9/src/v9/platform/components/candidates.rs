/**
 * The index of connected, not-yet-upgraded custom element candidates, by name. A definition finds
 * its elements here instead of walking the whole document, so defining hundreds of elements over a
 * page's life costs the elements it upgrades, not hundreds of document walks.
 *
 * The index holds only connected elements that are not custom and not failed, keyed by the name a
 * definition would match: the tag for a valid custom name, else the `is` value. Insertion, `is`
 * writes and removal keep it current; the first definition builds it from the document as it stands.
 * Entries that went stale (an element removed without a pruning pass) are harmless: the upgrade
 * re-checks the element, and a definition takes its key out whole.
 */
use super::registry::valid_name;
use crate::v9::platform::HTML;
use crate::v9::platform::Page;
use crate::v9::vm::Vm;
use std::collections::{BTreeSet, HashMap};

/** The rank of a node the document walk does not reach; such nodes sort last. */
const UNRANKED: usize = usize::MAX;

/** The name a node is indexed under: its tag when that is a custom name, else its `is` value. */
fn key_of(page: &mut Page, node: usize) -> Option<String> {
    let tag = page.tag(node);
    if valid_name(&tag) {
        return Some(tag);
    }
    let is = page.attribute_value(node, "is")?;
    valid_name(&is).then_some(is)
}

/** Index one node that is known to be connected, unless it is already custom or failed. */
fn insert_connected(vm: &mut Vm, node: usize) {
    if vm.page.state.components.custom.contains_key(&node)
        || vm.page.state.components.failed.contains(&node)
    {
        return;
    }
    if let Some(key) = key_of(&mut vm.page, node) {
        vm.page
            .state
            .components
            .candidates
            .entry(key)
            .or_default()
            .insert(node);
    }
}

/** Index a node an insertion just connected; the index is built only once a definition exists. */
pub(super) fn track_connected(vm: &mut Vm, node: usize) {
    if vm.page.state.components.candidates_built {
        insert_connected(vm, node);
    }
}

/** Index a node whose connection is not known (a created root, or an `is` write): only a node
that is in the document is a candidate. */
pub(super) fn track(vm: &mut Vm, node: usize) {
    if vm.page.state.components.candidates_built && vm.page.connected(node) {
        insert_connected(vm, node);
    }
}

/** Drop a subtree that left the document from the index, unless it is connected again already. */
pub(super) fn forget_tree(vm: &mut Vm, root: usize) {
    if !vm.page.state.components.candidates_built
        || vm.page.state.components.candidates.is_empty()
        || vm.page.connected(root)
    {
        return;
    }
    let mut nodes = vec![root];
    nodes.extend(vm.page.all_descendants(root));
    for node in nodes {
        if let Some(key) = key_of(&mut vm.page, node)
            && let Some(set) = vm.page.state.components.candidates.get_mut(&key)
        {
            set.remove(&node);
        }
    }
}

/**
 * The candidates of a name, taken out of the index, in document order. The first call builds the
 * index from the whole document, the only full walk the registry makes.
 */
pub(super) fn take(vm: &mut Vm, name: &str) -> Vec<usize> {
    if !vm.page.state.components.candidates_built {
        build(vm);
    }
    let nodes = vm
        .page
        .state
        .components
        .candidates
        .remove(name)
        .unwrap_or_default();
    document_order(&mut vm.page, nodes)
}

/** Index every connected candidate in the document, shadow trees included. */
fn build(vm: &mut Vm) {
    for node in vm.page.all_descendants(HTML) {
        insert_connected(vm, node);
    }
    vm.page.state.components.candidates_built = true;
}

/**
 * Sort candidates into document order. Projected nodes order by their projection position, which
 * is the document's order. A candidate outside the projection (one in the head) sends the whole
 * set through one walk of the document, the only case that costs O(document) per definition.
 */
fn document_order(page: &mut Page, nodes: BTreeSet<usize>) -> Vec<usize> {
    let positions: Option<Vec<usize>> = nodes
        .iter()
        .map(|node| page.projected_position(*node))
        .collect();
    let mut ordered: Vec<usize> = nodes.iter().copied().collect();
    match positions {
        Some(positions) => {
            let mut keyed: Vec<(usize, usize)> = positions.into_iter().zip(ordered).collect();
            keyed.sort_unstable();
            keyed.into_iter().map(|(_, node)| node).collect()
        }
        None => {
            let rank: HashMap<usize, usize> = page
                .all_descendants(HTML)
                .into_iter()
                .enumerate()
                .map(|(position, node)| (node, position))
                .collect();
            ordered.sort_by_key(|node| rank.get(node).copied().unwrap_or(UNRANKED));
            ordered
        }
    }
}
