/**
 * The compact private wire form of values: numbers and strings are plain JSON, rarer values are
 * one-key maps. It keeps the bounded state snapshot small; NaN payloads canonicalize.
 */
use super::{JsString, JsValue};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, MapAccess, SeqAccess, Visitor},
    ser::SerializeMap,
};

// String identities are scoped to one state decode and cannot grant cross-session identity.
#[cfg(feature = "conformance")]
thread_local! { static STRING_IDENTITIES: std::cell::RefCell<Option<std::collections::BTreeMap<u64, JsString>>> = const { std::cell::RefCell::new(None) }; }
#[cfg(feature = "conformance")]
thread_local! { static ENCODE_IDENTITIES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(feature = "conformance")]
thread_local! { static ENCODE_IDS: std::cell::RefCell<Option<std::collections::BTreeMap<u64, u64>>> = const { std::cell::RefCell::new(None) }; }

/** Assign stable per-snapshot identities independent of process allocation counters. */
#[cfg(feature = "conformance")]
fn encode_id(value: &JsString) -> u64 {
    ENCODE_IDS.with(|state| {
        let mut state = state.borrow_mut();
        let Some(ids) = state.as_mut() else {
            return value.snapshot_id();
        };
        let next = ids.len() as u64 + 1;
        *ids.entry(value.snapshot_id()).or_insert(next)
    })
}

/** Per-value metadata a conformance encode records, keyed by object identity. */
#[cfg(feature = "conformance")]
type EncodeMetadata = Option<std::collections::BTreeMap<u64, (bool, bool, bool)>>;
#[cfg(feature = "conformance")]
thread_local! { static ENCODE_METADATA: std::cell::RefCell<EncodeMetadata> = const { std::cell::RefCell::new(None) }; }
#[cfg(feature = "conformance")]
thread_local! { static SLICE_PARENTS: std::cell::RefCell<Option<std::collections::BTreeMap<u64, JsString>>> = const { std::cell::RefCell::new(None) }; }
#[cfg(feature = "conformance")]
thread_local! { static DECODE_STRING_BUDGET: std::cell::RefCell<Option<crate::v9::vm::string_budget::StringBudget>> = const { std::cell::RefCell::new(None) }; }

/** Track decoded backing weakly, so aliases do not pay repeatedly or extend lifetime. */
#[cfg(feature = "conformance")]
fn track_decoded(value: &JsString) {
    DECODE_STRING_BUDGET.with(|cell| {
        if let Some(budget) = cell.borrow_mut().as_mut() {
            budget.register(value);
        }
    });
}

/** Reserve output storage before decoding an ordinary string with no existing source cell. */
#[cfg(feature = "conformance")]
fn bounded_decoded_new<E: Error>(
    units: usize,
    make: impl FnOnce() -> crate::protocol::EngineResult<JsString>,
) -> Result<JsString, E> {
    let reserved = DECODE_STRING_BUDGET.with(|cell| -> Result<usize, E> {
        match cell.borrow_mut().as_mut() {
            Some(budget) => budget
                .reserve_new(units, crate::constants::JS_ENVIRONMENT_UNITS)
                .map_err(|e| E::custom(e.message)),
            None => Ok(0),
        }
    })?;
    let result = make();
    DECODE_STRING_BUDGET.with(|cell| {
        if let Some(budget) = cell.borrow_mut().as_mut() {
            if let Ok(value) = &result {
                budget.register(value);
            }
            budget.release(reserved);
        }
    });
    result.map_err(|error| E::custom(error.message))
}

/** Hold output and optional source-cache reservations until construction finishes. */
#[cfg(feature = "conformance")]
fn bounded_decoded_string<E: Error>(
    left: &JsString,
    right: Option<&JsString>,
    units: usize,
    materialize: bool,
    make: impl FnOnce() -> crate::protocol::EngineResult<JsString>,
) -> Result<JsString, E> {
    let reservations = DECODE_STRING_BUDGET.with(|cell| -> Result<(usize, usize), E> {
        let mut cell = cell.borrow_mut();
        let Some(budget) = cell.as_mut() else {
            return Ok((0, 0));
        };
        if let Some(right) = right {
            budget.register(right);
        }
        let source = if materialize {
            budget
                .preflight(left, crate::constants::JS_ENVIRONMENT_UNITS)
                .map_err(|e| E::custom(e.message))?
        } else {
            0
        };
        match budget.reserve_output(left, units, crate::constants::JS_ENVIRONMENT_UNITS) {
            Ok(output) => Ok((source, output)),
            Err(error) => {
                budget.release(source);
                Err(E::custom(error.message))
            }
        }
    })?;
    let result = make();
    DECODE_STRING_BUDGET.with(|cell| {
        if let Some(budget) = cell.borrow_mut().as_mut() {
            if let Ok(value) = &result {
                budget.register(value);
            }
            budget.release(reservations.0);
            budget.release(reservations.1);
        }
    });
    result.map_err(|error| E::custom(error.message))
}

