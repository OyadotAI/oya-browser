/** ECMAScript values: UTF-16 strings, binary64 numbers and arena identities, never native handles. */
mod number;
mod string;
mod wire;
use crate::{
    constants::JS_STRING_UNITS,
    protocol::{EngineError, EngineResult},
};
pub(super) use number::{
    literal, number_to_string, radix_string, string_number, to_int32, to_uint32,
};
use std::sync::Arc;
pub use string::CodePointCursor;

/** Heap object identities; intrinsic objects live in a reserved high range. */
pub type ObjectId = u32;

/// JavaScript strings keep UTF-16 code units, including lone surrogates, and share their buffer.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JsString(Arc<[u16]>);

impl std::fmt::Debug for JsString {
    /** Debug output shows the text, replacing unpaired surrogates. */
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}", String::from_utf16_lossy(&self.0))
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
        Ok(Self(units.into()))
    }
    /// Build from a slice under the same limit.
    pub fn from_slice(units: &[u16]) -> EngineResult<Self> {
        if units.len() > JS_STRING_UNITS {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                JS_STRING_UNITS,
            ));
        }
        Ok(Self(units.into()))
    }
    /// Count code units rather than Rust Unicode scalar values or UTF-8 bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    /// Environment validation can count an empty string without decoding it.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// The empty string.
    pub fn empty() -> Self {
        Self(Arc::from(Vec::new()))
    }
    /// DOMString support is incomplete; fail explicitly rather than silently replacing surrogates.
    pub fn to_dom(&self) -> EngineResult<String> {
        String::from_utf16(&self.0).map_err(|_| {
            EngineError::unsupported(
                "The UTF-8 document boundary cannot represent an unpaired JavaScript surrogate yet.",
            )
        })
    }
    /// Text for diagnostics and keys, replacing unpaired surrogates.
    pub fn lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
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
        let mut units = Vec::with_capacity(self.len() + other.len());
        units.extend_from_slice(&self.0);
        units.extend_from_slice(&other.0);
        Ok(Self(units.into()))
    }
    /// Whether the string equals ASCII text.
    pub fn is(&self, text: &str) -> bool {
        self.0.len() == text.len() && self.0.iter().zip(text.bytes()).all(|(a, b)| *a == b as u16)
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
    /// A BigInt. V9 bounds BigInts to 128 bits (RangeError beyond), which covers the 64-bit
    /// integer arithmetic real pages do (protobuf and Closure int64) without arbitrary precision.
    BigInt(i128),
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
            Self::Object(id) => !is_html_dda(*id),
            Self::Symbol(_) => true,
        }
    }
    /// Strings held by the value count against aggregate environment budgets.
    pub fn string_units(&self) -> usize {
        match self {
            Self::String(value) => value.len(),
            _ => 0,
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
    number.is_finite() && number.trunc() == number && big as f64 == number && number as i128 == big
}
