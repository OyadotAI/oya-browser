/** Stream bounded listener restoration and serialize borrowed records without cloning their strings. */
use super::{Listener, ListenerStore, validate_references};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use std::{collections::BTreeSet, fmt};

impl Serialize for ListenerStore {
    /** Preserve the existing registration-ordered array wire format with bounded pointer scratch. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut listeners: Vec<&Listener> = self.entries.values().collect();
        listeners.sort_unstable_by_key(|listener| listener.id);
        let mut sequence = serializer.serialize_seq(Some(listeners.len()))?;
        for listener in listeners {
            sequence.serialize_element(listener)?;
        }
        sequence.end()
    }
}

impl<'de> Deserialize<'de> for ListenerStore {
    /** Refuse at the first over-budget record rather than first allocating an entire hostile Vec. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(Records)
    }
}

/** Only one incoming record exists outside the already-admitted native index. */
struct Records;
impl<'de> Visitor<'de> for Records {
    type Value = ListenerStore;

    /** Describe the stable private wire shape when an input supplies a different type. */
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded event listener list")
    }

    /** Duplicate IDs/registration tuples are malformed, never repaired into extra registrations. */
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut store = ListenerStore::default();
        let mut ids = BTreeSet::new();
        while let Some(listener) = sequence.next_element::<Listener>()? {
            validate_references(&listener).map_err(|error| A::Error::custom(error.message))?;
            let callback = listener
                .callback
                .object()
                .ok_or_else(|| A::Error::custom("Invalid listener callback"))?;
            store
                .admit(&listener.kind)
                .map_err(|error| A::Error::custom(error.message))?;
            if !ids.insert(listener.id)
                || store
                    .find(listener.target, &listener.kind, listener.capture, callback)
                    .is_some()
            {
                return Err(A::Error::custom("Duplicate event listener registration"));
            }
            store
                .insert(listener)
                .map_err(|error| A::Error::custom(error.message))?;
        }
        Ok(store)
    }
}
