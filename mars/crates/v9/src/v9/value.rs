/** ECMAScript values: UTF-16 strings, binary64 numbers and arena identities, never native handles. */
mod f16;
pub(super) use f16::{decode as decode_f16, encode as encode_f16};
mod bigint;
pub use bigint::BigInt as BigInteger;
pub(in crate::v9) use bigint::{MAX_BITS as BIGINT_MAX_BITS, ParseError as BigIntParseError};
mod bigint_key;
pub(super) use bigint_key::bigint_property_key;
mod number;
mod storage;
mod string;
mod wire;
use crate::{
    constants::JS_STRING_UNITS,
    protocol::{EngineError, EngineResult},
};
pub(super) use number::{
    literal, number_to_string, radix_string, string_number, to_int32, to_uint32,
};
use storage::Storage;
pub(in crate::v9) use storage::materialized_units;
pub use string::CodePointCursor;
#[cfg(feature = "conformance")]
pub(in crate::v9) use wire::{
    serialize_slice_parents, set_slice_parents, with_slice_parents, with_string_identities,
    with_string_identity_encoding,
};

/** Heap object identities; intrinsic objects live in a reserved high range. */
pub type ObjectId = u32;

/// Code units at or below this value are Latin-1, the widest unit a one-byte string can hold.
const LATIN1_MAX_UNIT: u16 = 0xFF;
/** Keep streaming hash writes bounded while avoiding one hasher call per code unit. */
const HASH_CHUNK_BYTES: usize = 256;

/// JavaScript strings keep UTF-16 code units, including lone surrogates, and share their buffer.
/// The second field records the one-byte representation; it is computed at construction,
/// carried through clones, and combined in O(1) by concatenation.
#[derive(Clone)]
pub struct JsString(Storage, bool);

/** A non-owning string registration can observe backing allocations without extending lifetime. */
#[derive(Clone, Debug)]
pub(in crate::v9) struct WeakString(storage::WeakStorage);

impl WeakString {
    /** Reattach a live string without making the weak registry itself an owner. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn upgrade(&self, one_byte: bool) -> Option<JsString> {
        self.0.upgrade().map(|storage| JsString(storage, one_byte))
    }
    /** Check liveness without traversing or retaining child allocations. */
    pub(in crate::v9) fn is_alive(&self) -> bool {
        self.0.upgrade().is_some()
    }
    /** Visit current backing only while the underlying node still exists. */
    pub(in crate::v9) fn allocations(&self, visit: impl FnMut(bool, usize, usize, usize)) -> bool {
        let Some(storage) = self.0.upgrade() else {
            return false;
        };
        storage.allocations(visit);
        true
    }
}

/** A number retains its local hidden root into the session-owned shared arena. */
#[cfg(feature = "conformance")]
#[derive(Clone, Debug)]
pub struct SharedNumberData {
    pub(crate) value: f64,
    pub(crate) root: ObjectId,
}
#[cfg(feature = "conformance")]
impl std::ops::Deref for SharedNumberData {
    type Target = f64;
    fn deref(&self) -> &f64 {
        &self.value
    }
}

#[cfg(feature = "conformance")]
mod shared;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::collect as collect_shared_strings;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::find as find_shared_string;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::find_internalized as find_shared_internalized;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::find_noncanonical_two_byte as find_shared_two_byte_noncanonical;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::intern as intern_shared_string;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::intern_internalized as intern_shared_internalized;
#[cfg(feature = "conformance")]
pub(in crate::v9) use shared::intern_noncanonical_two_byte as intern_shared_two_byte_noncanonical;

