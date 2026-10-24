/**
 * BigInt arithmetic, bounded to 128 bits. Every result that leaves the i128 range throws a
 * catchable RangeError rather than growing without bound, mixing a BigInt with a Number throws a
 * TypeError as in JavaScript, and conversions accept the literal forms (decimal, 0x, 0o, 0b).
 */
use super::convert::Hint;
use super::{Res, Vm};
use crate::v9::compile::Op;
use crate::v9::value::JsValue;

/** Bits in V9's BigInt representation. */
const BITS: u32 = i128::BITS;

/** StringToBigInt: trimmed decimal (with sign) or a 0x/0o/0b literal; empty text is 0n. */
pub(in crate::v9) fn parse_bigint(text: &str) -> Option<i128> {
    let text = text.trim();
    if text.is_empty() {
        return Some(0);
    }
    let lower = text.get(..2).map(str::to_ascii_lowercase);
    let radix = match lower.as_deref() {
        Some("0x") => 16,
        Some("0o") => 8,
        Some("0b") => 2,
        _ => return parse_decimal(text),
    };
    let digits = &text[2..];
    (!digits.is_empty() && !digits.starts_with(['+', '-']))
        .then(|| i128::from_str_radix(digits, radix).ok())
        .flatten()
}

/** A signed decimal integer. */
fn parse_decimal(text: &str) -> Option<i128> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/** The text of a BigInt in `radix` (2..=36). */
pub(in crate::v9) fn bigint_to_radix(value: i128, radix: u32) -> String {
    if radix == 10 {
        return value.to_string();
    }
    let mut magnitude = value.unsigned_abs();
    let mut digits = Vec::new();
    loop {
        let digit = (magnitude % u128::from(radix)) as u32;
        digits.push(char::from_digit(digit, radix).unwrap_or('0'));
        magnitude /= u128::from(radix);
        if magnitude == 0 {
            break;
        }
    }
    if value < 0 {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

/** BigInt.asUintN: the value modulo 2^bits, or None when that does not fit 128 bits. */
pub(in crate::v9) fn as_uint_n(bits: u64, value: i128) -> Option<i128> {
    if bits >= u64::from(BITS - 1) {
        return (value >= 0).then_some(value);
    }
    Some(value.rem_euclid(1i128 << bits))
}

/** BigInt.asIntN: the value wrapped into a signed `bits`-bit integer (sign-extended). */
pub(in crate::v9) fn as_int_n(bits: u64, value: i128) -> i128 {
    match bits {
        0 => 0,
        bits if bits >= u64::from(BITS) => value,
        bits => {
            let shift = BITS - bits as u32;
            value.wrapping_shl(shift) >> shift
        }
    }
}

#[allow(clippy::wrong_self_convention)]
impl Vm<'_> {
    /** The RangeError a result past V9's BigInt bound throws. */
    fn too_big(&mut self) -> super::Thrown {
        self.range_error("Maximum BigInt size exceeded")
    }
    /** A binary operator with at least one BigInt operand (after ToPrimitive). */
    pub fn bigint_binary(&mut self, op: Op, left: &JsValue, right: &JsValue) -> Res<JsValue> {
        let (JsValue::BigInt(a), JsValue::BigInt(b)) = (left, right) else {
            return Err(
                self.type_error("Cannot mix BigInt and other types, use explicit conversions")
            );
        };
        let (a, b) = (*a, *b);
        let value = match op {
            Op::Add => a.checked_add(b),
            Op::Sub => a.checked_sub(b),
            Op::Mul => a.checked_mul(b),
            Op::Div | Op::Mod if b == 0 => return Err(self.range_error("Division by zero")),
            Op::Div => a.checked_div(b),
            Op::Mod => a.checked_rem(b),
            Op::Exp if b < 0 => return Err(self.range_error("Exponent must be non-negative")),
            Op::Exp => u32::try_from(b).ok().and_then(|b| a.checked_pow(b)),
            Op::BitAnd => Some(a & b),
            Op::BitOr => Some(a | b),
            Op::BitXor => Some(a ^ b),
            Op::Shl => shift_left(a, b),
            Op::Shr => shift_left(a, b.checked_neg().unwrap_or(i128::MAX)),
            _ => {
                return Err(self.type_error("BigInts have no unsigned right shift, use >> instead"));
            }
        };
        value.map(JsValue::BigInt).ok_or_else(|| self.too_big())
    }
    /** `left < right` when either primitive is a BigInt; None when a NaN or bad text is involved. */
    pub fn bigint_less_than(&mut self, left: &JsValue, right: &JsValue) -> Res<Option<bool>> {
        let order = |a: &JsValue, b: &JsValue| -> Option<std::cmp::Ordering> {
            match (a, b) {
                (JsValue::BigInt(a), JsValue::BigInt(b)) => Some(a.cmp(b)),
                (JsValue::BigInt(a), JsValue::String(text)) => {
                    parse_bigint(&text.lossy()).map(|b| a.cmp(&b))
                }
                (JsValue::String(text), JsValue::BigInt(b)) => {
                    parse_bigint(&text.lossy()).map(|a| a.cmp(b))
                }
                _ => None,
            }
        };
        if let Some(ordering) = order(left, right) {
            return Ok(Some(ordering.is_lt()));
        }
        if matches!(left, JsValue::String(_)) || matches!(right, JsValue::String(_)) {
            return Ok(None);
        }
        let (big, number, flipped) = match (left, right) {
            (JsValue::BigInt(big), other) => (*big, self.to_number(other)?, false),
            (other, JsValue::BigInt(big)) => (*big, self.to_number(other)?, true),
            _ => return Ok(None),
        };
        if number.is_nan() {
            return Ok(None);
        }
        let big = big as f64;
        Ok(Some(if flipped { number < big } else { big < number }))
    }
    /** Both operands of an arithmetic or bitwise operator as Numbers, or, when either is a
    BigInt, the operator's BigInt result (Err holds that value, not an error). */
    pub fn numeric_pair(
        &mut self,
        op: Op,
        left: &JsValue,
        right: &JsValue,
    ) -> Res<Result<(f64, f64), JsValue>> {
        let left = self.to_numeric(left)?;
        let right = self.to_numeric(right)?;
        match (left, right) {
            (JsValue::Number(a), JsValue::Number(b)) => Ok(Ok((a, b))),
            (left, right) => self.bigint_binary(op, &left, &right).map(Err),
        }
    }
    /** ToNumeric: a BigInt stays one, anything else becomes a Number. */
    pub fn to_numeric(&mut self, value: &JsValue) -> Res<JsValue> {
        let primitive = self.to_primitive(value, Hint::Number)?;
        if matches!(primitive, JsValue::BigInt(_)) {
            return Ok(primitive);
        }
        Ok(JsValue::Number(self.to_number(&primitive)?))
    }
    /** Unary minus, `~`, `++` and `--` on a BigInt. */
    pub fn bigint_unary(&mut self, op: Op, value: i128) -> Res<JsValue> {
        let result = match op {
            Op::Neg => value.checked_neg(),
            Op::BitNot => Some(!value),
            Op::Inc => value.checked_add(1),
            _ => value.checked_sub(1),
        };
        result.map(JsValue::BigInt).ok_or_else(|| self.too_big())
    }
}

/** `a << b` for any signed shift count; negative counts shift right with sign fill. */
fn shift_left(a: i128, b: i128) -> Option<i128> {
    if b < 0 {
        let count = b.unsigned_abs().min(u128::from(BITS - 1)) as u32;
        return Some(a >> count);
    }
    if a == 0 {
        return Some(0);
    }
    let count = u32::try_from(b).ok().filter(|count| *count < BITS)?;
    let shifted = a.checked_shl(count)?;
    (shifted >> count == a).then_some(shifted)
}
