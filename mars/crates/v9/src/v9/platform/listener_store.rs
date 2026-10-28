/**
 * The event listener registry, indexed by target, type, capture and callback object. An add or a
 * remove finds its twin by key in O(log n); a dispatch reads only the registrations for the target,
 * type and phase it fires at, so its cost is those listeners and no others. The serialized form is
 * the plain registration list older builds wrote, so existing state still loads.
 */
use super::{Listener, Target};
use crate::v9::value::ObjectId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/** The lowest registration id: the smallest key a range scan over one callback starts from. */
const FIRST_ID: u32 = 0;
/** The highest registration id: closes a range scan over one callback's registrations. */
const LAST_ID: u32 = u32::MAX;
/** The lowest callback object id: starts a scan over every callback of one type. */
const FIRST_CALLBACK: ObjectId = 0;

/**
 * A registration's place in the index: target, type, capture, callback object, then registration
 * id. The id last keeps two registrations of one callback apart when a renumbering re-keys them.
 */
pub(in crate::v9) type ListenerKey = (Target, String, bool, ObjectId, u32);

/** Build the key of a registration from its parts. */
fn key(target: Target, kind: &str, capture: bool, callback: ObjectId, id: u32) -> ListenerKey {
    (target, kind.to_string(), capture, callback, id)
}

/** The key of a registration; None when its callback is not an object, which cannot be keyed. */
fn key_of(listener: &Listener) -> Option<ListenerKey> {
    listener.callback.object().map(|callback| {
        key(
            listener.target,
            &listener.kind,
            listener.capture,
            callback,
            listener.id,
        )
    })
}

/** Every event listener the page registered, keyed for O(log n) lookup and dispatch. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(from = "Vec<Listener>", into = "Vec<Listener>")]
pub(in crate::v9) struct ListenerStore {
    /** Every registration by its key. */
    entries: BTreeMap<ListenerKey, Listener>,
    /** The serialized list held a record the index could not keep: a duplicate of another
    registration or one with a non-object callback. Loading drops it, so state validation
    reads this flag to refuse the whole state rather than run a silently repaired copy. */
    #[serde(skip)]
    malformed: bool,
}

impl From<Vec<Listener>> for ListenerStore {
    /** Rebuilds the index from the serialized list, in whatever order it was written. */
    fn from(listeners: Vec<Listener>) -> Self {
        let mut store = Self::default();
        let written = listeners.len();
        for listener in listeners {
            store.insert(listener);
        }
        store.malformed = store.len() != written;
        store
    }
}

impl From<ListenerStore> for Vec<Listener> {
    /** The serialized form: every registration in registration (id) order. */
    fn from(store: ListenerStore) -> Self {
        let mut listeners: Vec<Listener> = store.entries.into_values().collect();
        listeners.sort_by_key(|listener| listener.id);
        listeners
    }
}

impl ListenerStore {
    /** Every registration, in no particular order: the GC roots and the page-wide checks read these. */
    pub fn iter(&self) -> impl Iterator<Item = &Listener> {
        self.entries.values()
    }

    /** Whether the loaded list had records the index dropped (see the `malformed` field). */
    pub fn malformed(&self) -> bool {
        self.malformed
    }

    /** How many registrations exist; the limit on listeners counts these. */
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /** Whether a registration with this key still exists; a listener removed mid-dispatch does not run. */
    pub fn contains(&self, key: &ListenerKey) -> bool {
        self.entries.contains_key(key)
    }

    /** Add a registration. One whose callback is not an object has no key, so it is not kept. */
    pub fn insert(&mut self, listener: Listener) {
        if let Some(key) = key_of(&listener) {
            self.entries.insert(key, listener);
        }
    }

    /** The key of the registration with this target, type, capture and callback, if one exists. */
    pub fn find(
        &self,
        target: Target,
        kind: &str,
        capture: bool,
        callback: ObjectId,
    ) -> Option<ListenerKey> {
        let first = key(target, kind, capture, callback, FIRST_ID);
        let last = key(target, kind, capture, callback, LAST_ID);
        self.entries
            .range(first..=last)
            .next()
            .map(|(key, _)| key.clone())
    }

    /** Remove one registration by key, returning it. */
    pub fn remove(&mut self, key: &ListenerKey) -> Option<Listener> {
        self.entries.remove(key)
    }

