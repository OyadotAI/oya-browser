/**
 * The event listener registry, indexed by target, type, capture and callback object. An add or a
 * remove finds its twin by key in O(log n); a dispatch reads only the registrations for the target,
 * type and phase it fires at, so its cost is those listeners and no others. The serialized form is
 * the plain registration list older builds wrote, so existing state still loads.
 */
mod budget;
mod diagnostic;
mod wire;
use super::{Listener, Target};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::ObjectId;
use diagnostic::Stage;
use std::collections::{BTreeMap, BTreeSet};

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
#[derive(Debug, Default)]
pub(in crate::v9) struct ListenerStore {
    /** Every registration by its key. */
    entries: BTreeMap<ListenerKey, Listener>,
    /** The serialized list held a record the index could not keep: a duplicate of another
    registration or one with a non-object callback. Loading drops it, so state validation
    reads this flag to refuse the whole state rather than run a silently repaired copy. */
    malformed: bool,
    /** Cached accounted bytes; private and always recomputed when restoring or cloning state. */
    bytes: usize,
}

impl Clone for ListenerStore {
    /** String clones may shrink their spare capacity, so copied usage is measured again. */
    fn clone(&self) -> Self {
        let entries = self.entries.clone();
        let bytes = entries.iter().fold(0usize, |used, (key, listener)| {
            used.saturating_add(record_bytes(key, listener))
        });
        Self {
            entries,
            malformed: self.malformed,
            bytes,
        }
    }
}

/** Stored capacities include both independently owned copies of the event kind. */
fn record_bytes(key: &ListenerKey, listener: &Listener) -> usize {
    budget::record(&listener.kind, listener.kind.capacity(), key.1.capacity())
}

/** Fixed metadata accounting permits only arena identities, never inline string/bigint payloads. */
fn validate_references(listener: &Listener) -> EngineResult<()> {
    if listener.callback.object().is_none()
        || listener
            .signal
            .as_ref()
            .is_some_and(|signal| signal.object().is_none())
    {
        return Err(EngineError::invalid("Invalid event listener references."));
    }
    Ok(())
}

