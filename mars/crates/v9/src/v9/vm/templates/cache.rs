/** Cache entries serialize as bounded tuples; keys carry no authority beyond validated code and heap records. */
use crate::v9::value::ObjectId;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, SeqAccess, Visitor},
};
use std::{collections::BTreeMap, fmt};

/** Realm, parsed-source base, compiled function site and template slot identify one literal. */
pub(super) type Key = (u32, usize, u32, u32);
/** Literal arrays remain heap objects charged against the ordinary object/property/string/state quotas. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct Cache(pub(super) BTreeMap<Key, ObjectId>);

impl Cache {
    /** Empty caches need no saved-state field. */
    pub(in crate::v9) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /** Every entry owns distinct cooked/raw arrays, so the ordinary object limit also bounds metadata. */
    pub(in crate::v9) fn len(&self) -> usize {
        self.0.len()
    }
    /** The source-sensitive collector discards cache metadata for objects that actually died. */
    pub(in crate::v9::vm) fn retain_objects(
        &mut self,
        live: &crate::v9::vm::heap::FastSet<ObjectId>,
    ) {
        self.0.retain(|_, id| live.contains(id));
    }
    /** Read an identity only after restored cache invariants have been validated. */
    pub(super) fn get(&self, key: &Key) -> Option<ObjectId> {
        self.0.get(key).copied()
    }
    /** The newly frozen pair becomes visible to source-lifetime tracing before its temporary pins leave. */
    pub(super) fn insert(&mut self, key: Key, id: ObjectId) {
        self.0.insert(key, id);
    }
}

impl Serialize for Cache {
    /** Flat integer tuples have stable JSON form without lossy stringification of compound keys. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            self.0
                .iter()
                .map(|(&(realm, base, site, slot), &id)| (realm, base, site, slot, id)),
        )
    }
}

impl<'de> Deserialize<'de> for Cache {
    /** Decode incrementally under the existing object cap, rejecting duplicate parse-site identities immediately. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Cache;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded V9 template entries")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Cache, A::Error> {
                let mut entries = BTreeMap::new();
                while let Some((realm, base, site, slot, id)) =
                    seq.next_element::<(u32, usize, u32, u32, ObjectId)>()?
                {
                    if entries.len() >= crate::constants::VM_OBJECTS
                        || entries.insert((realm, base, site, slot), id).is_some()
                    {
                        return Err(A::Error::custom("Invalid V9 template cache entries"));
                    }
                }
                Ok(Cache(entries))
            }
        }
        deserializer.deserialize_seq(Entries)
    }
}