    /**
     * The registrations a dispatch fires at one target, type and phase, in registration order. The
     * scan visits only that bucket, so a dispatch costs the listeners it fires plus O(log n).
     */
    pub fn firing(&self, target: Target, kind: &str, capture: bool) -> Vec<(ListenerKey, Listener)> {
        let first = key(target, kind, capture, FIRST_CALLBACK, FIRST_ID);
        let mut firing: Vec<(ListenerKey, Listener)> = self
            .entries
            .range(first..)
            .take_while(|(key, _)| key.0 == target && key.1 == kind && key.2 == capture)
            .map(|(key, listener)| (key.clone(), listener.clone()))
            .collect();
        firing.sort_by_key(|(_, listener)| listener.id);
        firing
    }

    /** A node took a new identity in a renumbering: its registrations move to the new node. */
    pub fn renumber_node(&mut self, old: usize, new: usize) {
        let first = key(Target::Node(old), "", false, FIRST_CALLBACK, FIRST_ID);
        let moved: Vec<ListenerKey> = self
            .entries
            .range(first..)
            .take_while(|(key, _)| key.0 == Target::Node(old))
            .map(|(key, _)| key.clone())
            .collect();
        for old_key in moved {
            if let Some(mut listener) = self.entries.remove(&old_key) {
                listener.target = Target::Node(new);
                self.insert(listener);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the listener registry's index rules.
    use super::*;
    use crate::v9::value::JsValue;

    /** A registration on a target with the given callback object and id. */
    fn listener(target: Target, kind: &str, capture: bool, callback: ObjectId, id: u32) -> Listener {
        Listener {
            id,
            target,
            kind: kind.into(),
            callback: JsValue::Object(callback),
            capture,
            once: false,
            passive: false,
            signal: None,
        }
    }

    /** A find returns the key of the one registration with the same target, type, capture and callback. */
    #[test]
    fn find_matches_on_target_type_capture_and_callback() {
        let mut store = ListenerStore::default();
        store.insert(listener(Target::Window, "load", false, 7, 1));
        assert!(store.find(Target::Window, "load", false, 7).is_some());
        assert!(store.find(Target::Window, "load", true, 7).is_none());
        assert!(store.find(Target::Window, "load", false, 8).is_none());
        assert!(store.find(Target::Document, "load", false, 7).is_none());
    }

    /** A dispatch fires only the registrations of its target, type and phase, in registration order. */
    #[test]
    fn firing_returns_only_the_bucket_in_registration_order() {
        let mut store = ListenerStore::default();
        store.insert(listener(Target::Window, "click", false, 9, 3));
        store.insert(listener(Target::Window, "click", false, 4, 1));
        store.insert(listener(Target::Window, "click", true, 5, 2));
        store.insert(listener(Target::Document, "click", false, 6, 4));
        let ids: Vec<u32> = store
            .firing(Target::Window, "click", false)
            .into_iter()
            .map(|(_, listener)| listener.id)
            .collect();
        assert_eq!(ids, vec![1, 3]);
    }

    /** A callback that is not an object cannot be keyed, so it is not kept. */
    #[test]
    fn non_object_callback_is_not_kept() {
        let mut store = ListenerStore::default();
        let mut bad = listener(Target::Window, "load", false, 1, 1);
        bad.callback = JsValue::Number(1.0);
        store.insert(bad);
        assert_eq!(store.len(), 0);
    }

    /** Removing by key drops exactly that registration. */
    #[test]
    fn remove_drops_only_the_keyed_registration() {
        let mut store = ListenerStore::default();
        store.insert(listener(Target::Window, "load", false, 7, 1));
        store.insert(listener(Target::Window, "load", false, 8, 2));
        let key = store.find(Target::Window, "load", false, 7).unwrap();
        assert!(store.remove(&key).is_some());
        assert!(!store.contains(&key));
        assert_eq!(store.len(), 1);
    }

    /** A renumbering moves the registrations of the old node to the new one and leaves others alone. */
    #[test]
    fn renumber_node_moves_registrations_to_the_new_node() {
        let mut store = ListenerStore::default();
        store.insert(listener(Target::Node(3), "click", false, 7, 1));
        store.insert(listener(Target::Node(4), "click", false, 7, 2));
        store.renumber_node(3, 9);
        assert!(store.find(Target::Node(3), "click", false, 7).is_none());
        assert!(store.find(Target::Node(9), "click", false, 7).is_some());
        assert!(store.find(Target::Node(4), "click", false, 7).is_some());
    }

    /** The serialized form is the plain registration list, and it loads back into the index. */
    #[test]
    fn serialized_form_is_the_plain_list_and_reloads_indexed() {
        let mut store = ListenerStore::default();
        store.insert(listener(Target::Window, "load", false, 7, 1));
        let list: Vec<Listener> = store.clone().into();
        assert_eq!(list.len(), 1);
        let reloaded = ListenerStore::from(list);
        assert!(reloaded.find(Target::Window, "load", false, 7).is_some());
    }
}
