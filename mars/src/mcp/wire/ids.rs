/** Typed external IDs remain separate from native request IDs and normalize exact integral spellings. */
use crate::constants::{ID_BYTES, VM_MAX_SAFE_LENGTH};
use serde_json::Value;
/** Numeric and string IDs with the same visible digits must remain distinct. */
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::mcp) enum IdKey {
    Text(String),
    Number(i64),
}
/** Keep the original JSON value for replies and a canonical key for duplicate/cancellation checks. */
#[derive(Clone, Debug)]
pub(in crate::mcp) struct ExternalId {
    /// Replies preserve the original string-versus-number type and decoded value.
    pub value: Value,
    /// Equivalent integral numeric spellings share one whole-session identity.
    pub key: IdKey,
}
impl ExternalId {
    /** Safe integer validation uses the original token so fractional rounding cannot fabricate an integral ID. */
    pub fn parse(value: &Value, raw: Option<&str>) -> Option<Self> {
        let key = match value {
            Value::String(text) if text.len() <= ID_BYTES => IdKey::Text(text.clone()),
            Value::Number(_) => IdKey::Number(integer(raw?)?),
            _ => return None,
        };
        Some(Self {
            value: value.clone(),
            key,
        })
    }
}
/** Decimal scaling checks mathematical integrality and range without depending on binary64 rounding. */
fn integer(token: &str) -> Option<i64> {
    let (negative, unsigned) = token
        .strip_prefix('-')
        .map_or((false, token), |value| (true, value));
    let (mantissa, exponent) = unsigned.find(['e', 'E']).map_or((unsigned, 0), |index| {
        (&unsigned[..index], exponent(&unsigned[index + 1..]))
    });
    let fraction = mantissa
        .find('.')
        .map_or(0, |index| mantissa.len() - index - 1);
    let digits = mantissa
        .bytes()
        .filter(|byte| *byte != b'.')
        .collect::<Vec<_>>();
    let first = digits
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(digits.len());
    let digits = &digits[first..];
    if digits.is_empty() {
        return Some(0);
    }
    scaled(digits, exponent.saturating_sub(fraction as i64)).and_then(|number| {
        if number > VM_MAX_SAFE_LENGTH as u64 {
            None
        } else if negative {
            Some(-(number as i64))
        } else {
            Some(number as i64)
        }
    })
}
/** Exponent parsing saturates rather than performing work proportional to its numeric magnitude. */
fn exponent(source: &str) -> i64 {
    let (negative, digits) = source.strip_prefix('-').map_or(
        (false, source.strip_prefix('+').unwrap_or(source)),
        |value| (true, value),
    );
    let value = digits.bytes().fold(0i64, |value, digit| {
        value
            .saturating_mul(10)
            .saturating_add(i64::from(digit - b'0'))
    });
    if negative { -value } else { value }
}
/** At most sixteen significant decimal digits can fit this adapter's safe integer interval. */
fn scaled(digits: &[u8], scale: i64) -> Option<u64> {
    let digits = if scale < 0 {
        let remove = usize::try_from(scale.unsigned_abs()).ok()?;
        if remove > digits.len()
            || digits[digits.len() - remove..]
                .iter()
                .any(|digit| *digit != b'0')
        {
            return None;
        }
        &digits[..digits.len() - remove]
    } else {
        digits
    };
    let zeros = usize::try_from(scale.max(0)).ok()?;
    let safe_digits = (VM_MAX_SAFE_LENGTH as u64).to_string().len();
    if digits.len().saturating_add(zeros) > safe_digits {
        return None;
    }
    let mut value = digits.iter().try_fold(0u64, |value, digit| {
        value.checked_mul(10)?.checked_add(u64::from(*digit - b'0'))
    })?;
    for _ in 0..zeros {
        value = value.checked_mul(10)?;
    }
    Some(value)
}
