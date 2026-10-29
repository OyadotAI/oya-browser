/** Weak interning retains shared UTF-16 allocations only while a live string uses them. */
use super::{EngineError, EngineResult, JsString};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/** Bound metadata independently of the lifetime and length of shared strings. */
const MAX_ENTRIES: usize = 16_384;

/** The process-wide shared string table contains no strong reference to string data. */
static TABLE: OnceLock<Mutex<Table>> = OnceLock::new();

/** Find a live canonical shared allocation without reserving or creating new backing. */
pub(in crate::v9) fn find(value: &JsString) -> Option<JsString> {
    if value.is_shared() {
        return Some(value.clone());
    }
    let units = value.flat_units()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    units.hash(&mut hasher);
    let table = TABLE
        .get()?
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let found = table
        .buckets
        .get(&hasher.finish())?
        .iter()
        .filter_map(Weak::upgrade)
        .filter(|item| item.units.as_ref() == units)
        .max_by_key(|item| item.interned.load(std::sync::atomic::Ordering::Acquire))?;
    let one_byte = found.one_byte;
    Some(JsString(super::Storage::Shared(found), one_byte))
}

/** Reuse or promote an existing shared canonical cell without allocating a replacement. */
pub(in crate::v9) fn find_internalized(value: &JsString) -> Option<JsString> {
    use std::sync::atomic::Ordering;
    let units = value.flat_units()?;
    let one_byte = units.iter().all(|unit| *unit <= super::LATIN1_MAX_UNIT);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    units.hash(&mut hasher);
    let table = TABLE
        .get()?
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let bucket = table.buckets.get(&hasher.finish())?;
    let found = bucket
        .iter()
        .filter_map(Weak::upgrade)
        .find(|item| item.units.as_ref() == units && item.interned.load(Ordering::Acquire))
        .or_else(|| {
            bucket.iter().filter_map(Weak::upgrade).find(|item| {
                item.units.as_ref() == units
                    && item.one_byte == one_byte
                    && super::Storage::Shared(item.clone()).can_intern_in_place()
            })
        })?;
    found.interned.store(true, Ordering::Release);
    let encoding = found.one_byte;
    Some(JsString(super::Storage::Shared(found), encoding))
}

/** Locate a live noncanonical two-byte shared cell without changing its representation. */
pub(in crate::v9) fn find_noncanonical_two_byte(value: &JsString) -> Option<JsString> {
    use std::sync::atomic::Ordering;
    let units = value.flat_units()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    units.hash(&mut hasher);
    let table = TABLE
        .get()?
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let found = table
        .buckets
        .get(&hasher.finish())?
        .iter()
        .filter_map(Weak::upgrade)
        .find(|item| {
            item.units.as_ref() == units && !item.one_byte && !item.interned.load(Ordering::Acquire)
        })?;
    Some(JsString(super::Storage::Shared(found), false))
}

/** Count weak entries without scanning the table for every distinct literal. */
#[derive(Default)]
struct Table {
    buckets: HashMap<u64, Vec<Weak<super::storage::FlatData>>>,
    count: usize,
}

/** Drop dead weak metadata without retaining shared string allocations. */
pub(in crate::v9) fn collect() {
    if let Some(table) = TABLE.get() {
        let mut table = table.lock().unwrap_or_else(|error| error.into_inner());
        table.buckets.retain(|_, bucket| {
            bucket.retain(|entry| entry.strong_count() != 0);
            !bucket.is_empty()
        });
        table.count = table.buckets.values().map(Vec::len).sum();
    }
}

