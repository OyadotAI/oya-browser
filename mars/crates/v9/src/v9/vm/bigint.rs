/** Exact BigInt operators normalize arbitrary-width results while retaining bounded work and memory. */
use super::convert::Hint;
use super::{Res, Vm};
use crate::v9::compile::Op;
use crate::v9::value::{BigInteger, JsValue};
use std::cmp::Ordering;

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

impl Vm<'_> {
    /** Valid arithmetic that exceeds the retained-value budget raises the standard catchable size error. */
    fn too_big(&mut self) -> super::Thrown {
        self.range_error("Maximum BigInt size exceeded")
    }
    /** Both BigInt representations participate in one exact mathematical operator domain. */
    pub fn bigint_binary(&mut self, op: Op, left: &JsValue, right: &JsValue) -> Res<JsValue> {
        let (Some(a), Some(b)) = (left.bigint_value(), right.bigint_value()) else {
            return Err(
                self.type_error("Cannot mix BigInt and other types, use explicit conversions")
            );
        };
        if matches!(op, Op::Div | Op::Mod) && b.is_zero() {
            return Err(self.range_error("Division by zero"));
        }
        if op == Op::Exp && b.negative() {
            return Err(self.range_error("Exponent must be non-negative"));
        }
        let linear = a.words().len().saturating_add(b.words().len()).max(1);
        let work = match op {
            Op::Mul => a.product_work(&b).saturating_add(linear),
            Op::Div | Op::Mod if b.power_of_two().is_none() => a
                .words()
                .len()
                .saturating_mul(b.words().len())
                .saturating_add(linear),
            Op::Exp if a.power_of_two().is_none() && a.bit_len() > 1 => {
                let bits = b
                    .to_usize()
                    .and_then(|exponent| a.bit_len().checked_mul(exponent))
                    .unwrap_or(usize::MAX);
                let words = bits.div_ceil(u32::BITS as usize);
                words.saturating_mul(words).saturating_add(linear)
            }
            _ => linear,
        };
        // Oversized powers fail as RangeError before work accounting tries to represent an impossible result.
        if op == Op::Exp
            && a.bit_len() > 1
            && b.to_usize().is_none_or(|exponent| {
                (a.bit_len() - 1)
                    .checked_mul(exponent)
                    .is_none_or(|bits| bits >= crate::v9::value::BIGINT_MAX_BITS)
            })
        {
            return Err(self.too_big());
        }
        self.charge(work)?;
        let result = match op {
            Op::Add => a.add(&b),
            Op::Sub => a.subtract(&b),
            Op::Mul => a.multiply(&b),
            Op::Div => a.divide(&b).map(|pair| pair.0),
            Op::Mod => a.divide(&b).map(|pair| pair.1),
            Op::Exp => a.power(&b),
            Op::BitAnd => a.bitwise(&b, |a, b| a & b),
            Op::BitOr => a.bitwise(&b, |a, b| a | b),
            Op::BitXor => a.bitwise(&b, |a, b| a ^ b),
            Op::Shl | Op::Shr => shift(&a, &b, op == Op::Shl),
            _ => {
                return Err(self.type_error("BigInts have no unsigned right shift, use >> instead"));
            }
        };
        result.map(JsValue::bigint).ok_or_else(|| self.too_big())
    }
    /** Relational comparison never converts an integer operand to a rounded Number. */
    pub fn bigint_less_than(&mut self, left: &JsValue, right: &JsValue) -> Res<Option<bool>> {
        if let (Some(left), Some(right)) = (left.bigint_value(), right.bigint_value()) {
            return Ok(Some(left < right));
        }
        if let JsValue::String(text) = left {
            return Ok(BigInteger::parse(&text.lossy())
                .ok()
                .zip(right.bigint_value())
                .map(|(a, b)| a < b));
        }
        if let JsValue::String(text) = right {
            return Ok(left
                .bigint_value()
                .zip(BigInteger::parse(&text.lossy()).ok())
                .map(|(a, b)| a < b));
        }
        let (big, other, flipped) = match left.bigint_value() {
            Some(big) => (big, right, false),
            None => match right.bigint_value() {
                Some(big) => (big, left, true),
                None => return Ok(None),
            },
        };
        let number = self.to_number(other)?;
        let order = compare_number(&big, number);
        Ok(order.map(|order| {
            if flipped {
                order.is_gt()
            } else {
                order.is_lt()
            }
        }))
    }
    /** ToNumeric applies authored primitive conversion once before choosing Number or BigInt arithmetic. */
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
    /** Primitive integers retain their exact storage form. */
    #[allow(clippy::wrong_self_convention)]
    pub fn to_numeric(&mut self, value: &JsValue) -> Res<JsValue> {
        let primitive = self.to_primitive(value, Hint::Number)?;
        if primitive.is_bigint() {
            return Ok(primitive);
        }
        Ok(JsValue::Number(self.to_number(&primitive)?))
    }
    /** Unary operators share the same exact normalization and retained-value bounds as binary arithmetic. */
    pub fn bigint_unary(&mut self, op: Op, value: &JsValue) -> Res<JsValue> {
        let value = value
            .bigint_value()
            .ok_or_else(|| self.type_error("BigInt operand expected"))?;
        self.charge(value.words().len().max(1))?;
        let result = match op {
            Op::Neg => Some(value.negated()),
            Op::BitNot => value.complement(),
            Op::Inc => value.add(&BigInteger::from_i128(1)),
            _ => value.subtract(&BigInteger::from_i128(1)),
        };
        result.map(JsValue::bigint).ok_or_else(|| self.too_big())
    }
}
/** Negative counts reverse shift direction; a huge arithmetic right shift saturates to the sign. */
fn shift(value: &BigInteger, count: &BigInteger, left: bool) -> Option<BigInteger> {
    let left = left != count.negative();
    let count = if count.negative() {
        count.negated()
    } else {
        count.clone()
    };
    match count.to_usize() {
        Some(count) if left => value.shift_left(count),
        Some(count) => Some(value.shift_right(count)),
        None if !left => Some(BigInteger::from_i128(if value.negative() { -1 } else { 0 })),
        None if value.is_zero() => Some(BigInteger::zero()),
        _ => None,
    }
}
/** Integral and fractional Numbers compare against the exact integer part before considering a fractional tail. */
fn compare_number(big: &BigInteger, number: f64) -> Option<Ordering> {
    if number.is_nan() {
        return None;
    }
    if number == f64::INFINITY {
        return Some(Ordering::Less);
    }
    if number == f64::NEG_INFINITY {
        return Some(Ordering::Greater);
    }
    let truncated = BigInteger::from_f64(number.trunc())?;
    let order = big.cmp(&truncated);
    Some(if order == Ordering::Equal && number.fract() != 0.0 {
        if number > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    } else {
        order
    })
}
