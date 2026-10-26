/** Dense arrays keep their existing wire form; sparse arrays retain only length and present entries. */
use super::{ArrayElements, JsValue};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/** Sparse keys are explicit so no absent index allocates memory during decoding. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sparse {
    /** JavaScript's maximum legal array length remains uint32::MAX. */
    length: u32,
    /** Strictly increasing present indices, all below length, with no explicit Hole values. */
    elements: Vec<(u32, JsValue)>,
}

/** The old dense state remains readable without a migration. */
#[derive(Deserialize)]
#[serde(untagged)]
enum Wire {
    Dense(Vec<JsValue>),
    Sparse(Sparse),
}

impl Serialize for ArrayElements {
    /** Only a genuine dense sequence is serialized by enumerating all positions. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(dense) = self.dense_slice() {
            return dense.serialize(serializer);
        }
        Sparse {
            length: self.length as u32,
            elements: self
                .resident_entries()
                .map(|(index, value)| (index as u32, value.clone()))
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ArrayElements {
    /** Invalid indices and duplicate sparse records fail before they can enter the heap. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Wire::deserialize(deserializer)? {
            Wire::Dense(values) if values.len() <= u32::MAX as usize => Ok(values.into()),
            Wire::Dense(_) => Err(serde::de::Error::custom("array length exceeds uint32")),
            Wire::Sparse(wire) => {
                decode(wire).ok_or_else(|| serde::de::Error::custom("invalid sparse array entries"))
            }
        }
    }
}

/** Canonical sorted records preserve identity and distinguish absent indices from undefined. */
fn decode(wire: Sparse) -> Option<ArrayElements> {
    let mut elements = ArrayElements::holes(wire.length as usize);
    let mut previous = None;
    for (index, value) in wire.elements {
        if index >= wire.length
            || previous.is_some_and(|previous| previous >= index)
            || value == JsValue::Hole
        {
            return None;
        }
        previous = Some(index);
        elements.set(index as usize, value);
    }
    Some(elements)
}
