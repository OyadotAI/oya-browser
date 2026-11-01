/** A page map kept with its compact JSON between steps, so a summary's size check and its
encoding copy the kept bytes instead of walking the map again. */
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

/** A value with its compact JSON kept until the value changes. Reads go through `Deref`; every
mutable access goes through `DerefMut`, which drops the kept bytes first. The borrow checker is
the invalidation: a kept encoding cannot describe an older value, and no writer has to report a
change. Clones keep the bytes, which stay valid because the clone holds the same value. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct Encoded<T> {
    /** The value itself. */
    value: T,
    /** Its compact JSON, filled on first use and dropped by any mutable access. */
    json: OnceLock<Box<RawValue>>,
}

impl<T> Encoded<T> {
    /** Wrap `value` with no encoding kept yet. */
    pub fn new(value: T) -> Self {
        Self {
            value,
            json: OnceLock::new(),
        }
    }
    /** Drop the kept encoding, so the next encoding walks the value again. */
    #[cfg(debug_assertions)]
    pub fn forget_encoding(&mut self) {
        self.json.take();
    }
}

impl<T: Serialize> Encoded<T> {
    /** Keep the compact JSON of the current value. A value that cannot be encoded keeps nothing,
    and the encoding error surfaces where the value is written. */
    pub fn keep_encoding(&self) {
        let _ = self.encoding();
    }
    /** The kept JSON, encoding the value on first use. */
    fn encoding(&self) -> serde_json::Result<&RawValue> {
        if let Some(json) = self.json.get() {
            return Ok(json);
        }
        let json = serde_json::value::to_raw_value(&self.value)?;
        Ok(self.json.get_or_init(|| json))
    }
}

impl<K, V> Encoded<BTreeMap<K, V>> {
    /** Whether the map is empty, for `skip_serializing_if` on a kept map. */
    pub fn is_empty_map(&self) -> bool {
        self.value.is_empty()
    }
}

impl<T> Deref for Encoded<T> {
    type Target = T;
    /** Read access to the value; the kept encoding stays. */
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for Encoded<T> {
    /** Mutable access to the value. It drops the kept encoding, because the caller may change the
    value through the reference. */
    fn deref_mut(&mut self) -> &mut T {
        self.json.take();
        &mut self.value
    }
}

impl<T: Serialize> Serialize for Encoded<T> {
    /** Writes the kept JSON verbatim, so the bytes are those of the value itself. The raw form is
    serde_json's marker, so only serde_json may consume this type; the page is only ever encoded as
    JSON. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let json = self.encoding().map_err(serde::ser::Error::custom)?;
        json.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Encoded<T> {
    /** Decodes the value; nothing is kept until it is first encoded. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::Encoded;

    /** A change made through the mutable reference is what the next encoding shows, so a kept
    encoding cannot outlive the value it describes. */
    #[test]
    fn a_mutation_drops_the_kept_encoding() {
        let mut map = Encoded::new(std::collections::BTreeMap::from([(
            1_usize,
            "a".to_string(),
        )]));
        map.keep_encoding();
        map.insert(2, "b".to_string());
        assert_eq!(serde_json::to_string(&map).unwrap(), r#"{"1":"a","2":"b"}"#);
    }

    /** Reads keep the bytes, and a kept encoding writes exactly the bytes a fresh walk would. */
    #[test]
    fn a_kept_encoding_writes_the_same_bytes_as_a_fresh_one() {
        let kept = Encoded::new(vec![1_u8, 2, 3]);
        kept.keep_encoding();
        let fresh = Encoded::new(vec![1_u8, 2, 3]);
        assert_eq!(
            serde_json::to_string(&kept).unwrap(),
            serde_json::to_string(&fresh).unwrap()
        );
    }
}
