/**
 * The live Range registry: which Range objects are registered and, for each DOM node, the ranges
 * with a boundary point in it. A tree mutation reads only the ranges its nodes touch, not every
 * range the page holds. Each range remembers the two nodes it is linked under, so unlinking never
 * reads the heap. The serialized form lists each range with its nodes; a state written before the
 * index existed lists bare ids, and its node index is rebuilt from the heap once, on first use.
 */
use crate::v9::value::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/** The two containers of a registered range: its start node and its end node. */
pub(in crate::v9) type Nodes = (usize, usize);

/** The two wire forms: the current linked list, and the bare id list older builds wrote. */
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(in crate::v9) enum Stored {
    /** Each registered range with the two nodes it is linked under. */
    Linked(Vec<(ObjectId, Nodes)>),
    /** Bare range ids, from a state that predates the node index. */
    Plain(Vec<ObjectId>),
}

/**
 * Registered ranges and their node index. `indexed` is false only for a state loaded in the old
 * bare-id form, until the owner links every registered range from the heap.
 */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "Stored", into = "Stored")]
pub(in crate::v9) struct LiveRanges {
    /** Every registered range. */
    ids: BTreeSet<ObjectId>,
    /** The containers each indexed range is linked under. */
    nodes: BTreeMap<ObjectId, Nodes>,
    /** The indexed ranges with a boundary point in each node. */
    by_node: BTreeMap<usize, BTreeSet<ObjectId>>,
    /** Whether `nodes` and `by_node` describe every registered range. */
    indexed: bool,
}

impl Default for LiveRanges {
    /** A fresh registry: nothing registered, and the index is complete by construction. */
    fn default() -> Self {
        Self {
            ids: BTreeSet::new(),
            nodes: BTreeMap::new(),
            by_node: BTreeMap::new(),
            indexed: true,
        }
    }
}

impl From<Stored> for LiveRanges {
    /** Loads either wire form: linked ranges are indexed again as they come; bare ids wait for a rebuild. */
    fn from(stored: Stored) -> Self {
        match stored {
            Stored::Linked(linked) => {
                let mut live = Self::default();
                for (id, nodes) in linked {
                    live.place(id, nodes);
                }
                live
            }
            Stored::Plain(ids) => Self {
                ids: ids.into_iter().collect(),
                indexed: false,
                ..Self::default()
            },
        }
    }
}

impl From<LiveRanges> for Stored {
    /** The serialized form: linked ranges when the index is complete, bare ids while it is not. */
    fn from(live: LiveRanges) -> Self {
        if live.indexed {
            Stored::Linked(live.nodes.into_iter().collect())
        } else {
            Stored::Plain(live.ids.into_iter().collect())
        }
    }
}