impl PartialEq for JsString {
    /** Strings compare UTF-16 content, independently of flat or concatenated storage. */
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if self.0.same(&other.0) {
            return true;
        }
        if let (Some(left), Some(right)) = (self.flat_units(), other.flat_units()) {
            return left == right;
        }
        (0..self.len()).all(|index| self.unit_at(index) == other.unit_at(index))
    }
}
impl Eq for JsString {}
impl PartialOrd for JsString {
    /** Ordering remains the existing code-unit ordering, including unmatched surrogates. */
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for JsString {
    /** Tree shape and allocation identities never determine JavaScript key order. */
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.0.same(&other.0) {
            return std::cmp::Ordering::Equal;
        }
        if let (Some(left), Some(right)) = (self.flat_units(), other.flat_units()) {
            return left.cmp(right);
        }
        (0..self.len())
            .map(|index| self.unit_at(index))
            .cmp((0..other.len()).map(|index| other.unit_at(index)))
    }
}
impl std::hash::Hash for JsString {
    /** Equal UTF-16 contents hash identically without materializing compact storage. */
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // A flat string hashes as its unit slice does, in one write: property keys are hashed on
        // every lookup, and the unit-by-unit walk below was a large share of V9's execution.
        if let Some(units) = self.flat_units() {
            units.hash(state);
            return;
        }
        state.write_usize(self.len());
        let mut bytes = [0u8; HASH_CHUNK_BYTES];
        let mut used = 0;
        for unit in self.code_units() {
            bytes[used..used + 2].copy_from_slice(&unit.to_le_bytes());
            used += 2;
            if used == bytes.len() {
                state.write(&bytes);
                used = 0;
            }
        }
        if used != 0 {
            state.write(&bytes[..used]);
        }
    }
}

impl std::fmt::Debug for JsString {
    /** Debug output shows the text, replacing unpaired surrogates. */
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}", self.lossy())
    }
}

impl JsString {
    /** The identity `weak_registration` would report, without taking a weak reference. */
    pub(in crate::v9) fn registration_identity(&self) -> (u8, usize) {
        self.0.registration_identity()
    }
    /** Retain a non-owning allocation identity for VM resident tracking. */
    pub(in crate::v9) fn weak_registration(&self) -> ((u8, usize), WeakString) {
        let (identity, weak) = self.0.downgrade();
        (identity, WeakString(weak))
    }
    /** Visit immutable backing allocations for bounded heap accounting. */
    pub(in crate::v9) fn resident_allocations(&self, visit: impl FnMut(bool, usize, usize, usize)) {
        self.0.allocations(visit);
    }
    /** `resident_allocations` without the child subtrees `known` names by registration
    identity: those already counted (see `Storage::allocations_except`). */
    pub(in crate::v9) fn new_allocations(
        &self,
        known: impl Fn((u8, usize)) -> bool,
        visit: impl FnMut(bool, usize, usize, usize),
    ) {
        self.0.allocations_except(known, visit);
    }

    /** Estimate resident UTF-16 units including allocation overhead, deduplicating this value's aliases. */
    pub fn resident_units(&self) -> usize {
        let mut total = 0usize;
        let mut seen = std::collections::HashSet::new();
        self.resident_allocations(|node, identity, _, upper| {
            if seen.insert((node, identity)) {
                total = total.saturating_add(upper);
            }
        });
        total.div_ceil(2)
    }

