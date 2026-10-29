/** Type conversions and operators: ToPrimitive, ToNumber, ToString, equality and comparison. */
use super::data::Class;
use super::heap::{Key, Object};
use super::realm::{realm, symbol};
use super::{Res, Vm};
use crate::v9::value::{JsString, JsValue, ObjectId, number_to_string, string_number};

/** ToPrimitive's preferred type. */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::v9) enum Hint {
    Default,
    Number,
    String,
}

#[allow(clippy::wrong_self_convention)]
impl Vm<'_> {
    /** ToPrimitive: Symbol.toPrimitive, then valueOf/toString in hint order. */
    pub fn to_primitive(&mut self, value: &JsValue, hint: Hint) -> Res<JsValue> {
        let JsValue::Object(id) = value else {
            return Ok(value.clone());
        };
        let exotic = self.get(value, &Key::Sym(symbol::TO_PRIMITIVE))?;
        if !exotic.nullish() {
            let name = match hint {
                Hint::Default => "default",
                Hint::Number => "number",
                Hint::String => "string",
            };
            let result = self.call(exotic, value.clone(), vec![JsValue::string(name)?])?;
            if matches!(result, JsValue::Object(_)) {
                return Err(self.type_error("Cannot convert object to primitive value"));
            }
            return Ok(result);
        }
        let order = if hint == Hint::String {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };
        for name in order {
            let method = self.get(value, &Key::from(name))?;
            if self.is_callable(&method) {
                let result = self.call(method, value.clone(), Vec::new())?;
                if !matches!(result, JsValue::Object(_)) {
                    return Ok(result);
                }
            }
        }
        let _ = id;
        Err(self.type_error("Cannot convert object to primitive value"))
    }
    /** ToNumber. */
    pub fn to_number(&mut self, value: &JsValue) -> Res<f64> {
        Ok(match value {
            #[cfg(feature = "conformance")]
            JsValue::Smi(value) => *value,
            JsValue::Number(value) => *value,
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(value) | JsValue::LocalNumber(value) => value.value,
            JsValue::Undefined | JsValue::Hole => f64::NAN,
            JsValue::Null => 0.0,
            JsValue::Boolean(value) => f64::from(u8::from(*value)),
            JsValue::String(text) => string_to_number(self.materialize_string(text)?),
            JsValue::Symbol(_) => {
                return Err(self.type_error("Cannot convert a Symbol value to a number"));
            }
            JsValue::BigInt(_) | JsValue::BigIntWide(_) => {
                return Err(self.type_error("Cannot convert a BigInt value to a number"));
            }
            JsValue::Object(_) => {
                let primitive = self.to_primitive(value, Hint::Number)?;
                return self.to_number(&primitive);
            }
        })
    }
    /** ToString. */
    pub fn to_string(&mut self, value: &JsValue) -> Res<JsString> {
        let text = match value {
            JsValue::String(text) => text.clone(),
            #[cfg(feature = "conformance")]
            JsValue::Smi(value) => JsString::from_utf8(&number_to_string(*value))?,
            JsValue::Number(value) => JsString::from_utf8(&number_to_string(*value))?,
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(value) | JsValue::LocalNumber(value) => {
                JsString::from_utf8(&number_to_string(value.value))?
            }
            JsValue::Boolean(true) => JsString::from_utf8("true")?,
            JsValue::Boolean(false) => JsString::from_utf8("false")?,
            JsValue::Null => JsString::from_utf8("null")?,
            JsValue::BigInt(value) => JsString::from_utf8(&value.to_string())?,
            JsValue::BigIntWide(value) => {
                self.charge(value.format_work(10))?;
                JsString::from_utf8(&value.to_string())?
            }
            JsValue::Undefined | JsValue::Hole => JsString::from_utf8("undefined")?,
            JsValue::Symbol(_) => {
                return Err(self.type_error("Cannot convert a Symbol value to a string"));
            }
            JsValue::Object(_) => {
                let primitive = self.to_primitive(value, Hint::String)?;
                return self.to_string(&primitive);
            }
        };
        self.track_string(&text);
        Ok(text)
    }
    /** ToString as Rust text, replacing lone surrogates. */
    pub fn to_text(&mut self, value: &JsValue) -> Res<String> {
        let text = self.to_string(value)?;
        Ok(String::from_utf16_lossy(self.materialize_string(&text)?))
    }
    /** ToPropertyKey. */
    pub fn to_key(&mut self, value: &JsValue) -> Res<Key> {
        Ok(match value {
            JsValue::Symbol(id) => Key::Sym(*id),
            JsValue::String(text) => Key::Str(text.clone()),
            JsValue::Number(number)
                if number.fract() == 0.0 && *number >= 0.0 && *number < 1e15 =>
            {
                Key::Str(JsString::from_utf8(&format!("{}", *number as u64))?)
            }
            JsValue::Object(_) => {
                let primitive = self.to_primitive(value, Hint::String)?;
                return self.to_key(&primitive);
            }
            other => Key::Str(self.to_string(other)?),
        })
    }
    /** ToObject: primitives get wrapper objects. */
    pub fn to_object(&mut self, value: &JsValue) -> Res<ObjectId> {
        let ids = &realm().ids;
        let (class, proto) = match value {
            JsValue::Object(id) => return Ok(*id),
            JsValue::Undefined | JsValue::Null | JsValue::Hole => {
                return Err(self.type_error("Cannot convert undefined or null to object"));
            }
            JsValue::Boolean(value) => (Class::Boolean(*value), ids.boolean_prototype),
            #[cfg(feature = "conformance")]
            JsValue::Smi(value) => (Class::Number(*value), ids.number_prototype),
            JsValue::Number(value) => (Class::Number(*value), ids.number_prototype),
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(value) | JsValue::LocalNumber(value) => {
                (Class::Number(value.value), ids.number_prototype)
            }
            JsValue::String(text) => (Class::String(text.clone()), ids.string_prototype),
            JsValue::Symbol(id) => (Class::Symbol(*id), ids.symbol_prototype),
            JsValue::BigInt(value) => (Class::BigInt(*value), ids.bigint_prototype),
            JsValue::BigIntWide(value) => (Class::BigIntWide(value.clone()), ids.bigint_prototype),
        };
        #[cfg(feature = "conformance")]
        let mark = self.pins.len();
        #[cfg(feature = "conformance")]
        let pinned_shared =
            if let JsValue::SharedNumber(value) | JsValue::LocalNumber(value) = value {
                self.pin(JsValue::Object(value.root));
                true
            } else {
                false
            };
        let allocated = self.alloc(Object::new(class, Some(proto)));
        #[cfg(feature = "conformance")]
        if pinned_shared {
            self.pins.remove(mark);
        }
        let id = allocated?;
        #[cfg(feature = "conformance")]
        if matches!(value, JsValue::SharedNumber(_) | JsValue::LocalNumber(_)) {
            self.heap.put(
                id,
                Key::private("shared-number-value"),
                super::heap::Prop::data(value.clone()),
            );
        }
        Ok(id)
    }
    /** ToIntegerOrInfinity. */
    pub fn to_integer(&mut self, value: &JsValue) -> Res<f64> {
        let number = self.to_number(value)?;
        Ok(if number.is_nan() { 0.0 } else { number.trunc() })
    }
    /** A relative index clamped to `0..=length`, as slice and friends take it. */
    pub fn relative_index(&mut self, value: &JsValue, length: usize, default: usize) -> Res<usize> {
        if matches!(value, JsValue::Undefined) {
            return Ok(default);
        }
        let integer = self.to_integer(value)?;
        let length = length as f64;
        Ok(if integer < 0.0 {
            (length + integer).max(0.0)
        } else {
            integer.min(length)
        } as usize)
    }
    /** typeof. */
    pub fn type_of(&self, value: &JsValue) -> &'static str {
        match value {
            JsValue::Undefined | JsValue::Hole => "undefined",
            JsValue::Null => "object",
            JsValue::Boolean(_) => "boolean",
            #[cfg(feature = "conformance")]
            JsValue::Smi(_) => "number",
            JsValue::Number(_) => "number",
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(_) | JsValue::LocalNumber(_) => "number",
            JsValue::String(_) => "string",
            JsValue::Symbol(_) => "symbol",
            JsValue::BigInt(_) | JsValue::BigIntWide(_) => "bigint",
            JsValue::Object(id) if crate::v9::value::is_html_dda(*id) => "undefined",
            JsValue::Object(_) => {
                if self.is_callable(value) {
                    "function"
                } else {
                    "object"
                }
            }
        }
    }
    /** Abstract (loose) equality. */
    pub fn loose_equal(&mut self, left: &JsValue, right: &JsValue) -> Res<bool> {
        #[cfg(feature = "conformance")]
        {
            if let JsValue::Smi(value) = left {
                return self.loose_equal(&JsValue::Number(*value), right);
            }
            if let JsValue::Smi(value) = right {
                return self.loose_equal(left, &JsValue::Number(*value));
            }
            if let JsValue::SharedNumber(value) | JsValue::LocalNumber(value) = left {
                return self.loose_equal(&JsValue::Number(value.value), right);
            }
            if let JsValue::SharedNumber(value) | JsValue::LocalNumber(value) = right {
                return self.loose_equal(left, &JsValue::Number(value.value));
            }
        }
        use JsValue::*;
        Ok(match (left, right) {
            (Undefined | Null | Hole, Undefined | Null | Hole) => true,
            (Undefined | Null | Hole, Object(id)) | (Object(id), Undefined | Null | Hole) => {
                crate::v9::value::is_html_dda(*id)
            }
            (Undefined | Null | Hole, _) | (_, Undefined | Null | Hole) => false,
            (BigInt(big), Number(number)) | (Number(number), BigInt(big)) => {
                crate::v9::value::bigint_equals_number(*big, *number)
            }
            (BigInt(big), String(text)) | (String(text), BigInt(big)) => {
                crate::v9::value::BigInteger::parse(&text.lossy())
                    .ok()
                    .and_then(|value| value.to_i128())
                    == Some(*big)
            }
            (BigIntWide(big), String(text)) | (String(text), BigIntWide(big)) => {
                crate::v9::value::BigInteger::parse(&text.lossy())
                    .ok()
                    .is_some_and(|value| value == *big)
            }
            (BigIntWide(big), Number(number)) | (Number(number), BigIntWide(big)) => {
                crate::v9::value::BigInteger::from_f64(*number).is_some_and(|value| value == *big)
            }
            (Number(_), String(_)) | (String(_), Number(_)) => {
                let left = self.to_number(left)?;
                let right = self.to_number(right)?;
                left == right
            }
            (Boolean(_), _) => {
                let left = Number(self.to_number(left)?);
                return self.loose_equal(&left, right);
            }
            (_, Boolean(_)) => {
                let right = Number(self.to_number(right)?);
                return self.loose_equal(left, &right);
            }
            (Object(_), Object(_)) => left == right,
            (Object(_), _) => {
                let left = self.to_primitive(left, Hint::Default)?;
                return self.loose_equal(&left, right);
            }
            (_, Object(_)) => {
                let right = self.to_primitive(right, Hint::Default)?;
                return self.loose_equal(left, &right);
            }
            _ => left.strict_equal(right),
        })
    }
    /** `left < right`; None means undefined (a NaN was involved). */
    pub fn less_than(
        &mut self,
        left: &JsValue,
        right: &JsValue,
        left_first: bool,
    ) -> Res<Option<bool>> {
        let (left, right) = if left_first {
            let left = self.to_primitive(left, Hint::Number)?;
            let right = self.to_primitive(right, Hint::Number)?;
            (left, right)
        } else {
            let right = self.to_primitive(right, Hint::Number)?;
            let left = self.to_primitive(left, Hint::Number)?;
            (left, right)
        };
        if let (JsValue::String(left), JsValue::String(right)) = (&left, &right) {
            return Ok(Some(left < right));
        }
        if left.is_bigint() || right.is_bigint() {
            return self.bigint_less_than(&left, &right);
        }
        let left = self.to_number(&left)?;
        let right = self.to_number(&right)?;
        if left.is_nan() || right.is_nan() {
            return Ok(None);
        }
        Ok(Some(left < right))
    }
    /** The `+` operator. */
    pub fn add(&mut self, left: &JsValue, right: &JsValue) -> Res<JsValue> {
        if let (JsValue::Number(left), JsValue::Number(right)) = (left, right) {
            return Ok(JsValue::Number(left + right));
        }
        if let (JsValue::String(left), JsValue::String(right)) = (left, right) {
            return Ok(JsValue::String(self.concat_strings(left, right)?));
        }
        let left = self.to_primitive(left, Hint::Default)?;
        let right = self.to_primitive(right, Hint::Default)?;
        if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
            let left = self.to_string(&left)?;
            let right = self.to_string(&right)?;
            return Ok(JsValue::String(self.concat_strings(&left, &right)?));
        }
        if left.is_bigint() || right.is_bigint() {
            return self.bigint_binary(super::super::compile::Op::Add, &left, &right);
        }
        Ok(JsValue::Number(
            self.to_number(&left)? + self.to_number(&right)?,
        ))
    }
    /** `instanceof`, honoring Symbol.hasInstance and bound functions. */
    pub fn instance_of(&mut self, value: &JsValue, constructor: &JsValue) -> Res<bool> {
        if !matches!(constructor, JsValue::Object(_)) {
            return Err(self.type_error("Right-hand side of 'instanceof' is not an object"));
        }
        let custom = self.get(constructor, &Key::Sym(symbol::HAS_INSTANCE))?;
        if !custom.nullish() {
            let result = self.call(custom, constructor.clone(), vec![value.clone()])?;
            return Ok(result.truthy());
        }
        if !self.is_callable(constructor) {
            return Err(self.type_error("Right-hand side of 'instanceof' is not callable"));
        }
        self.ordinary_has_instance(constructor, value)
    }
}

/** StringToNumber over UTF-16. */
pub(in crate::v9) fn string_to_number(units: &[u16]) -> f64 {
    match String::from_utf16(units) {
        Ok(text) => string_number(&text),
        Err(_) => f64::NAN,
    }
}