/** Isolate the parent table to one snapshot and restore any enclosing snapshot. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn with_slice_parents<T>(
    parents: std::collections::BTreeMap<u64, JsString>,
    f: impl FnOnce() -> T,
) -> T {
    SLICE_PARENTS.with(|cell| {
        let previous = cell.replace(Some(parents));
        struct Restore<'a>(
            &'a std::cell::RefCell<Option<std::collections::BTreeMap<u64, JsString>>>,
            Option<std::collections::BTreeMap<u64, JsString>>,
        );
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.replace(self.1.take());
            }
        }
        let _restore = Restore(cell, previous);
        f()
    })
}

/** Install a validated table inside the current scoped snapshot decode. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn set_slice_parents(parents: std::collections::BTreeMap<u64, JsString>) {
    SLICE_PARENTS.with(|cell| {
        cell.replace(Some(parents));
    });
}

/** Serialize the collected table without producing references within the table itself. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn serialize_slice_parents<S: Serializer>(
    serializer: S,
) -> Result<S::Ok, S::Error> {
    SLICE_PARENTS.with(|cell| {
        let parents = cell.replace(None).unwrap_or_default();
        let result = serializer.collect_seq(parents.iter());
        cell.replace(Some(parents));
        result
    })
}

/** A parent reference is emitted only in the enclosing snapshot, never standalone. */
#[cfg(feature = "conformance")]
struct SliceParent<'a>(&'a JsString);
#[cfg(feature = "conformance")]
impl Serialize for SliceParent<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let id = SLICE_PARENTS.with(|cell| {
            let mut table = cell.borrow_mut();
            table.as_mut().map(|table| {
                let id = encode_id(self.0);
                table.entry(id).or_insert_with(|| self.0.clone());
                id
            })
        });
        match id {
            Some(id) => single(serializer, "R", &id),
            None => self.0.serialize(serializer),
        }
    }
}

/** Sample mutable backing metadata once for each allocation in one state snapshot. */
#[cfg(feature = "conformance")]
fn encode_metadata(value: &JsString) -> (bool, bool, bool) {
    ENCODE_METADATA.with(|state| {
        let mut state = state.borrow_mut();
        let sample = || {
            (
                value.is_old(),
                value.survived_once(),
                value.is_external() && value.thin_target().is_none(),
            )
        };
        match state.as_mut() {
            Some(table) => *table.entry(value.snapshot_id()).or_insert_with(sample),
            None => sample(),
        }
    })
}

/** Request identity tags only for complete VM snapshots, preserving standalone string wire form. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn with_string_identity_encoding<T>(encode: impl FnOnce() -> T) -> T {
    ENCODE_IDENTITIES.with(|state| {
        let previous = state.replace(true);
        struct Restore<'a>(&'a std::cell::Cell<bool>, bool);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(state, previous);
        ENCODE_IDS.with(|ids| {
            let previous = ids.replace(Some(std::collections::BTreeMap::new()));
            struct RestoreIds<'a>(
                &'a std::cell::RefCell<Option<std::collections::BTreeMap<u64, u64>>>,
                Option<std::collections::BTreeMap<u64, u64>>,
            );
            impl Drop for RestoreIds<'_> {
                fn drop(&mut self) {
                    self.0.replace(self.1.take());
                }
            }
            let _restore = RestoreIds(ids, previous);
            ENCODE_METADATA.with(|metadata| {
                let previous = metadata.replace(Some(std::collections::BTreeMap::new()));
                struct RestoreMetadata<'a>(&'a std::cell::RefCell<EncodeMetadata>, EncodeMetadata);
                impl Drop for RestoreMetadata<'_> {
                    fn drop(&mut self) {
                        self.0.replace(self.1.take());
                    }
                }
                let _restore = RestoreMetadata(metadata, previous);
                encode()
            })
        })
    })
}

/** Establish an isolated identity namespace while a VM snapshot is deserialized. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn with_string_identities<T>(decode: impl FnOnce() -> T) -> T {
    STRING_IDENTITIES.with(|state| {
        let previous = state.replace(Some(std::collections::BTreeMap::new()));
        struct Restore<'a>(
            &'a std::cell::RefCell<Option<std::collections::BTreeMap<u64, JsString>>>,
            Option<std::collections::BTreeMap<u64, JsString>>,
        );
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.replace(self.1.take());
            }
        }
        let _restore = Restore(state, previous);
        DECODE_STRING_BUDGET.with(|budget| {
            let previous = budget.replace(Some(Default::default()));
            struct RestoreBudget<'a>(
                &'a std::cell::RefCell<Option<crate::v9::vm::string_budget::StringBudget>>,
                Option<crate::v9::vm::string_budget::StringBudget>,
            );
            impl Drop for RestoreBudget<'_> {
                fn drop(&mut self) {
                    self.0.replace(self.1.take());
                }
            }
            let _restore = RestoreBudget(budget, previous);
            decode()
        })
    })
}

/** Reuse a local allocation only for identical validated contents and representation. */
#[cfg(feature = "conformance")]
fn identity_value<E: Error>(id: u64, value: JsString) -> Result<JsString, E> {
    if id == 0 {
        return Err(E::custom("invalid string identity"));
    }
    STRING_IDENTITIES.with(|state| {
        let mut state = state.borrow_mut();
        let Some(table) = state.as_mut() else {
            return Ok(value);
        };
        if let Some(existing) = table.get(&id) {
            if existing != &value
                || !existing.0.same_representation(&value.0)
                || existing.is_latin1() != value.is_latin1()
                || existing.is_old() != value.is_old()
                || existing.is_external() != value.is_external()
                || existing.survived_once() != value.survived_once()
            {
                return Err(E::custom("inconsistent string identity"));
            }
            return Ok(existing.clone());
        }
        table.insert(id, value.clone());
        Ok(value)
    })
}