impl LiveRanges {
    /** Whether no range is registered, so no mutation has anything to move. */
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /** Whether a range object is registered. */
    pub fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }

    /** Every registered range id, copied so the caller can read the heap while it walks them. */
    pub fn registered(&self) -> Vec<ObjectId> {
        self.ids.iter().copied().collect()
    }

    /** Whether the node index covers every registered range. */
    pub fn is_indexed(&self) -> bool {
        self.indexed
    }

    /** Marks the node index complete once the owner has linked every registered range. */
    pub fn mark_indexed(&mut self) {
        self.indexed = true;
    }

    /** The ranges with a boundary point in a node, copied out; O(ranges at that node). */
    pub fn ranges_at(&self, node: usize) -> Vec<ObjectId> {
        self.by_node
            .get(&node)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    /**
     * Registers a range, or moves it, under its containers. Relinking under the same containers is
     * harmless, so the owner need not compare first.
     */
    pub fn place(&mut self, id: ObjectId, nodes: Nodes) {
        self.ids.insert(id);
        if let Some(old) = self.nodes.insert(id, nodes) {
            self.unlink(id, old);
        }
        self.link(id, nodes);
    }

    /** Drops every registered range the predicate rejects, from the ids and the node index. */
    pub fn retain(&mut self, keep: impl Fn(&ObjectId) -> bool) {
        let dropped: Vec<ObjectId> = self.ids.iter().copied().filter(|id| !keep(id)).collect();
        for id in dropped {
            self.ids.remove(&id);
            if let Some(old) = self.nodes.remove(&id) {
                self.unlink(id, old);
            }
        }
    }

    /**
     * A node took a new identity: the ranges linked under the old one move to the new one. Only
     * the ranges of that node are touched, so the cost is those ranges.
     */
    pub fn renumber(&mut self, old: usize, new: usize) {
        let Some(ids) = self.by_node.remove(&old) else {
            return;
        };
        for id in &ids {
            if let Some((start, end)) = self.nodes.get_mut(id) {
                for node in [start, end] {
                    if *node == old {
                        *node = new;
                    }
                }
            }
        }
        self.by_node.entry(new).or_default().extend(ids);
    }

    /** Adds a range to the node index under both of its containers. */
    fn link(&mut self, id: ObjectId, (start, end): Nodes) {
        self.by_node.entry(start).or_default().insert(id);
        self.by_node.entry(end).or_default().insert(id);
    }

    /** Removes a range from the node index under both of its containers. */
    fn unlink(&mut self, id: ObjectId, (start, end): Nodes) {
        self.unlink_node(id, start);
        self.unlink_node(id, end);
    }

    /** Removes one range from one node's set, dropping the set once it is empty. */
    fn unlink_node(&mut self, id: ObjectId, node: usize) {
        if let Some(ids) = self.by_node.get_mut(&node) {
            ids.remove(&id);
            if ids.is_empty() {
                self.by_node.remove(&node);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the live range registry's index rules.
    use super::*;

    /** A range is listed under both of its containers, and under no other node. */
    #[test]
    fn place_links_a_range_under_both_containers() {
        let mut live = LiveRanges::default();
        live.place(1, (4, 6));
        assert_eq!(live.ranges_at(4), vec![1]);
        assert_eq!(live.ranges_at(6), vec![1]);
        assert!(live.ranges_at(5).is_empty());
    }

    /** Moving a range drops its old containers from the index. */
    #[test]
    fn place_moves_a_range_to_its_new_containers() {
        let mut live = LiveRanges::default();
        live.place(1, (4, 4));
        live.place(1, (7, 4));
        assert_eq!(live.ranges_at(7), vec![1]);
        assert_eq!(live.ranges_at(4), vec![1]);
        live.place(1, (7, 8));
        assert!(live.ranges_at(4).is_empty());
    }

    /** A collected range leaves both the id registry and the node index. */
    #[test]
    fn retain_drops_a_collected_range_from_the_index() {
        let mut live = LiveRanges::default();
        live.place(1, (3, 3));
        live.place(2, (3, 5));
        live.retain(|id| *id != 1);
        assert!(!live.contains(1));
        assert_eq!(live.ranges_at(3), vec![2]);
    }

    /** A renumbering moves exactly the ranges of the old node, under the new id. */
    #[test]
    fn renumber_moves_the_ranges_of_the_old_node() {
        let mut live = LiveRanges::default();
        live.place(1, (3, 8));
        live.place(2, (5, 5));
        live.renumber(3, 9);
        assert!(live.ranges_at(3).is_empty());
        assert_eq!(live.ranges_at(9), vec![1]);
        assert_eq!(live.ranges_at(8), vec![1]);
        assert_eq!(live.ranges_at(5), vec![2]);
    }

    /** The current form keeps each range's containers, so it reloads already indexed. */
    #[test]
    fn linked_form_round_trips_indexed() {
        let mut live = LiveRanges::default();
        live.place(3, (1, 2));
        let stored: Stored = live.into();
        let loaded = LiveRanges::from(stored);
        assert!(loaded.is_indexed());
        assert_eq!(loaded.ranges_at(2), vec![3]);
    }

    /** A state written as bare ids loads unindexed, so the owner knows to rebuild it. */
    #[test]
    fn bare_id_form_loads_unindexed() {
        let loaded = LiveRanges::from(Stored::Plain(vec![3]));
        assert!(loaded.contains(3));
        assert!(!loaded.is_indexed());
    }
}
