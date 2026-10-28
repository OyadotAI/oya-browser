/** Dense arrays keep their existing wire form; sparse arrays retain only length and present entries. */
use super::{ArrayElements, Characters, JsValue, MAP_ENTRY_SLOTS, Repeated};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::sync::Arc;

/** Sparse keys are explicit so no absent index allocates memory during decoding. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sparse {
    /** JavaScript's maximum legal array length remains uint32::MAX. */
    length: u32,
    /** Strictly increasing present indices, all below length, with no explicit Hole values. */
    elements: Vec<(u32, JsValue)>,
}

/** A compact repeated pattern retains physical values rather than expanding logical indices. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepeatedWire {
    length: u32,
    span: u32,
    base: Vec<JsValue>,
    overrides: Vec<(u32, JsValue)>,
    sparse: Vec<(u32, JsValue)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CharactersWire {
    length: u32,
    span: u32,
    source: JsValue,
    roots: Vec<JsValue>,
    overrides: Vec<(u32, JsValue)>,
    sparse: Vec<(u32, JsValue)>,
}

/** Stream sparse records rather than allocate a second snapshot-sized value vector. */
struct Entries<'a>(&'a BTreeMap<usize, JsValue>);

impl Serialize for Entries<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (index, value) in self.0 {
            seq.serialize_element(&(*index as u32, value))?;
        }
        seq.end()
    }
}

/** The encoder borrows all resident values so peak memory stays bounded. */
#[derive(Serialize)]
struct RepeatedRef<'a> {
    length: u32,
    span: u32,
    base: &'a [JsValue],
    overrides: Entries<'a>,
    sparse: Entries<'a>,
}

#[derive(Serialize)]
struct CharactersRef<'a> {
    length: u32,
    span: u32,
    source: &'a JsValue,
    roots: &'a [JsValue],
    overrides: Entries<'a>,
    sparse: Entries<'a>,
}

/** The old dense state remains readable without a migration. */
#[derive(Deserialize)]
#[serde(untagged)]
enum Wire {
    Dense(Vec<JsValue>),
    Sparse(Sparse),
    Repeated(RepeatedWire),
    Characters(CharactersWire),
}

impl Serialize for ArrayElements {
    /** Only a genuine dense sequence is serialized by enumerating all positions. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if let Some(characters) = &self.characters {
            return CharactersRef {
                length: self.length as u32,
                span: characters.span as u32,
                source: &characters.source,
                roots: &characters.roots,
                overrides: Entries(&characters.overrides),
                sparse: Entries(&self.sparse),
            }
            .serialize(serializer);
        }
        if let Some(repeated) = &self.repeated {
            return RepeatedRef {
                length: self.length as u32,
                span: repeated.span as u32,
                base: &repeated.base,
                overrides: Entries(&repeated.overrides),
                sparse: Entries(&self.sparse),
            }
            .serialize(serializer);
        }
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
            Wire::Repeated(wire) => decode_repeated(wire)
                .ok_or_else(|| serde::de::Error::custom("invalid repeated array entries")),
            Wire::Characters(wire) => decode_characters(wire)
                .ok_or_else(|| serde::de::Error::custom("invalid character array entries")),
        }
    }
}

/** Reconstruct canonical content and reject forged roots, offsets and missing source data. */
fn decode_characters(wire: CharactersWire) -> Option<ArrayElements> {
    let span = wire.span as usize;
    if span == 0 || wire.span > wire.length || wire.roots.len() != 256 {
        return None;
    }
    for (index, value) in wire.roots.iter().enumerate() {
        let JsValue::String(text) = value else {
            return None;
        };
        if text.len() != 1 || text.unit_at(0) != Some(index as u16) {
            return None;
        }
        #[cfg(feature = "conformance")]
        if !text.is_read_only_root() {
            return None;
        }
        #[cfg(not(feature = "conformance"))]
        return None;
    }
    let mut overrides = BTreeMap::new();
    let mut previous = None;
    for (index, value) in wire.overrides {
        if index >= wire.span || previous.is_some_and(|last| last >= index) {
            return None;
        }
        previous = Some(index);
        overrides.insert(index as usize, value);
    }
    match &wire.source {
        JsValue::String(text)
            if text.len() >= span
                && (0..span).all(|i| text.unit_at(i).is_some_and(|c| c <= 255)) => {}
        JsValue::Hole if overrides.len() == span => {}
        _ => return None,
    }
    let mut sparse = BTreeMap::new();
    previous = None;
    for (index, value) in wire.sparse {
        if index < wire.span
            || index >= wire.length
            || value == JsValue::Hole
            || previous.is_some_and(|last| last >= index)
        {
            return None;
        }
        previous = Some(index);
        sparse.insert(index as usize, value);
    }
    let resident = 258usize.checked_add(
        overrides
            .len()
            .checked_add(sparse.len())?
            .checked_mul(MAP_ENTRY_SLOTS)?,
    )?;
    if resident > crate::constants::VM_ARRAY_LENGTH {
        return None;
    }
    let units = std::iter::once(&wire.source)
        .chain(wire.roots.iter())
        .chain(overrides.values())
        .chain(sparse.values())
        .map(JsValue::string_units)
        .sum();
    Some(ArrayElements {
        length: wire.length as usize,
        resident,
        dense: Vec::new(),
        sparse,
        repeated: None,
        characters: Some(Characters {
            source: wire.source,
            roots: Arc::from(wire.roots),
            span,
            overrides,
            units,
        }),
    })
}

/** Rebuild derived coverage and reject forged missing pattern values or excessive physical state. */
fn decode_repeated(wire: RepeatedWire) -> Option<ArrayElements> {
    let limit = crate::constants::VM_ARRAY_LENGTH;
    let span = wire.span as usize;
    if span == 0 || wire.span > wire.length || wire.base.is_empty() || wire.base.len() > limit {
        return None;
    }
    let mut repeated = Repeated {
        base: Arc::from(wire.base),
        span,
        overrides: BTreeMap::new(),
        covered: BTreeMap::new(),
        units: 0,
    };
    let mut previous = None;
    for (index, value) in wire.overrides {
        if index >= wire.span || previous.is_some_and(|last| last >= index) {
            return None;
        }
        previous = Some(index);
        repeated.overrides.insert(index as usize, value);
    }
    repeated.recount();
    for (offset, value) in repeated.base.iter().enumerate() {
        if *value == JsValue::Hole {
            let count = if offset < span {
                (span - 1 - offset) / repeated.base.len() + 1
            } else {
                0
            };
            if repeated.covered.get(&offset).copied().unwrap_or(0) != count {
                return None;
            }
        }
    }
    let mut sparse = BTreeMap::new();
    previous = None;
    for (index, value) in wire.sparse {
        if index < wire.span
            || index >= wire.length
            || value == JsValue::Hole
            || previous.is_some_and(|last| last >= index)
        {
            return None;
        }
        previous = Some(index);
        sparse.insert(index as usize, value);
    }
    let resident = repeated
        .physical_slots()
        .checked_add(sparse.len().checked_mul(MAP_ENTRY_SLOTS)?)?;
    if resident > limit {
        return None;
    }
    repeated.units = repeated
        .base
        .iter()
        .chain(repeated.overrides.values())
        .chain(sparse.values())
        .map(JsValue::string_units)
        .sum();
    Some(ArrayElements {
        length: wire.length as usize,
        resident,
        dense: Vec::new(),
        sparse,
        repeated: Some(repeated),
        characters: None,
    })
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