/** Reuse an already decoded external resource before making another quota reservation. */
#[cfg(feature = "conformance")]
fn external_identity<E: Error>(
    id: u64,
    units: &[u16],
    shared: bool,
    one_byte: bool,
) -> Result<Option<JsString>, E> {
    if id == 0 {
        return Err(E::custom("invalid string identity"));
    }
    STRING_IDENTITIES.with(|state| {
        let state = state.borrow();
        let Some(existing) = state.as_ref().and_then(|table| table.get(&id)) else {
            return Ok(None);
        };
        if existing.as_units() != units
            || existing.is_shared() != shared
            || existing.is_latin1() != one_byte
            || !existing.is_external()
            || !existing.is_old()
        {
            return Err(E::custom("inconsistent string identity"));
        }
        Ok(Some(existing.clone()))
    })
}

impl Serialize for JsValue {
    /** Plain JSON where it is unambiguous; tagged maps otherwise. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            #[cfg(feature = "conformance")]
            Self::Smi(value) => {
                if !value.is_finite()
                    || value.fract() != 0.0
                    || *value < -(1_i64 << 30) as f64
                    || *value >= (1_i64 << 30) as f64
                    || (*value == 0.0 && value.is_sign_negative())
                {
                    return Err(serde::ser::Error::custom("invalid Smi"));
                }
                single(serializer, "M", &(*value as i32))
            }
            #[cfg(feature = "conformance")]
            Self::SharedNumber(value) => single(serializer, "q", &(value.to_bits(), value.root)),
            #[cfg(feature = "conformance")]
            Self::LocalNumber(value) => single(serializer, "Q", &(value.to_bits(), value.root)),
            Self::Number(value) if value.is_finite() => serializer.serialize_f64(*value),
            Self::Number(value) => {
                let mut map = serializer.serialize_map(Some(1))?;
                let name = if value.is_nan() {
                    "NaN"
                } else if *value > 0.0 {
                    "Infinity"
                } else {
                    "-Infinity"
                };
                map.serialize_entry("n", name)?;
                map.end()
            }
            Self::String(value) => encode_string(value, serializer, true, true),
            Self::Boolean(value) => serializer.serialize_bool(*value),
            Self::Null => serializer.serialize_unit(),
            Self::Undefined => serializer.serialize_map(Some(0))?.end(),
            Self::Object(id) => single(serializer, "o", id),
            Self::Symbol(id) => single(serializer, "y", id),
            Self::BigInt(value) => single(serializer, "b", &value.to_string()),
            Self::BigIntWide(value) => match value.to_i128() {
                Some(inline) => single(serializer, "b", &inline.to_string()),
                None => single(serializer, "B", value),
            },
            Self::Hole => single(serializer, "h", &0u8),
        }
    }
}

/** Serialize a string while allowing an age wrapper without recursion. */
fn encode_string<S: Serializer>(
    value: &JsString,
    serializer: S,
    include_age: bool,
    include_intern: bool,
) -> Result<S::Ok, S::Error> {
    #[cfg(feature = "conformance")]
    let (snapshot_old, snapshot_survived, snapshot_external) = encode_metadata(value);
    #[cfg(feature = "conformance")]
    if include_age
        && ENCODE_IDENTITIES.with(|enabled| enabled.get())
        && snapshot_old
        && !value.is_shared()
        && !snapshot_external
    {
        return single(serializer, "g", &(encode_id(value), Unaged(value)));
    }
    #[cfg(feature = "conformance")]
    if include_age
        && ENCODE_IDENTITIES.with(|enabled| enabled.get())
        && !snapshot_old
        && snapshot_survived
    {
        return single(serializer, "j", &(encode_id(value), Unaged(value)));
    }
    #[cfg(feature = "conformance")]
    if include_age
        && ENCODE_IDENTITIES.with(|enabled| enabled.get())
        && !snapshot_old
        && !snapshot_survived
        && !value.is_shared()
    {
        return single(serializer, "k", &(encode_id(value), Unaged(value)));
    }
    let _ = (include_age, include_intern);
    #[cfg(feature = "conformance")]
    if value.is_read_only_root() {
        return single(
            serializer,
            "C",
            &value.unit_at(0).expect("one-byte character root"),
        );
    }
    #[cfg(feature = "conformance")]
    if let Some(target) = value.thin_target() {
        return single(serializer, "I", &target);
    }
    #[cfg(feature = "conformance")]
    if include_intern && value.is_shared() && value.is_interned() {
        return single(serializer, "x", &(value.as_units(), snapshot_external));
    }
    #[cfg(feature = "conformance")]
    if include_intern && value.is_interned() {
        return single(serializer, "i", &Uninterned(value));
    }

    #[cfg(feature = "conformance")]
    if snapshot_external {
        return single(
            serializer,
            "e",
            &(
                encode_id(value),
                value.as_units(),
                value.is_shared(),
                value.is_latin1(),
            ),
        );
    }
    #[cfg(feature = "conformance")]
    if value.is_shared()
        && !value.is_interned()
        && !value.is_latin1()
        && value
            .as_units()
            .iter()
            .all(|unit| *unit <= super::LATIN1_MAX_UNIT)
    {
        return single(serializer, "d", value.as_units());
    }
    #[cfg(feature = "conformance")]
    if value.is_shared()
        && !value.is_latin1()
        && value
            .as_units()
            .iter()
            .all(|unit| *unit <= super::LATIN1_MAX_UNIT)
    {
        return single(serializer, "u", value.as_units());
    }
    #[cfg(feature = "conformance")]
    if value.is_shared() {
        return single(serializer, "s", value.as_units());
    }
    #[cfg(feature = "conformance")]
    if let Some((parent, start, length)) = value.slice_parts() {
        if start > 0 && start.checked_add(length) == Some(parent.len()) {
            return single(serializer, "l", &(SliceParent(&parent), start));
        }
        return single(serializer, "v", &(SliceParent(&parent), start, length));
    }
    #[cfg(feature = "conformance")]
    if ENCODE_IDENTITIES.with(|enabled| enabled.get())
        && let Some(parts) = value.flattened_repeat_parts()
    {
        return single(serializer, "p", &parts);
    }
    #[cfg(feature = "conformance")]
    if ENCODE_IDENTITIES.with(|enabled| enabled.get())
        && let Some(child) = value.flattened_cons_child()
    {
        return single(serializer, "m", &child);
    }
    #[cfg(feature = "conformance")]
    if ENCODE_IDENTITIES.with(|enabled| enabled.get())
        && let Some(parts) = value.cons_parts()
    {
        return single(serializer, "z", &parts);
    }
    if let Some((child, count)) = value.repeat_parts() {
        return single(serializer, "r", &(child, count));
    }
    if let Some(parts) = value.compact_concat_parts() {
        return single(serializer, "c", &parts);
    }
    #[cfg(feature = "conformance")]
    if !value.is_latin1()
        && value
            .as_units()
            .iter()
            .all(|unit| *unit <= super::LATIN1_MAX_UNIT)
    {
        return single(serializer, "t", value.as_units());
    }
    match String::from_utf16(value.as_units()) {
        Ok(text) => serializer.serialize_str(&text),
        Err(_) => {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("w", value.as_units())?;
            map.end()
        }
    }
}
#[cfg(feature = "conformance")]
struct Unaged<'a>(&'a JsString);
#[cfg(feature = "conformance")]
impl Serialize for Unaged<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        encode_string(self.0, serializer, false, true)
    }
}