    /** Materialize only within the existing resident string limit. */
    pub fn try_as_units(&self) -> EngineResult<&[u16]> {
        if self.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(self.as_units())
    }
    /** Borrow already contiguous units without forcing compact strings to flatten. */
    pub(in crate::v9) fn flat_units(&self) -> Option<&[u16]> {
        self.0.flat_units()
    }
    /** Preserve a compact repeated string in snapshots without observing or flattening its content. */
    pub(super) fn repeat_parts(&self) -> Option<(Self, usize)> {
        self.0
            .repeat_parts()
            .map(|(child, count)| (Self(child, self.effective_encoding()), count))
    }
    /** Preserve a compact repeated child inside a concatenation without changing ordinary wire form. */
    pub(super) fn compact_concat_parts(&self) -> Option<(Self, Self)> {
        self.0.compact_concat_parts().map(|(left, right)| {
            let left_latin1 = left.latin1();
            let right_latin1 = right.latin1();
            (Self(left, left_latin1), Self(right, right_latin1))
        })
    }
    /** Read an indexed UTF-16 unit without flattening shared or concatenated storage. */
    pub(in crate::v9) fn unit_at(&self, index: usize) -> Option<u16> {
        self.0.unit_at(index)
    }
    /** Short slices traverse bounded rope nodes; long slices retain the existing bulk-copy path. */
    pub(in crate::v9) fn copy_range(&self, start: usize, end: usize) -> Vec<u16> {
        const SHORT_SLICE_UNITS: usize = 1024;
        if end - start <= SHORT_SLICE_UNITS {
            return (start..end)
                .map(|index| self.unit_at(index).expect("checked string range"))
                .collect();
        }
        self.as_units()[start..end].to_vec()
    }
    /** Preserve repeated spans without allocating their logical contents. */
    pub(in crate::v9) fn slice_range(&self, start: usize, end: usize) -> EngineResult<Self> {
        if start == 0 && end == self.len() {
            return Ok(self.clone());
        }
        #[cfg(feature = "conformance")]
        if let Some(storage) = self.0.sliced_range(start, end) {
            return Ok(Self(storage, self.effective_encoding()));
        }
        if let Some((child, _)) = self.repeat_parts() {
            let width = child.len();
            let offset = start % width;
            let prefix_len = if offset == 0 {
                0
            } else {
                (width - offset).min(end - start)
            };
            let remaining = end - start - prefix_len;
            let prefix = child.slice_range(offset, offset + prefix_len)?;
            let middle = child.repeat(remaining / width)?;
            let suffix = child.slice_range(0, remaining % width)?;
            return prefix.concat(&middle)?.concat(&suffix);
        }
        if let Some((left, right)) = self.compact_concat_parts() {
            let boundary = left.len();
            if end <= boundary {
                return left.slice_range(start, end);
            }
            if start >= boundary {
                return right.slice_range(start - boundary, end - boundary);
            }
            return left
                .slice_range(start, boundary)?
                .concat(&right.slice_range(0, end - boundary)?);
        }
        Self::from_units(self.copy_range(start, end))
    }
    /** Repeated strings share one bounded immutable backing until contiguous text is needed. */
    pub(in crate::v9) fn repeat_will_flatten(&self, count: usize) -> bool {
        count != 1 && self.0.repeat_will_flatten(count)
    }
    pub(in crate::v9) fn repeat(&self, count: usize) -> EngineResult<Self> {
        if count == 1 {
            return Ok(self.clone());
        }
        let length = self.len().saturating_mul(count);
        if length > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self(self.0.repeat(count), self.effective_encoding()))
    }
    /** Retain the immutable native backing of an actually shared string. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn from_shared(units: std::sync::Arc<[u16]>) -> EngineResult<Self> {
        if units.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        let latin1 = units.iter().all(|unit| *unit <= LATIN1_MAX_UNIT);
        Ok(Self(Storage::shared(units, latin1), latin1))
    }
    /** Distinguish native shared backing from equal ordinary string contents. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_shared(&self) -> bool {
        matches!(self.0, Storage::Shared(_))
    }
    /** Publish actual external storage for all aliases sharing this string cell. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn externalize(&self) -> EngineResult<()> {
        self.0.externalize(self.effective_encoding())
    }
    /** Report a real published external allocation. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_external(&self) -> bool {
        self.0.is_external()
    }
    /** Copy immutable compiled text into one isolate's independently mutable string cell. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn isolate_code_constant(&self) -> Self {
        let value = Self(
            Storage::flat_encoded(std::sync::Arc::from(self.as_units()), self.1),
            self.1,
        );
        value.mark_old();
        value.mark_interned();
        value
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_interned(&self) -> bool {
        self.0.interned()
    }
    /** Construct one permanent, old, internalized one-byte character cell. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn character_root(unit: u8) -> Self {
        Self(Storage::character_root(unit), true)
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_read_only_root(&self) -> bool {
        self.0.is_read_only_root()
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn can_intern_in_place(&self) -> bool {
        self.0.can_intern_in_place()
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn mark_interned(&self) -> bool {
        self.0.mark_interned()
    }
    /** Thin construction distinguishes actual cons shape from a cell already externalized. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_actual_cons(&self) -> bool {
        self.0.is_cons() && !self.is_external() && self.thin_target().is_none()
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn thin_target(&self) -> Option<Self> {
        self.0.forwarding_target().map(|target| {
            let one_byte = target.actual_one_byte();
            Self(target, one_byte)
        })
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn forward_to(&self, target: &Self) -> bool {
        self.0.forward_to(&target.0)
    }
    /** Record actual survival of this isolate's string through a full collection. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn mark_old(&self) {
        self.0.mark_old();
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn is_old(&self) -> bool {
        self.0.is_old()
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn survived_minor(&self) {
        self.0.survived_minor();
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn survived_once(&self) -> bool {
        self.0.survived_once()
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn age_reachable(
        &self,
        major: bool,
        seen: &mut std::collections::BTreeSet<usize>,
    ) {
        self.0.age_reachable(major, seen);
    }
    #[cfg(feature = "conformance")]
    pub(super) fn snapshot_id(&self) -> u64 {
        self.0.snapshot_id()
    }
    /** Compare actual backing identity without equating independently allocated strings. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn same_backing(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    /// Explicit DOM inputs are UTF-8, converted once at the VM boundary.
    pub fn from_utf8(value: &str) -> EngineResult<Self> {
        Self::from_units(value.encode_utf16().collect())
    }
    /// Limits apply before a literal or service-supplied string enters an environment.
    pub fn from_units(units: Vec<u16>) -> EngineResult<Self> {
        if units.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self::flat(&units))
    }
    /** Keep a host-requested two-byte representation even for Latin-1 content. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn from_units_two_byte(units: Vec<u16>) -> EngineResult<Self> {
        if units.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self(Storage::flat_encoded(units.into(), false), false))
    }
    /// Build from a slice under the same limit.
    pub fn from_slice(units: &[u16]) -> EngineResult<Self> {
        if units.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self::flat(units))
    }
    /// Count code units rather than Rust Unicode scalar values or UTF-8 bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Environment validation can count an empty string without decoding it.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The empty string.
    pub fn empty() -> Self {
        Self::flat(&[])
    }
    /// Wrap a fresh flat buffer, scanning it once for the Latin-1 property.
    fn flat(units: &[u16]) -> Self {
        let latin1 = units.iter().all(|unit| *unit <= LATIN1_MAX_UNIT);
        #[cfg(feature = "conformance")]
        {
            Self(Storage::flat_encoded(units.into(), latin1), latin1)
        }
        #[cfg(not(feature = "conformance"))]
        {
            Self(Storage::flat(units.into()), latin1)
        }
    }
    /** Preserve effective encoding through operations, including a thin forwarding target. */
    fn effective_encoding(&self) -> bool {
        #[cfg(feature = "conformance")]
        {
            self.0.forwarded_one_byte().unwrap_or(self.1)
        }
        #[cfg(not(feature = "conformance"))]
        {
            self.1
        }
    }
    /// Whether this string uses a one-byte representation, answered without rescanning.
    #[cfg(feature = "conformance")]
    pub fn is_latin1(&self) -> bool {
        self.effective_encoding()
    }
    /// DOMString support is incomplete; fail explicitly rather than silently replacing surrogates.
    pub fn to_dom(&self) -> EngineResult<String> {
        if self.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        char::decode_utf16(self.code_units()).collect::<Result<String, _>>().map_err(|_| {
            EngineError::unsupported(
                "The UTF-8 document boundary cannot represent an unpaired JavaScript surrogate yet.",
            )
        })
    }
    /// Text for diagnostics and keys, replacing unpaired surrogates.
    pub fn lossy(&self) -> String {
        char::decode_utf16(self.code_units())
            .map(|point| point.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    }
    /** Concatenation checks the code-unit budget before reserving output. */
    pub(in crate::v9) fn concat_will_flatten(&self, other: &Self) -> bool {
        !self.is_empty() && !other.is_empty() && self.0.concat_will_flatten(&other.0)
    }
    pub fn concat(&self, other: &Self) -> EngineResult<Self> {
        if self.len() + other.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        if other.is_empty() {
            return Ok(self.clone());
        }
        if self.is_empty() {
            return Ok(other.clone());
        }
        Ok(Self(
            self.0.concat(&other.0),
            self.effective_encoding() && other.effective_encoding(),
        ))
    }
    /** Construct the native test-only cons shape while retaining both original children. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn construct_cons(&self, other: &Self) -> EngineResult<Option<Self>> {
        let Some(length) = self.len().checked_add(other.len()) else {
            return Ok(None);
        };
        if length < 13 {
            return Ok(None);
        }
        if length > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        let storage = self.0.concat(&other.0);
        if !storage.is_cons() {
            return Err(EngineError::unsupported("string rope depth limit"));
        }
        Ok(Some(Self(
            storage,
            self.effective_encoding() && other.effective_encoding(),
        )))
    }
    /** Construct an actual suffix view whose backing retains its parent. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn construct_slice(&self, start: usize) -> EngineResult<Option<Self>> {
        if start == 0 || start >= self.len() || self.len() - start < 13 {
            return Ok(None);
        }
        let Some(storage) = self.0.sliced(start) else {
            return Err(EngineError::unsupported("string rope depth limit"));
        };
        Ok(Some(Self(storage, self.effective_encoding())))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn slice_parts(&self) -> Option<(Self, usize, usize)> {
        self.0
            .slice_parts()
            .map(|(parent, start, length)| (Self(parent, self.effective_encoding()), start, length))
    }
    /** Restore a checked retained range without copying its parent. */
    #[cfg(feature = "conformance")]
    pub(super) fn from_slice_range(parent: &Self, start: usize, length: usize) -> Option<Self> {
        let end = start.checked_add(length)?;
        parent
            .0
            .sliced_range(start, end)
            .map(|storage| Self(storage, parent.effective_encoding()))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn cons_parts(&self) -> Option<(Self, Self)> {
        self.0
            .cons_parts(self.effective_encoding())
            .map(|(left, right)| {
                let l = left.actual_one_byte();
                let r = right.actual_one_byte();
                (Self(left, l), Self(right, r))
            })
    }
    /** Materialize the cached real flat child while preserving direct and sliced identities. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn flatten_for_intrinsic(&self) -> EngineResult<Self> {
        if self.0.needs_intrinsic_flatten() && self.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self(
            self.0.flatten_intrinsic(self.effective_encoding()),
            self.effective_encoding(),
        ))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn flattened_cons_child(&self) -> Option<Self> {
        self.0
            .flattened_cons_child(self.effective_encoding())
            .map(|child| Self(child, self.effective_encoding()))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn from_flattened_cons(child: &Self) -> EngineResult<Option<Self>> {
        let empty = Self::from_utf8("")?;
        let Some(cons) = empty.construct_cons(child)? else {
            return Ok(None);
        };
        if !cons.0.restore_flattened_cons(&child.0) {
            return Ok(None);
        }
        Ok(Some(cons))
    }
    #[cfg(feature = "conformance")]
    pub(super) fn flattened_repeat_parts(&self) -> Option<(Self, usize, Self)> {
        self.0
            .flattened_repeat_parts(self.effective_encoding())
            .map(|(parent, count, flat)| {
                let one_byte = parent.actual_one_byte();
                (
                    Self(parent, one_byte),
                    count,
                    Self(flat, self.effective_encoding()),
                )
            })
    }
    #[cfg(feature = "conformance")]
    pub(super) fn from_flattened_repeat(
        parent: &Self,
        count: usize,
        flat: &Self,
    ) -> EngineResult<Option<Self>> {
        let repeated = parent.repeat(count)?;
        if repeated.len() != flat.len()
            || repeated != *flat
            || !repeated.0.restore_flattened_repeat(&flat.0)
        {
            return Ok(None);
        }
        Ok(Some(repeated))
    }
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn externalizable_cons_parts(&self) -> Option<(Self, Self)> {
        self.0.externalizable_cons_parts().map(|(left, right)| {
            let l = left.actual_one_byte();
            let r = right.actual_one_byte();
            (Self(left, l), Self(right, r))
        })
    }
    /// Whether the string equals ASCII text.
    pub fn is(&self, text: &str) -> bool {
        self.len() == text.len()
            && self
                .code_units()
                .zip(text.bytes())
                .all(|(a, b)| a == b as u16)
    }
    /** Native accounting observes actual shared allocations without forcing a flat string buffer. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn storage_allocations(&self, visit: impl FnMut(bool, usize, usize, usize)) {
        self.0.allocations(visit);
    }
}

/// Largest integer represented by the conformance VM's tagged small-integer storage.
#[cfg(feature = "conformance")]
pub(in crate::v9) const SMI_MAX: i32 = (1 << 30) - 1;

/// Values: primitives and identities. `Hole` marks dead-zone bindings and array holes only.
#[derive(Clone, Debug)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    /// A tracked tagged small integer, retaining the distinction from heap numbers.
    #[cfg(feature = "conformance")]
    Smi(f64),
    /// A shared Number primitive retains actual shared heap backing identity.
    #[cfg(feature = "conformance")]
    SharedNumber(std::sync::Arc<SharedNumberData>),
    /// A local Number primitive retains a traced heap allocation.
    #[cfg(feature = "conformance")]
    LocalNumber(std::sync::Arc<SharedNumberData>),
    String(JsString),
    /// An object in the page heap or an intrinsic.
    Object(ObjectId),
    /// A symbol identity.
    Symbol(u32),
    /// Inline BigInts retain the existing compact public representation.
    BigInt(i128),
    /// Wider mathematical integers own immutable, bounded limbs.
    BigIntWide(BigInteger),
    /// Never visible to scripts: an uninitialized binding or a missing array element.
    #[doc(hidden)]
    Hole,
}

impl PartialEq for JsValue {
    fn eq(&self, other: &Self) -> bool {
        use JsValue::*;
        match (self, other) {
            (Undefined, Undefined) | (Null, Null) | (Hole, Hole) => true,
            (Boolean(a), Boolean(b)) => a == b,
            (Number(a), Number(b)) => a == b,
            #[cfg(feature = "conformance")]
            (Smi(a), Smi(b) | Number(b)) | (Number(b), Smi(a)) => a == b,
            #[cfg(feature = "conformance")]
            (SharedNumber(a) | LocalNumber(a), Number(b) | Smi(b))
            | (Number(b) | Smi(b), SharedNumber(a) | LocalNumber(a)) => a.value == *b,
            #[cfg(feature = "conformance")]
            (SharedNumber(a) | LocalNumber(a), SharedNumber(b) | LocalNumber(b)) => {
                a.value == b.value
            }
            (String(a), String(b)) => a == b,
            (Object(a), Object(b)) => a == b,
            (Symbol(a), Symbol(b)) => a == b,
            (BigInt(a), BigInt(b)) => a == b,
            (BigIntWide(a), BigIntWide(b)) => a == b,
            _ => false,
        }
    }
}

/// Whether an object is `document.all`, the one object with [[IsHTMLDDA]]: ToBoolean is false,
/// typeof is "undefined" and it loosely equals null and undefined (ECMA-262 B.3.6). Pages compare
/// against it to tell a missing value from a present one (YouTube's attribute sanitizer does).
pub(in crate::v9) fn is_html_dda(id: u32) -> bool {
    id == crate::v9::vm::realm::realm().ids.document_all
}

impl JsValue {
    /// Return a primitive Number without coercing non-number values.
    pub fn number_value(&self) -> Option<f64> {
        match self {
            #[cfg(feature = "conformance")]
            Self::Smi(value) => Some(*value),
            Self::Number(value) => Some(*value),
            #[cfg(feature = "conformance")]
            Self::SharedNumber(value) | Self::LocalNumber(value) => Some(value.value),
            _ => None,
        }
    }
    /// Input strings remain literal values; entering one never evaluates source.
    pub fn string(value: &str) -> EngineResult<Self> {
        Ok(Self::String(JsString::from_utf8(value)?))
    }
    /// Truthiness never converts a string to a number or invokes native code.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null | Self::Hole => false,
            Self::Boolean(value) => *value,
            #[cfg(feature = "conformance")]
            Self::Smi(value) => *value != 0.0 && !value.is_nan(),
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            #[cfg(feature = "conformance")]
            Self::SharedNumber(value) | Self::LocalNumber(value) => {
                value.value != 0.0 && !value.is_nan()
            }
            Self::String(value) => !value.is_empty(),
            Self::BigInt(value) => *value != 0,
            Self::BigIntWide(value) => !value.is_zero(),
            Self::Object(id) => !is_html_dda(*id),
            Self::Symbol(_) => true,
        }
    }
    /// Strings held by the value count against aggregate environment budgets.
    pub fn string_units(&self) -> usize {
        match self {
            Self::String(value) => value.len(),
            Self::BigIntWide(value) => value.units(),
            _ => 0,
        }
    }
    /** Every mathematical integer result returns to the inline representation whenever it fits. */
    pub(in crate::v9) fn bigint(value: BigInteger) -> Self {
        value
            .to_i128()
            .map_or_else(|| Self::BigIntWide(value), Self::BigInt)
    }
    /** Numeric operators recognize both storage forms as the one ECMAScript BigInt type. */
    pub(in crate::v9) fn is_bigint(&self) -> bool {
        matches!(self, Self::BigInt(_) | Self::BigIntWide(_))
    }
    /** Shared immutable limbs make numeric dispatch independent of the storage form. */
    pub(in crate::v9) fn bigint_value(&self) -> Option<BigInteger> {
        match self {
            Self::BigInt(value) => Some(BigInteger::from_i128(*value)),
            Self::BigIntWide(value) => Some(value.clone()),
            _ => None,
        }
    }
    /// Null or undefined.
    pub fn nullish(&self) -> bool {
        matches!(self, Self::Undefined | Self::Null)
    }
    /// The object identity, if any.
    pub fn object(&self) -> Option<ObjectId> {
        match self {
            Self::Object(id) => Some(*id),
            _ => None,
        }
    }
    /// A primitive as document text; objects and symbols need the VM's conversions.
    pub fn to_dom(&self) -> EngineResult<String> {
        Ok(match self {
            Self::String(text) => return text.to_dom(),
            #[cfg(feature = "conformance")]
            Self::Smi(value) => number_to_string(*value),
            Self::Number(value) => number_to_string(*value),
            #[cfg(feature = "conformance")]
            Self::SharedNumber(value) | Self::LocalNumber(value) => number_to_string(value.value),
            Self::Boolean(value) => value.to_string(),
            Self::BigInt(value) => value.to_string(),
            Self::BigIntWide(value) => value.to_string(),
            Self::Null => "null".into(),
            Self::Undefined | Self::Hole => "undefined".into(),
            Self::Object(_) | Self::Symbol(_) => {
                return Err(EngineError::unsupported(
                    "Converting an object to text needs the V9 VM.",
                ));
            }
        })
    }
    /// A primitive's ToNumber; objects and symbols give NaN here.
    pub fn to_number(&self) -> f64 {
        match self {
            #[cfg(feature = "conformance")]
            Self::Smi(value) => *value,
            Self::Number(value) => *value,
            #[cfg(feature = "conformance")]
            Self::SharedNumber(value) | Self::LocalNumber(value) => value.value,
            Self::Boolean(value) => f64::from(u8::from(*value)),
            Self::Null => 0.0,
            Self::String(text) => char::decode_utf16(text.code_units())
                .collect::<Result<String, _>>()
                .map_or(f64::NAN, |text| string_number(&text)),
            _ => f64::NAN,
        }
    }
    /// Loose equality (==) between values that need no script conversion: identities compare
    /// as identities; an object against a primitive needs the VM and is refused here.
    pub fn abstract_equal(&self, other: &Self) -> EngineResult<bool> {
        #[cfg(feature = "conformance")]
        {
            if let Self::SharedNumber(value) | Self::LocalNumber(value) = self {
                return Self::Number(value.value).abstract_equal(other);
            }
            if let Self::SharedNumber(value) | Self::LocalNumber(value) = other {
                return self.abstract_equal(&Self::Number(value.value));
            }
        }

        #[cfg(feature = "conformance")]
        {
            if let Self::Smi(value) = self {
                return Self::Number(*value).abstract_equal(other);
            }
            if let Self::Smi(value) = other {
                return self.abstract_equal(&Self::Number(*value));
            }
        }
        use JsValue::*;
        Ok(match (self, other) {
            (Undefined | Null | Hole, Undefined | Null | Hole) => true,
            (Undefined | Null | Hole, _) | (_, Undefined | Null | Hole) => false,
            (Object(_) | Symbol(_), Object(_) | Symbol(_)) => self == other,
            (BigIntWide(big), Number(number)) | (Number(number), BigIntWide(big)) => {
                BigInteger::from_f64(*number).is_some_and(|number| *big == number)
            }
            (BigInt(big), Number(number)) | (Number(number), BigInt(big)) => {
                bigint_equals_number(*big, *number)
            }
            (Object(_), _) | (_, Object(_)) => {
                return Err(EngineError::unsupported(
                    "Comparing an object with a primitive needs the V9 VM.",
                ));
            }
            (Number(_) | String(_) | Boolean(_), Number(_) | String(_) | Boolean(_))
                if std::mem::discriminant(self) != std::mem::discriminant(other) =>
            {
                self.to_number() == other.to_number()
            }
            _ => self.strict_equal(other),
        })
    }
    /// Strict equality (===).
    pub fn strict_equal(&self, other: &Self) -> bool {
        match (self, other) {
            #[cfg(feature = "conformance")]
            (Self::Smi(a), Self::Smi(b) | Self::Number(b)) | (Self::Number(b), Self::Smi(a)) => {
                a == b
            }
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::BigInt(a), Self::BigIntWide(b)) | (Self::BigIntWide(b), Self::BigInt(a)) => {
                b.to_i128() == Some(*a)
            }
            (Self::Undefined | Self::Hole, Self::Undefined | Self::Hole) => true,
            _ => self == other,
        }
    }
    /// SameValueZero, used by includes, Map and Set.
    pub fn same_value_zero(&self, other: &Self) -> bool {
        #[cfg(feature = "conformance")]
        {
            if let Self::Smi(value) = self {
                return Self::Number(*value).same_value_zero(other);
            }
            if let Self::Smi(value) = other {
                return self.same_value_zero(&Self::Number(*value));
            }
        }
        #[cfg(feature = "conformance")]
        {
            if let Self::SharedNumber(value) | Self::LocalNumber(value) = self {
                return Self::Number(value.value).same_value_zero(other);
            }
            if let Self::SharedNumber(value) | Self::LocalNumber(value) = other {
                return self.same_value_zero(&Self::Number(value.value));
            }
        }

        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a == b || a.is_nan() && b.is_nan(),
            _ => self.strict_equal(other),
        }
    }
    /// SameValue, used by Object.is.
    pub fn same_value(&self, other: &Self) -> bool {
        #[cfg(feature = "conformance")]
        {
            if let Self::Smi(value) = self {
                return Self::Number(*value).same_value(other);
            }
            if let Self::Smi(value) = other {
                return self.same_value(&Self::Number(*value));
            }
        }
        #[cfg(feature = "conformance")]
        {
            if let Self::SharedNumber(value) | Self::LocalNumber(value) = self {
                return Self::Number(value.value).same_value(other);
            }
            if let Self::SharedNumber(value) | Self::LocalNumber(value) = other {
                return self.same_value(&Self::Number(value.value));
            }
        }

        match (self, other) {
            (Self::Number(a), Self::Number(b)) => {
                a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan()
            }
            _ => self.strict_equal(other),
        }
    }
}

/// Whether a BigInt and a Number denote the same mathematical value (`1n == 1`).
pub(in crate::v9) fn bigint_equals_number(big: i128, number: f64) -> bool {
    BigInteger::from_f64(number).and_then(|value| value.to_i128()) == Some(big)
}