impl From<Vec<Listener>> for ListenerStore {
    /** Rebuilds the index from the serialized list, in whatever order it was written. */
    fn from(listeners: Vec<Listener>) -> Self {
        let mut store = Self::default();
        let written = listeners.len();
        for listener in listeners {
            if store.insert(listener).is_err() {
                store.malformed = true;
            }
        }
        store.malformed |= store.len() != written;
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

    /** Collection removes registrations whose target and callback cycle has no external owner. */
    pub fn retain(&mut self, mut keep: impl FnMut(&Listener) -> bool) {
        let bytes = &mut self.bytes;
        self.entries.retain(|key, listener| {
            let retained = keep(listener);
            if !retained {
                *bytes = bytes.saturating_sub(record_bytes(key, listener));
            }
            retained
        });
    }

    /** Whether the loaded list had records the index dropped (see the `malformed` field). */
    pub fn malformed(&self) -> bool {
        self.malformed
    }

    /** Registration count bounds traversal work; storage admission uses actual accounted bytes. */
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /** Whether a registration with this key still exists; a listener removed mid-dispatch does not run. */
    pub fn contains(&self, key: &ListenerKey) -> bool {
        self.entries.contains_key(key)
    }

    /** Admit a prospective registration before copying its event kind into the index. */
    pub fn admit(&self, kind: &String) -> EngineResult<()> {
        self.admit_at(kind, Stage::Add)
    }

    /** The wire reader labels admission separately from an actual script registration. */
    fn admit_at(&self, kind: &String, stage: Stage) -> EngineResult<()> {
        let bytes = self
            .bytes
            .saturating_add(budget::record(kind, kind.capacity(), kind.len()));
        self.check_storage(bytes, stage, Some(kind))
    }

    /** Only a failed budget check scans native counts; no author getters or event names are read. */
    fn check_storage(&self, bytes: usize, stage: Stage, kind: Option<&String>) -> EngineResult<()> {
        budget::validate(bytes).map_err(|error| diagnostic::annotate(self, error, stage, kind))
    }

    /** Full and heap-less restored states enforce identical storage and identity constraints. */
    pub fn validate(&self, next: u32) -> EngineResult<()> {
        self.check_storage(self.bytes, Stage::Validate, None)?;
        let mut ids = BTreeSet::new();
        let mut bytes = 0usize;
        for (key, listener) in &self.entries {
            validate_references(listener)?;
            bytes = bytes.saturating_add(record_bytes(key, listener));
            if listener.id >= next || !ids.insert(listener.id) {
                return Err(EngineError::invalid("Invalid event listener identities."));
            }
        }
        if self.malformed || bytes != self.bytes {
            return Err(EngineError::invalid("Invalid event listener storage."));
        }
        Ok(())
    }

    /** Invalid callback identities and excess storage fail before the index is mutated. */
    pub fn insert(&mut self, listener: Listener) -> EngineResult<()> {
        self.insert_at(listener, Stage::Add)
    }

    /** Preserve the originating operation when the allocated index key reaches the budget. */
    fn insert_at(&mut self, listener: Listener, stage: Stage) -> EngineResult<()> {
        validate_references(&listener)?;
        self.admit_at(&listener.kind, stage)?;
        let key = key_of(&listener)
            .ok_or_else(|| EngineError::invalid("Invalid event listener callback."))?;
        if self.entries.contains_key(&key) {
            return Err(EngineError::invalid("Duplicate event listener identity."));
        }
        let bytes = self.bytes.saturating_add(record_bytes(&key, &listener));
        self.check_storage(bytes, stage, Some(&listener.kind))?;
        self.entries.insert(key, listener);
        self.bytes = bytes;
        Ok(())
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
        let (key, listener) = self.entries.remove_entry(key)?;
        self.bytes = self.bytes.saturating_sub(record_bytes(&key, &listener));
        Some(listener)
    }

    /** Charge the exact dispatch bucket and its copied names before allocating snapshot records. */
    pub fn firing_work(&self, target: Target, kind: &str, capture: bool) -> usize {
        let first = key(target, kind, capture, FIRST_CALLBACK, FIRST_ID);
        self.entries
            .range(first..)
            .take_while(|(key, _)| key.0 == target && key.1 == kind && key.2 == capture)
            .fold(1usize, |work, (_, listener)| {
                work.saturating_add(listener.kind.len().saturating_mul(2).saturating_add(1))
            })
    }

    /**
     * The registrations a dispatch fires at one target, type and phase, in registration order. The
     * scan visits only that bucket, so a dispatch costs the listeners it fires plus O(log n).
     */
    pub fn firing(
        &self,
        target: Target,
        kind: &str,
        capture: bool,
    ) -> Vec<(ListenerKey, Listener)> {
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
            if let Some((mut key, mut listener)) = self.entries.remove_entry(&old_key) {
                // Preserve the strings and their capacities; changing an identity costs no bytes.
                listener.target = Target::Node(new);
                key.0 = Target::Node(new);
                if self.entries.insert(key, listener).is_some() {
                    self.malformed = true;
                }
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
    fn listener(
        target: Target,
        kind: &str,
        capture: bool,
        callback: ObjectId,
        id: u32,
    ) -> Listener {
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
        store
            .insert(listener(Target::Window, "load", false, 7, 1))
            .unwrap();
        assert!(store.find(Target::Window, "load", false, 7).is_some());
        assert!(store.find(Target::Window, "load", true, 7).is_none());
        assert!(store.find(Target::Window, "load", false, 8).is_none());
        assert!(store.find(Target::Document, "load", false, 7).is_none());
    }

    /** A dispatch fires only the registrations of its target, type and phase, in registration order. */
    #[test]
    fn firing_returns_only_the_bucket_in_registration_order() {
        let mut store = ListenerStore::default();
        store
            .insert(listener(Target::Window, "click", false, 9, 3))
            .unwrap();
        store
            .insert(listener(Target::Window, "click", false, 4, 1))
            .unwrap();
        store
            .insert(listener(Target::Window, "click", true, 5, 2))
            .unwrap();
        store
            .insert(listener(Target::Document, "click", false, 6, 4))
            .unwrap();
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
        assert!(store.insert(bad).is_err());
        assert_eq!(store.len(), 0);
    }

    /** Removing by key drops exactly that registration. */
    #[test]
    fn remove_drops_only_the_keyed_registration() {
        let mut store = ListenerStore::default();
        store
            .insert(listener(Target::Window, "load", false, 7, 1))
            .unwrap();
        store
            .insert(listener(Target::Window, "load", false, 8, 2))
            .unwrap();
        let key = store.find(Target::Window, "load", false, 7).unwrap();
        assert!(store.remove(&key).is_some());
        assert!(!store.contains(&key));
        assert_eq!(store.len(), 1);
    }

    /** A renumbering moves the registrations of the old node to the new one and leaves others alone. */
    #[test]
    fn renumber_node_moves_registrations_to_the_new_node() {
        let mut store = ListenerStore::default();
        store
            .insert(listener(Target::Node(3), "click", false, 7, 1))
            .unwrap();
        store
            .insert(listener(Target::Node(4), "click", false, 7, 2))
            .unwrap();
        store.renumber_node(3, 9);
        assert!(store.find(Target::Node(3), "click", false, 7).is_none());
        assert!(store.find(Target::Node(9), "click", false, 7).is_some());
        assert!(store.find(Target::Node(4), "click", false, 7).is_some());
    }

    /** The serialized form is the plain registration list, and it loads back into the index. */
    #[test]
    fn serialized_form_is_the_plain_list_and_reloads_indexed() {
        let mut store = ListenerStore::default();
        store
            .insert(listener(Target::Window, "load", false, 7, 1))
            .unwrap();
        let list: Vec<Listener> = store.clone().into();
        assert_eq!(list.len(), 1);
        let reloaded = ListenerStore::from(list);
        assert!(reloaded.find(Target::Window, "load", false, 7).is_some());
    }

    /** Both native string copies and worst-case JSON expansion count before insertion. */
    #[test]
    fn long_event_kinds_exhaust_bytes_before_many_records() {
        let mut store = ListenerStore::default();
        let kind = "x".repeat(1024 * 1024);
        store
            .insert(listener(Target::Window, &kind, false, 1, 1))
            .unwrap();
        store
            .insert(listener(Target::Window, &kind, false, 2, 2))
            .unwrap();
        let error = store
            .insert(listener(Target::Window, &kind, false, 3, 3))
            .unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(
            error.message.contains("[add n=2 used="),
            "{}",
            error.message
        );
        assert!(
            error.message.contains("kind=1048576/1048576"),
            "{}",
            error.message
        );
        assert!(
            error.message.contains("W/D/N/O=2/0/0/0"),
            "{}",
            error.message
        );
        assert!(error.message.len() <= 220);
        assert!(!error.message.contains(&kind));
        assert_eq!(store.len(), 2);
        store.validate(3).unwrap();
    }

    /** Removal, pruning and rekeying preserve cached usage, including spare string capacity. */
    #[test]
    fn lifecycle_mutations_release_accounted_storage() {
        let mut store = ListenerStore::default();
        let kind = "x".repeat(1024 * 1024);
        store
            .insert(listener(Target::Node(1), &kind, false, 1, 1))
            .unwrap();
        store
            .insert(listener(Target::Node(2), &kind, false, 2, 2))
            .unwrap();
        let key = store.find(Target::Node(1), &kind, false, 1).unwrap();
        store.remove(&key).unwrap();
        store
            .insert(listener(Target::Node(3), &kind, false, 3, 3))
            .unwrap();
        store.retain(|listener| listener.id != 2);
        store
            .insert(listener(Target::Node(4), &kind, false, 4, 4))
            .unwrap();
        store.renumber_node(3, 9);
        store.validate(5).unwrap();
        store.clone().validate(5).unwrap();
        let wire = serde_json::to_vec(&store).unwrap();
        let restored: ListenerStore = serde_json::from_slice(&wire).unwrap();
        restored.validate(5).unwrap();
    }

    /** Decoding refuses excess storage before constructing an unbounded registration vector. */
    #[test]
    fn restoration_rejects_excess_bytes_and_duplicate_identities() {
        let kind = "x".repeat(1024 * 1024);
        let entries: Vec<_> = (1..=3)
            .map(|id| listener(Target::Window, &kind, false, id, id))
            .collect();
        let error = serde_json::from_slice::<ListenerStore>(&serde_json::to_vec(&entries).unwrap())
            .unwrap_err();
        assert!(error.to_string().contains("[restore n=2 used="), "{error}");
        let duplicate = vec![
            listener(Target::Window, "x", false, 1, 1),
            listener(Target::Window, "y", false, 2, 1),
        ];
        assert!(
            serde_json::from_slice::<ListenerStore>(&serde_json::to_vec(&duplicate).unwrap())
                .is_err()
        );
    }

    /** Summary restoration cannot smuggle unaccounted inline values in the signal reference. */
    #[test]
    fn signal_reference_rejects_inline_payloads() {
        let mut bad = listener(Target::Window, "x", false, 1, 1);
        bad.signal = Some(JsValue::String("not an arena identity".into()));
        let mut store = ListenerStore::default();
        assert!(store.insert(bad.clone()).is_err());
        let encoded = serde_json::to_vec(&vec![bad]).unwrap();
        assert!(serde_json::from_slice::<ListenerStore>(&encoded).is_err());
    }

    /** Invariant validation uses its own label and cannot expose event text through diagnostics. */
    #[test]
    fn storage_validation_diagnostic_identifies_native_stage_and_targets() {
        let mut store = ListenerStore::default();
        store
            .insert(listener(
                Target::Document,
                "private-event-name",
                false,
                1,
                1,
            ))
            .unwrap();
        store.bytes = budget::STORAGE_BYTES;
        let error = store.validate(2).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(
            error.message.contains("[validate n=1 used="),
            "{}",
            error.message
        );
        assert!(
            error.message.contains("kind=0/0 W/D/N/O=0/1/0/0"),
            "{}",
            error.message
        );
        assert!(!error.message.contains("private-event-name"));
        assert!(error.message.len() <= 220);
    }
}