#[cfg(feature = "conformance")]
struct Uninterned<'a>(&'a JsString);
#[cfg(feature = "conformance")]
impl Serialize for Uninterned<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        encode_string(self.0, serializer, false, false)
    }
}

/** A one-entry tagged map. */
fn single<S: Serializer, T: Serialize + ?Sized>(
    serializer: S,
    key: &str,
    value: &T,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(1))?;
    map.serialize_entry(key, value)?;
    map.end()
}

impl<'de> Deserialize<'de> for JsValue {
    /** The inverse of the compact form; unknown tags fail. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

/** Reads the compact form. */
struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = JsValue;
    /** Error text for unexpected input. */
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a V9 value")
    }
    /** Booleans. */
    fn visit_bool<E: Error>(self, value: bool) -> Result<JsValue, E> {
        Ok(JsValue::Boolean(value))
    }
    /** Integers become numbers. */
    fn visit_i64<E: Error>(self, value: i64) -> Result<JsValue, E> {
        Ok(JsValue::Number(value as f64))
    }
    /** Integers become numbers. */
    fn visit_u64<E: Error>(self, value: u64) -> Result<JsValue, E> {
        Ok(JsValue::Number(value as f64))
    }
    /** Numbers. */
    fn visit_f64<E: Error>(self, value: f64) -> Result<JsValue, E> {
        Ok(JsValue::Number(value))
    }
    /** Strings, under the string limit. */
    fn visit_str<E: Error>(self, value: &str) -> Result<JsValue, E> {
        #[cfg(feature = "conformance")]
        let units = value.encode_utf16().count();
        #[cfg(feature = "conformance")]
        let value = if units <= crate::constants::JS_STRING_UNITS {
            bounded_decoded_new::<E>(units.saturating_mul(2), || JsString::from_utf8(value))?
        } else {
            JsString::from_utf8(value).map_err(|error| E::custom(error.message))?
        };
        #[cfg(not(feature = "conformance"))]
        let value = JsString::from_utf8(value).map_err(|error| E::custom(error.message))?;
        #[cfg(feature = "conformance")]
        track_decoded(&value);
        Ok(JsValue::String(value))
    }
    /** null. */
    fn visit_unit<E: Error>(self) -> Result<JsValue, E> {
        Ok(JsValue::Null)
    }
    /** null. */
    fn visit_none<E: Error>(self) -> Result<JsValue, E> {
        Ok(JsValue::Null)
    }
    /** Sequences are not values. */
    fn visit_seq<A: SeqAccess<'de>>(self, _: A) -> Result<JsValue, A::Error> {
        Err(A::Error::custom("unexpected sequence"))
    }
    /** Tagged maps. */
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsValue, A::Error> {
        let Some(key) = map.next_key::<String>()? else {
            return Ok(JsValue::Undefined);
        };
        let value = match key.as_str() {
            "n" => match map.next_value::<String>()?.as_str() {
                "NaN" => JsValue::Number(f64::NAN),
                "Infinity" => JsValue::Number(f64::INFINITY),
                "-Infinity" => JsValue::Number(f64::NEG_INFINITY),
                _ => return Err(A::Error::custom("invalid number")),
            },
            "w" => {
                let units = map.next_value::<Vec<u16>>()?;
                #[cfg(feature = "conformance")]
                let value = bounded_decoded_new::<A::Error>(
                    units.capacity().saturating_add(units.len()),
                    || JsString::from_units(units),
                )?;
                #[cfg(not(feature = "conformance"))]
                let value =
                    JsString::from_units(units).map_err(|error| A::Error::custom(error.message))?;
                JsValue::String(value)
            }
            #[cfg(feature = "conformance")]
            "R" => {
                let id: u64 = map.next_value()?;
                let parent = SLICE_PARENTS.with(|cell| {
                    cell.borrow()
                        .as_ref()
                        .and_then(|table| table.get(&id))
                        .cloned()
                });
                JsValue::String(parent.ok_or_else(|| A::Error::custom("unknown slice parent"))?)
            }
            #[cfg(feature = "conformance")]
            "l" => {
                let (parent, start): (JsString, usize) = map.next_value()?;
                let slice = parent
                    .construct_slice(start)
                    .map_err(|error| A::Error::custom(error.message))?
                    .ok_or_else(|| A::Error::custom("invalid sliced string"))?;
                JsValue::String(slice)
            }
            #[cfg(feature = "conformance")]
            "v" => {
                let (parent, start, length): (JsString, usize, usize) = map.next_value()?;
                let slice = JsString::from_slice_range(&parent, start, length)
                    .ok_or_else(|| A::Error::custom("invalid sliced string range"))?;
                JsValue::String(slice)
            }
            #[cfg(feature = "conformance")]
            "p" => {
                let (parent, count, flat): (JsString, usize, JsString) = map.next_value()?;
                let repeated = JsString::from_flattened_repeat(&parent, count, &flat)
                    .map_err(|error| A::Error::custom(error.message))?
                    .ok_or_else(|| A::Error::custom("invalid flattened repeat string"))?;
                JsValue::String(repeated)
            }
            #[cfg(feature = "conformance")]
            "m" => {
                let child: JsString = map.next_value()?;
                let cons = JsString::from_flattened_cons(&child)
                    .map_err(|error| A::Error::custom(error.message))?
                    .ok_or_else(|| A::Error::custom("invalid flattened cons string"))?;
                JsValue::String(cons)
            }
            #[cfg(feature = "conformance")]
            "z" => {
                let (left, right): (JsString, JsString) = map.next_value()?;
                let cons = left
                    .construct_cons(&right)
                    .map_err(|error| A::Error::custom(error.message))?
                    .ok_or_else(|| A::Error::custom("invalid cons string"))?;
                JsValue::String(cons)
            }
            "r" => {
                let (child, count): (JsString, usize) = map.next_value()?;
                #[cfg(feature = "conformance")]
                let result = if child.repeat_will_flatten(count)
                    && child.len().saturating_mul(count) <= crate::constants::JS_STRING_UNITS
                {
                    let units = child.len().saturating_mul(count).saturating_mul(2);
                    bounded_decoded_string::<A::Error>(&child, None, units, true, || {
                        child.repeat(count)
                    })?
                } else {
                    child
                        .repeat(count)
                        .map_err(|error| A::Error::custom(error.message))?
                };
                #[cfg(not(feature = "conformance"))]
                let result = child
                    .repeat(count)
                    .map_err(|error| A::Error::custom(error.message))?;
                JsValue::String(result)
            }
            "c" => {
                let (left, right): (JsString, JsString) = map.next_value()?;
                #[cfg(feature = "conformance")]
                let result = if left.concat_will_flatten(&right)
                    && left.len().saturating_add(right.len()) <= crate::constants::JS_STRING_UNITS
                {
                    let units = left.len().saturating_add(right.len()).saturating_mul(2);
                    bounded_decoded_string::<A::Error>(&left, Some(&right), units, false, || {
                        left.concat(&right)
                    })?
                } else {
                    left.concat(&right)
                        .map_err(|error| A::Error::custom(error.message))?
                };
                #[cfg(not(feature = "conformance"))]
                let result = left
                    .concat(&right)
                    .map_err(|error| A::Error::custom(error.message))?;
                JsValue::String(result)
            }
            #[cfg(feature = "conformance")]
            "s" => {
                let units = map.next_value::<Vec<u16>>()?;
                let value = bounded_decoded_new::<A::Error>(
                    units.capacity().saturating_add(units.len()),
                    || JsString::from_units(units),
                )?;
                let shared = match super::find_shared_string(&value) {
                    Some(shared) => shared,
                    None => bounded_decoded_string::<A::Error>(
                        &value,
                        None,
                        value.len(),
                        false,
                        || super::intern_shared_string(&value),
                    )?,
                };
                JsValue::String(shared)
            }
            #[cfg(feature = "conformance")]
            "x" => {
                let (units, external): (Vec<u16>, bool) = map.next_value()?;
                let source = bounded_decoded_new::<A::Error>(
                    units.capacity().saturating_add(units.len()),
                    || JsString::from_units(units),
                )?;
                let value = match super::find_shared_internalized(&source) {
                    Some(value) => value,
                    None => bounded_decoded_string::<A::Error>(
                        &source,
                        None,
                        source.len(),
                        false,
                        || super::intern_shared_internalized(&source),
                    )?,
                };
                if external && !value.is_external() {
                    bounded_decoded_string::<A::Error>(
                        &value,
                        None,
                        value.len().saturating_mul(3),
                        false,
                        || {
                            value.externalize()?;
                            Ok(value.clone())
                        },
                    )?;
                }
                JsValue::String(value)
            }
            #[cfg(feature = "conformance")]
            "d" => {
                let units = map.next_value::<Vec<u16>>()?;
                let value = bounded_decoded_new::<A::Error>(
                    units.capacity().saturating_add(units.len()),
                    || JsString::from_units_two_byte(units),
                )?;
                let shared = match super::find_shared_two_byte_noncanonical(&value) {
                    Some(shared) => shared,
                    None => bounded_decoded_string::<A::Error>(
                        &value,
                        None,
                        value.len(),
                        false,
                        || super::intern_shared_two_byte_noncanonical(&value),
                    )?,
                };
                JsValue::String(shared)
            }
            #[cfg(feature = "conformance")]
            "M" => {
                let value: i32 = map.next_value()?;
                if !(-(1_i32 << 30)..(1_i32 << 30)).contains(&value) {
                    return Err(A::Error::custom("invalid Smi"));
                }
                JsValue::Smi(f64::from(value))
            }
            #[cfg(feature = "conformance")]
            "Q" => {
                let (bits, root): (u64, super::ObjectId) = map.next_value()?;
                JsValue::LocalNumber(std::sync::Arc::new(super::SharedNumberData {
                    value: f64::from_bits(bits),
                    root,
                }))
            }
            #[cfg(feature = "conformance")]
            "q" => {
                let (bits, root): (u64, super::ObjectId) = map.next_value()?;
                JsValue::SharedNumber(std::sync::Arc::new(super::SharedNumberData {
                    value: f64::from_bits(bits),
                    root,
                }))
            }
            #[cfg(feature = "conformance")]
            "u" => {
                let units = map.next_value::<Vec<u16>>()?;
                let value = bounded_decoded_new::<A::Error>(
                    units.capacity().saturating_add(units.len()),
                    || JsString::from_units_two_byte(units),
                )?;
                let shared = match super::find_shared_string(&value) {
                    Some(shared) => shared,
                    None => bounded_decoded_string::<A::Error>(
                        &value,
                        None,
                        value.len(),
                        false,
                        || super::intern_shared_string(&value),
                    )?,
                };
                JsValue::String(shared)
            }
            #[cfg(feature = "conformance")]
            "e" => {
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum ExternalWire {
                    Current((u64, Vec<u16>, bool, bool)),
                    Legacy((Vec<u16>, bool, bool)),
                }
                let (identity, units, shared, one_byte) = match map.next_value::<ExternalWire>()? {
                    ExternalWire::Current((id, units, shared, one_byte)) => {
                        (Some(id), units, shared, one_byte)
                    }
                    ExternalWire::Legacy((units, shared, one_byte)) => {
                        (None, units, shared, one_byte)
                    }
                };
                let existing = if let Some(id) = identity {
                    external_identity::<A::Error>(id, &units, shared, one_byte)?
                } else {
                    None
                };
                let value = if let Some(value) = existing {
                    value
                } else {
                    let allocation = units.capacity().saturating_add(units.len());
                    let mut value = bounded_decoded_new::<A::Error>(allocation, || {
                        if one_byte {
                            JsString::from_units(units)
                        } else {
                            JsString::from_units_two_byte(units)
                        }
                    })?;
                    if shared {
                        let noncanonical = !one_byte
                            && value
                                .code_units()
                                .all(|unit| unit <= super::LATIN1_MAX_UNIT);
                        let existing = if noncanonical {
                            super::find_shared_two_byte_noncanonical(&value)
                        } else {
                            super::find_shared_string(&value)
                        };
                        value = if let Some(existing) = existing {
                            existing
                        } else {
                            bounded_decoded_string::<A::Error>(
                                &value,
                                None,
                                value.len(),
                                false,
                                || {
                                    if noncanonical {
                                        super::intern_shared_two_byte_noncanonical(&value)
                                    } else {
                                        super::intern_shared_string(&value)
                                    }
                                },
                            )?
                        };
                    }
                    value.mark_old();
                    if !value.is_external() {
                        bounded_decoded_string::<A::Error>(
                            &value,
                            None,
                            value.len().saturating_mul(3),
                            false,
                            || {
                                value.externalize()?;
                                Ok(value.clone())
                            },
                        )?;
                    }
                    if let Some(id) = identity {
                        identity_value::<A::Error>(id, value)?
                    } else {
                        value
                    }
                };
                JsValue::String(value)
            }
            #[cfg(feature = "conformance")]
            "t" => {
                let units = map.next_value::<Vec<u16>>()?;
                let value = bounded_decoded_new::<A::Error>(
                    units.capacity().saturating_add(units.len()),
                    || JsString::from_units_two_byte(units),
                )?;
                JsValue::String(value)
            }
            #[cfg(feature = "conformance")]
            "g" => {
                let (id, value): (u64, JsString) = map.next_value()?;
                value.mark_old();
                JsValue::String(identity_value::<A::Error>(id, value)?)
            }
            #[cfg(feature = "conformance")]
            "j" => {
                let (id, value): (u64, JsString) = map.next_value()?;
                value.survived_minor();
                JsValue::String(identity_value::<A::Error>(id, value)?)
            }
            #[cfg(feature = "conformance")]
            "k" => {
                let (id, value): (u64, JsString) = map.next_value()?;
                if value.is_old()
                    || value.is_shared()
                    || (value.is_external() && value.thin_target().is_none())
                    || value.survived_once()
                {
                    return Err(A::Error::custom("invalid young string"));
                }
                JsValue::String(identity_value::<A::Error>(id, value)?)
            }
            #[cfg(feature = "conformance")]
            "C" => {
                let unit: u16 = map.next_value()?;
                let unit =
                    u8::try_from(unit).map_err(|_| A::Error::custom("invalid character root"))?;
                JsValue::String(bounded_decoded_new::<A::Error>(1, || {
                    Ok(JsString::character_root(unit))
                })?)
            }
            #[cfg(feature = "conformance")]
            "i" => {
                let value: JsString = map.next_value()?;
                if !value.mark_interned() {
                    return Err(A::Error::custom("invalid internalized string"));
                }
                JsValue::String(value)
            }
            #[cfg(feature = "conformance")]
            "I" => {
                let target: JsString = map.next_value()?;
                if !target.is_interned() {
                    return Err(A::Error::custom("invalid thin string target"));
                }
                let source = JsString::from_units(target.as_units().to_vec())
                    .map_err(|error| A::Error::custom(error.message))?;
                if !source.forward_to(&target) {
                    return Err(A::Error::custom("invalid thin string"));
                }
                JsValue::String(source)
            }
            "o" => JsValue::Object(map.next_value()?),
            "y" => JsValue::Symbol(map.next_value()?),
            "b" => JsValue::BigInt(
                map.next_value::<String>()?
                    .parse()
                    .map_err(|_| A::Error::custom("invalid bigint"))?,
            ),
            "B" => {
                let value = map.next_value::<super::BigInteger>()?;
                if value.to_i128().is_some() {
                    return Err(A::Error::custom("wide BigInt must not fit inline"));
                }
                JsValue::BigIntWide(value)
            }
            "h" => {
                map.next_value::<u8>()?;
                JsValue::Hole
            }
            _ => return Err(A::Error::custom("unknown value tag")),
        };
        if map.next_key::<String>()?.is_some() {
            return Err(A::Error::custom("extra value field"));
        }
        #[cfg(feature = "conformance")]
        if let JsValue::String(value) = &value {
            track_decoded(value);
        }
        Ok(value)
    }
}

