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

impl Serialize for JsValue {
    /** Plain JSON where it is unambiguous; tagged maps otherwise. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
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
            Self::String(value) => match String::from_utf16(value.as_units()) {
                Ok(text) => serializer.serialize_str(&text),
                Err(_) => {
                    let mut map = serializer.serialize_map(Some(1))?;
                    map.serialize_entry("w", value.as_units())?;
                    map.end()
                }
            },
            Self::Boolean(value) => serializer.serialize_bool(*value),
            Self::Null => serializer.serialize_unit(),
            Self::Undefined => serializer.serialize_map(Some(0))?.end(),
            Self::Object(id) => single(serializer, "o", id),
            Self::Symbol(id) => single(serializer, "y", id),
            Self::BigInt(value) => single(serializer, "b", &value.to_string()),
            Self::Hole => single(serializer, "h", &0u8),
        }
    }
}

/** A one-entry tagged map. */
fn single<S: Serializer, T: Serialize>(
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
        JsString::from_utf8(value)
            .map(JsValue::String)
            .map_err(|error| E::custom(error.message))
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
            "w" => JsString::from_units(map.next_value::<Vec<u16>>()?)
                .map(JsValue::String)
                .map_err(|error| A::Error::custom(error.message))?,
            "o" => JsValue::Object(map.next_value()?),
            "y" => JsValue::Symbol(map.next_value()?),
            "b" => JsValue::BigInt(
                map.next_value::<String>()?
                    .parse()
                    .map_err(|_| A::Error::custom("invalid bigint"))?,
            ),
            "h" => {
                map.next_value::<u8>()?;
                JsValue::Hole
            }
            _ => return Err(A::Error::custom("unknown value tag")),
        };
        if map.next_key::<String>()?.is_some() {
            return Err(A::Error::custom("extra value field"));
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