/** Intern immutable code units and preserve their actual shared allocation and provenance. */
pub(in crate::v9) fn intern(value: &JsString) -> EngineResult<JsString> {
    if value.is_shared() {
        return Ok(value.clone());
    }
    let units = value.as_units();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    units.hash(&mut hasher);
    let hash = hasher.finish();
    let mut table = TABLE
        .get_or_init(|| Mutex::new(Table::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(found) = table.buckets.get(&hash).and_then(|bucket| {
        bucket
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|item| item.units.as_ref() == units)
            .max_by_key(|item| item.interned.load(std::sync::atomic::Ordering::Acquire))
    }) {
        let one_byte = found.one_byte;
        return Ok(JsString(super::Storage::Shared(found), one_byte));
    }
    if table.count >= MAX_ENTRIES {
        table.buckets.retain(|_, bucket| {
            bucket.retain(|entry| entry.strong_count() != 0);
            !bucket.is_empty()
        });
        table.count = table.buckets.values().map(Vec::len).sum();
        if table.count >= MAX_ENTRIES {
            return Err(EngineError::limit("shared interned strings", MAX_ENTRIES));
        }
    }
    let shared = match &value.0 {
        super::Storage::Shared(cell) => cell.clone(),
        _ => Arc::new(super::storage::FlatData {
            units: Arc::from(units),
            one_byte: value.is_latin1(),
            external: super::storage::ExternalSlot::default(),
            old: std::sync::atomic::AtomicBool::new(true),
            survivals: std::sync::atomic::AtomicU8::new(0),
            id: super::Storage::next_id(),
            interned: std::sync::atomic::AtomicBool::new(false),
            read_only: false,
            forward: std::sync::OnceLock::new(),
        }),
    };
    table
        .buckets
        .entry(hash)
        .or_default()
        .push(Arc::downgrade(&shared));
    table.count += 1;
    Ok(JsString(super::Storage::Shared(shared), value.is_latin1()))
}

/** Restore a noncanonical two-byte shared cell without merging it with a one-byte canonical peer. */
pub(in crate::v9) fn intern_noncanonical_two_byte(value: &JsString) -> EngineResult<JsString> {
    use std::sync::atomic::Ordering;
    let units = value.as_units();
    if units.iter().any(|unit| *unit > super::LATIN1_MAX_UNIT) {
        return Err(EngineError::invalid("invalid noncanonical shared string"));
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    units.hash(&mut hasher);
    let hash = hasher.finish();
    let mut table = TABLE
        .get_or_init(|| Mutex::new(Table::default()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(found) = table.buckets.get(&hash).and_then(|bucket| {
        bucket.iter().filter_map(Weak::upgrade).find(|item| {
            item.units.as_ref() == units && !item.one_byte && !item.interned.load(Ordering::Acquire)
        })
    }) {
        return Ok(JsString(super::Storage::Shared(found), false));
    }
    if table.count >= MAX_ENTRIES {
        table.buckets.retain(|_, bucket| {
            bucket.retain(|entry| entry.strong_count() != 0);
            !bucket.is_empty()
        });
        table.count = table.buckets.values().map(Vec::len).sum();
        if table.count >= MAX_ENTRIES {
            return Err(EngineError::limit("shared interned strings", MAX_ENTRIES));
        }
    }
    let shared = Arc::new(super::storage::FlatData {
        units: Arc::from(units),
        one_byte: false,
        external: super::storage::ExternalSlot::default(),
        old: std::sync::atomic::AtomicBool::new(true),
        survivals: std::sync::atomic::AtomicU8::new(0),
        id: super::Storage::next_id(),
        interned: std::sync::atomic::AtomicBool::new(false),
        read_only: false,
        forward: std::sync::OnceLock::new(),
    });
    table
        .buckets
        .entry(hash)
        .or_default()
        .push(Arc::downgrade(&shared));
    table.count += 1;
    Ok(JsString(super::Storage::Shared(shared), false))
}

/** Internalized shared strings normalize encoding and retain a separate real canonical cell. */
pub(in crate::v9) fn intern_internalized(value: &JsString) -> EngineResult<JsString> {
    use std::sync::atomic::Ordering;
    let units = value.as_units();
    let one_byte = units.iter().all(|unit| *unit <= super::LATIN1_MAX_UNIT);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    units.hash(&mut hasher);
    let hash = hasher.finish();
    let mut table = TABLE
        .get_or_init(|| Mutex::new(Table::default()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(found) = table.buckets.get(&hash).and_then(|bucket| {
        bucket
            .iter()
            .filter_map(Weak::upgrade)
            .find(|item| item.units.as_ref() == units && item.interned.load(Ordering::Acquire))
    }) {
        let encoding = found.one_byte;
        return Ok(JsString(super::Storage::Shared(found), encoding));
    }
    if let Some(found) = table.buckets.get(&hash).and_then(|bucket| {
        bucket.iter().filter_map(Weak::upgrade).find(|item| {
            item.units.as_ref() == units
                && item.one_byte == one_byte
                && super::Storage::Shared(item.clone()).can_intern_in_place()
        })
    }) {
        found.interned.store(true, Ordering::Release);
        return Ok(JsString(super::Storage::Shared(found), one_byte));
    }
    if table.count >= MAX_ENTRIES {
        table.buckets.retain(|_, bucket| {
            bucket.retain(|entry| entry.strong_count() != 0);
            !bucket.is_empty()
        });
        table.count = table.buckets.values().map(Vec::len).sum();
        if table.count >= MAX_ENTRIES {
            return Err(EngineError::limit("shared interned strings", MAX_ENTRIES));
        }
    }
    let shared = Arc::new(super::storage::FlatData {
        units: Arc::from(units),
        one_byte,
        external: super::storage::ExternalSlot::default(),
        old: std::sync::atomic::AtomicBool::new(true),
        survivals: std::sync::atomic::AtomicU8::new(0),
        id: super::Storage::next_id(),
        interned: std::sync::atomic::AtomicBool::new(true),
        read_only: false,
        forward: std::sync::OnceLock::new(),
    });
    table
        .buckets
        .entry(hash)
        .or_default()
        .push(Arc::downgrade(&shared));
    table.count += 1;
    Ok(JsString(super::Storage::Shared(shared), one_byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Looking up an existing allocation preserves its identity without allocating a copy. */
    #[test]
    fn lookup_reuses_live_shared_backing() {
        let source = JsString::from_utf8("canonical lookup without allocation").unwrap();
        let canonical = intern(&source).unwrap();
        let found = find(&source).unwrap();
        assert!(canonical.same_backing(&found));
    }

    /** Shared aliases see the same actual external allocation even when publication races. */
    #[test]
    fn shared_externalization_publishes_one_real_resource() {
        let source = JsString::from_utf8("external shared string").unwrap();
        let shared = intern(&source).unwrap();
        let other = shared.clone();
        std::thread::scope(|scope| {
            scope.spawn(|| shared.externalize().unwrap());
            scope.spawn(|| other.externalize().unwrap());
        });
        assert!(shared.is_external() && other.is_external());
        let super::super::Storage::Shared(cell) = &shared.0 else {
            panic!("shared")
        };
        let resource = cell.external.get().unwrap();
        assert_eq!(
            resource.one_byte.as_deref(),
            Some(b"external shared string".as_slice())
        );
        assert!(shared.0.same(&other.0));
        let restored: JsString =
            serde_json::from_str(&serde_json::to_string(&shared).unwrap()).unwrap();
        assert!(restored.is_shared() && restored.is_external());
        assert!(shared.0.same(&restored.0));
    }

    /** Explicit two-byte representation keeps a real two-byte resource through snapshots. */
    #[test]
    fn external_two_byte_representation_survives_serialization() {
        let value = JsString::from_units_two_byte(vec![65, 66]).unwrap();
        assert!(!value.is_latin1());
        value.externalize().unwrap();
        let restored: JsString =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert!(!restored.is_latin1() && restored.is_external());
        let super::super::Storage::Flat(cell) = &restored.0 else {
            panic!("flat")
        };
        assert!(cell.external.get().unwrap().one_byte.is_none());
    }

    /** A shared arena field retains the external resource even after the writer drops its local string. */
    #[test]
    fn shared_field_keeps_external_resource_until_graph_collection() {
        use crate::v9::vm::data::{SharedGcArena, SharedGcValue};
        let source = JsString::from_utf8("arena external lifetime").unwrap();
        let shared = intern(&source).unwrap();
        let arena = SharedGcArena::new().unwrap();
        let root = arena
            .allocate(
                b"test",
                0,
                false,
                vec![SharedGcValue::SharedString(shared.clone())],
            )
            .unwrap();
        shared.externalize().unwrap();
        let super::super::Storage::Shared(cell) = &shared.0 else {
            panic!("shared")
        };
        let weak = Arc::downgrade(cell);
        drop(shared);
        let (SharedGcValue::SharedString(restored), _) = root.read(0).unwrap() else {
            panic!("field")
        };
        assert!(restored.is_external() && weak.upgrade().is_some());
        drop((restored, root));
        arena.collect();
        assert!(weak.upgrade().is_none());
    }

    /** Shared arena snapshots reconstruct external backing after the original graph is gone. */
    #[test]
    fn shared_field_snapshot_restores_external_backing() {
        use crate::v9::vm::data::{SharedGc, SharedGcArena, SharedGcValue};
        let source = JsString::from_utf8("external snapshot field").unwrap();
        let shared = intern(&source).unwrap();
        shared.externalize().unwrap();
        let arena = SharedGcArena::new().unwrap();
        let root = arena
            .allocate(
                b"test",
                0,
                false,
                vec![SharedGcValue::SharedString(shared.clone())],
            )
            .unwrap();
        let wire = serde_json::to_string(&root).unwrap();
        drop((root, shared));
        arena.collect();
        let restored: SharedGc = serde_json::from_str(&wire).unwrap();
        let (SharedGcValue::SharedString(value), _) = restored.read(0).unwrap() else {
            panic!("shared field")
        };
        assert!(value.is_shared() && value.is_external());
        assert_eq!(value.lossy(), "external snapshot field");
    }

    /** Equal live strings share an allocation while unrelated ordinary strings retain their provenance. */
    #[test]
    fn shares_equal_live_strings_without_branding_ordinary_values() {
        let source = JsString::from_units(vec![0xd800, 42]).unwrap();
        let first = intern(&source).unwrap();
        let second = intern(&source).unwrap();
        assert!(!source.is_shared());
        assert!(first.is_shared() && second.is_shared());
        assert!(first.0.same(&second.0));
        assert_eq!(first.as_units(), source.as_units());
    }

    /** The table does not retain the allocation after the final live string is dropped. */
    #[test]
    fn weak_entries_do_not_keep_string_allocations_alive() {
        let source = JsString::from_utf8("weak shared lifetime test").unwrap();
        let shared = intern(&source).unwrap();
        let super::super::Storage::Shared(units) = &shared.0 else {
            panic!("expected shared storage")
        };
        let weak = Arc::downgrade(units);
        drop(shared);
        assert!(weak.upgrade().is_none());
    }
    /** Shared graph snapshots distinguish a two-byte source from its one-byte internalized peer. */
    #[test]
    fn shared_graph_snapshot_preserves_canonical_encoding_and_identity() {
        use crate::v9::vm::data::{SharedGc, SharedGcArena, SharedGcValue};
        let source =
            JsString::from_units_two_byte("shared graph encoding test".encode_utf16().collect())
                .unwrap();
        let noncanonical = intern_noncanonical_two_byte(&source).unwrap();
        let canonical = intern_internalized(&noncanonical).unwrap();
        assert!(!noncanonical.same_backing(&canonical));
        let arena = SharedGcArena::new().unwrap();
        let root = arena
            .allocate(
                b"test",
                0,
                false,
                vec![
                    SharedGcValue::SharedString(noncanonical.clone()),
                    SharedGcValue::SharedString(canonical.clone()),
                ],
            )
            .unwrap();
        let wire = serde_json::to_string(&root).unwrap();
        drop((root, noncanonical, canonical));
        arena.collect();
        let restored: SharedGc = serde_json::from_str(&wire).unwrap();
        let (SharedGcValue::SharedString(first), _) = restored.read(0).unwrap() else {
            panic!("first string")
        };
        let (SharedGcValue::SharedString(second), _) = restored.read(1).unwrap() else {
            panic!("second string")
        };
        assert!(!first.is_latin1() && second.is_latin1());
        assert!(!first.same_backing(&second));
    }
}
