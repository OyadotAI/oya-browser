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
pub use string::CodePointCursor;

/** Heap object identities; intrinsic objects live in a reserved high range. */
pub type ObjectId = u32;

/// Code units at or below this value are Latin-1, the widest unit a one-byte string can hold.
const LATIN1_MAX_UNIT: u16 = 0xFF;

/// JavaScript strings keep UTF-16 code units, including lone surrogates, and share their buffer.
/// The second field records whether every unit fits Latin-1; it is computed once at construction
/// (a copy already touches every unit), carried through clones, and combined in O(1) by concatenation.
#[derive(Clone)]
pub struct JsString(Storage, bool);

impl PartialEq for JsString {
    /** Strings compare UTF-16 content, independently of flat or concatenated storage. */
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && (self.0.same(&other.0) || self.as_units() == other.as_units())
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
        self.as_units().cmp(other.as_units())
    }
}
impl std::hash::Hash for JsString {
    /** Preserve exactly the slice hash bytes used by the former Arc-backed representation. */
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.as_units(), state);
    }
}

impl std::fmt::Debug for JsString {
    /** Debug output shows the text, replacing unpaired surrogates. */
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}", String::from_utf16_lossy(self.as_units()))
    }
}

impl JsString {
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
        Self(Storage::Flat(units.into()), latin1)
    }
    /// Whether every code unit fits Latin-1; answered from the flag, never by scanning.
    #[cfg(feature = "conformance")]
    pub fn is_latin1(&self) -> bool {
        self.1
    }
    /// DOMString support is incomplete; fail explicitly rather than silently replacing surrogates.
    pub fn to_dom(&self) -> EngineResult<String> {
        String::from_utf16(self.as_units()).map_err(|_| {
            EngineError::unsupported(
                "The UTF-8 document boundary cannot represent an unpaired JavaScript surrogate yet.",
            )
        })
    }
    /// Text for diagnostics and keys, replacing unpaired surrogates.
    pub fn lossy(&self) -> String {
        String::from_utf16_lossy(self.as_units())
    }
    /** Concatenation checks the code-unit budget before reserving output. */
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
        Ok(Self(self.0.concat(&other.0), self.1 && other.1))
    }
    /// Whether the string equals ASCII text.
    pub fn is(&self, text: &str) -> bool {
        self.len() == text.len()
            && self
                .as_units()
                .iter()
                .zip(text.bytes())
                .all(|(a, b)| *a == b as u16)
    }
    /** Native accounting observes actual shared allocations without forcing a flat string buffer. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn storage_allocations(&self, visit: impl FnMut(bool, usize, usize, usize)) {
        self.0.allocations(visit);
    }
}

/// Values: primitives and identities. `Hole` marks dead-zone bindings and array holes only.
#[derive(Clone, Debug, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
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

/// Whether an object is `document.all`, the one object with [[IsHTMLDDA]]: ToBoolean is false,
/// typeof is "undefined" and it loosely equals null and undefined (ECMA-262 B.3.6). Pages compare
/// against it to tell a missing value from a present one (YouTube's attribute sanitizer does).
pub(in crate::v9) fn is_html_dda(id: u32) -> bool {
    id == crate::v9::vm::realm::realm().ids.document_all
}

impl JsValue {
    /// Input strings remain literal values; entering one never evaluates source.
    pub fn string(value: &str) -> EngineResult<Self> {
        Ok(Self::String(JsString::from_utf8(value)?))
    }
    /// Truthiness never converts a string to a number or invokes native code.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null | Self::Hole => false,
            Self::Boolean(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
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
            Self::Number(value) => number_to_string(*value),
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
            Self::Number(value) => *value,
            Self::Boolean(value) => f64::from(u8::from(*value)),
            Self::Null => 0.0,
            Self::String(text) => {
                String::from_utf16(text.as_units()).map_or(f64::NAN, |text| string_number(&text))
            }
            _ => f64::NAN,
        }
    }
    /// Loose equality (==) between values that need no script conversion: identities compare
    /// as identities; an object against a primitive needs the VM and is refused here.
    pub fn abstract_equal(&self, other: &Self) -> EngineResult<bool> {
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
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a == b || a.is_nan() && b.is_nan(),
            _ => self.strict_equal(other),
        }
    }
    /// SameValue, used by Object.is.
    pub fn same_value(&self, other: &Self) -> bool {
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