impl Serialize for JsString {
    /** Strings serialize as values do. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        JsValue::String(self.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for JsString {
    /** Only string values are accepted. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match JsValue::deserialize(deserializer)? {
            JsValue::String(value) => Ok(value),
            _ => Err(D::Error::custom("expected a string")),
        }
    }
}

#[cfg(all(test, feature = "conformance"))]
mod decode_budget_tests {
    /** Ordinary string cells are charged alongside previously decoded live strings. */
    #[test]
    fn ordinary_strings_respect_live_decoding_budget() {
        let wire = serde_json::to_string(&"a".repeat(crate::constants::JS_STRING_UNITS)).unwrap();
        super::with_string_identities(|| {
            let mut values = Vec::new();
            let mut rejected = false;
            for _ in 0..24 {
                match serde_json::from_str::<super::JsString>(&wire) {
                    Ok(value) => values.push(value),
                    Err(error) => {
                        assert!(
                            error
                                .to_string()
                                .contains("resident string materialization units"),
                            "{error}"
                        );
                        rejected = true;
                        break;
                    }
                }
            }
            assert!(rejected);
        });
    }
    /** Compact forged trees cannot accumulate unbounded flat fallback buffers during one state. */
    #[test]
    fn nested_flat_fallbacks_respect_live_decoding_budget() {
        let mut wire = format!(
            "{{\"r\":[\"a\",{}]}}",
            crate::constants::JS_STRING_UNITS - 80
        );
        for _ in 0..45 {
            wire = format!("{{\"c\":[{wire},\"b\"]}}");
        }
        super::with_string_identities(|| {
            let mut values = Vec::new();
            let mut rejected = false;
            for _ in 0..24 {
                match serde_json::from_str::<super::JsString>(&wire) {
                    Ok(value) => values.push(value),
                    Err(error) => {
                        assert!(
                            error
                                .to_string()
                                .contains("resident string materialization units"),
                            "{error}"
                        );
                        rejected = true;
                        break;
                    }
                }
            }
            assert!(rejected);
        });
    }
}
