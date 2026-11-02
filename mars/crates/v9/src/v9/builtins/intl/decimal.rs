/*!
 * Exact decimal arithmetic for Intl.NumberFormat, ported from ECMA-402's ToRawFixed,
 * ToRawPrecision and FormatNumericToString (V8 hands these to ICU's DecimalQuantity): a value is
 * its digit string and point, so rounding to fraction or significant digits, by an increment and
 * in any of the nine rounding modes, is exact for doubles (shortest round-trip digits, as ICU
 * reads them), BigInts and decimal strings alike.
 */

/** A finite magnitude: `0.d1d2... x 10^point`, digits without leading or trailing zeros. */
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Dec {
    pub digits: Vec<u8>,
    pub point: i64,
}

/** A mathematical value: NaN, an infinity, or a signed finite decimal (sign kept for -0). */
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Num {
    NaN,
    Inf(bool),
    Fin(bool, Dec),
}

/** Exponents beyond this are clamped; no format can show more digits than this. */
const MAX_EXPONENT: i64 = 100_000;

impl Dec {
    /** Zero. */
    pub fn zero() -> Dec {
        Dec {
            digits: Vec::new(),
            point: 0,
        }
    }
    /** Whether this is zero. */
    pub fn is_zero(&self) -> bool {
        self.digits.is_empty()
    }
    /** floor(log10(x)) for a nonzero value. */
    pub fn magnitude(&self) -> i64 {
        self.point - 1
    }
    /** x times 10^n. */
    pub fn shift(&self, n: i64) -> Dec {
        if self.is_zero() {
            return Dec::zero();
        }
        Dec {
            digits: self.digits.clone(),
            point: self.point + n,
        }
    }
    /** Normalised from raw big-endian digits whose first digit sits at `point`. */
    fn from_raw(mut digits: Vec<u8>, mut point: i64) -> Dec {
        let lead = digits.iter().take_while(|d| **d == 0).count();
        digits.drain(..lead);
        point -= lead as i64;
        while digits.last() == Some(&0) {
            digits.pop();
        }
        if digits.is_empty() {
            return Dec::zero();
        }
        Dec { digits, point }
    }
    /** Whether the value has no fraction digits. */
    pub fn is_integer(&self) -> bool {
        self.point >= self.digits.len() as i64
    }
    /** The digit at place value 10^place. */
    fn at(&self, place: i64) -> u8 {
        let index = self.point - 1 - place;
        if index < 0 || index >= self.digits.len() as i64 {
            0
        } else {
            self.digits[index as usize]
        }
    }
    /** The integer digits and exactly `fraction` fraction digits. */
    pub fn fixed(&self, fraction: i64) -> (String, String) {
        let int: String = if self.point <= 0 {
            "0".into()
        } else {
            (0..self.point)
                .rev()
                .map(|p| char::from(b'0' + self.at(p)))
                .collect()
        };
        let frac = (1..=fraction)
            .map(|j| char::from(b'0' + self.at(-j)))
            .collect();
        (int, frac)
    }
}

/** Parse decimal digits with an optional point and exponent (`12.5e3`), without sign. */
pub(super) fn parse_decimal(text: &str) -> Option<Dec> {
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (&text[..at], &text[at + 1..]),
        None => (text, "0"),
    };
    let exponent = parse_exponent(exponent)?;
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = int.bytes().chain(frac.bytes()).map(|b| b - b'0').collect();
    Some(Dec::from_raw(digits, int.len() as i64 + exponent))
}

/** A signed exponent, clamped so absurd ones stay cheap. */
fn parse_exponent(text: &str) -> Option<i64> {
    let (negative, body) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = body
        .trim_start_matches('0')
        .parse::<i64>()
        .unwrap_or(if body.trim_start_matches('0').is_empty() {
            0
        } else {
            MAX_EXPONENT
        })
        .min(MAX_EXPONENT);
    Some(if negative { -value } else { value })
}

/** A double as its shortest round-trip decimal, as ICU's DecimalQuantity reads it. */
pub(super) fn from_f64(x: f64) -> Num {
    if x.is_nan() {
        return Num::NaN;
    }
    if x.is_infinite() {
        return Num::Inf(x < 0.0);
    }
    let negative = x.is_sign_negative();
    if x == 0.0 {
        return Num::Fin(negative, Dec::zero());
    }
    let text = format!("{:e}", x.abs());
    Num::Fin(negative, parse_decimal(&text).unwrap_or_else(Dec::zero))
}

/** A StringNumericLiteral read exactly (Intl.NumberFormat v3), or None when it is not one. */
pub(super) fn from_string(text: &str) -> Option<Num> {
    let trimmed = text.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if trimmed.is_empty() {
        return Some(Num::Fin(false, Dec::zero()));
    }
    let lower = trimmed.as_bytes().get(..2).map(|p| p.to_ascii_lowercase());
    if matches!(lower.as_deref(), Some(b"0x" | b"0o" | b"0b")) {
        return None;
    }
    let (negative, body) = match trimmed.as_bytes()[0] {
        b'-' => (true, &trimmed[1..]),
        b'+' => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    if body == "Infinity" {
        return Some(Num::Inf(negative));
    }
    if body.starts_with(['+', '-']) {
        return Some(Num::NaN);
    }
    Some(parse_decimal(body).map_or(Num::NaN, |d| Num::Fin(negative, d)))
}

/** How an unsigned magnitude rounds (ECMA-402 GetUnsignedRoundingMode). */
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Unsigned {
    Infinity,
    Zero,
    HalfInfinity,
    HalfZero,
    HalfEven,
}

/** The unsigned rounding mode for a rounding mode name and the value's sign. */
pub(super) fn unsigned_mode(mode: &str, negative: bool) -> Unsigned {
    use Unsigned::*;
    match (mode, negative) {
        ("ceil", false) | ("floor", true) | ("expand", _) => Infinity,
        ("ceil", true) | ("floor", false) | ("trunc", _) => Zero,
        ("halfCeil", false) | ("halfFloor", true) | ("halfExpand", _) => HalfInfinity,
        ("halfCeil", true) | ("halfFloor", false) | ("halfTrunc", _) => HalfZero,
        _ => HalfEven,
    }
}

/** Big-endian digits divided by a small number: quotient and remainder. */
fn div_small(digits: &[u8], divisor: u32) -> (Vec<u8>, u32) {
    let mut quotient = Vec::with_capacity(digits.len());
    let mut rest = 0u32;
    for digit in digits {
        let value = rest * 10 + u32::from(*digit);
        quotient.push((value / divisor) as u8);
        rest = value % divisor;
    }
    (quotient, rest)
}

/** Big-endian digits times a small number. */
fn mul_small(digits: &[u8], factor: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(digits.len() + 5);
    let mut carry = 0u32;
    for digit in digits.iter().rev() {
        let value = u32::from(*digit) * factor + carry;
        out.push((value % 10) as u8);
        carry = value / 10;
    }
    while carry > 0 {
        out.push((carry % 10) as u8);
        carry /= 10;
    }
    out.reverse();
    out
}

/** Round a magnitude to a multiple of `increment x 10^at`. */
pub(super) fn round(x: &Dec, at: i64, increment: u32, mode: Unsigned) -> Dec {
    if x.is_zero() {
        return Dec::zero();
    }
    let keep = x.point - at;
    let whole: Vec<u8> = (0..keep.max(0)).map(|i| x.at(x.point - 1 - i)).collect();
    // The fraction below the rounding place, compared with one half: -1, 0 or 1, None if zero.
    let rest = if keep < 0 {
        Some(-1)
    } else {
        let tail: Vec<u8> = x.digits.iter().skip(keep as usize).copied().collect();
        match tail.first() {
            None => None,
            Some(first) if *first > 5 => Some(1),
            Some(first) if *first < 5 => Some(-1),
            Some(_) => Some(if tail.len() > 1 { 1 } else { 0 }),
        }
    };
    let (quotient, remainder) = div_small(&whole, increment);
    if remainder == 0 && rest.is_none() {
        return x.clone();
    }
    let twice = 2 * remainder;
    let half = if twice > increment {
        1
    } else if twice == increment {
        if rest.is_some() { 1 } else { 0 }
    } else if twice + 1 == increment {
        rest.unwrap_or(-1)
    } else {
        -1
    };
    let odd = quotient.last().is_some_and(|d| d % 2 == 1);
    let up = match mode {
        Unsigned::Infinity => true,
        Unsigned::Zero => false,
        Unsigned::HalfInfinity => half >= 0,
        Unsigned::HalfZero => half > 0,
        Unsigned::HalfEven => half > 0 || (half == 0 && odd),
    };
    let quotient = if up { add_one(quotient) } else { quotient };
    let value = mul_small(&quotient, increment);
    let len = value.len() as i64;
    Dec::from_raw(value, len + at)
}

/** Big-endian digits plus one. */
fn add_one(mut digits: Vec<u8>) -> Vec<u8> {
    for digit in digits.iter_mut().rev() {
        if *digit == 9 {
            *digit = 0;
        } else {
            *digit += 1;
            return digits;
        }
    }
    digits.insert(0, 1);
    digits
}

/** A rounded result: the value, its integer and fraction digits, and its rounding magnitude. */
#[derive(Clone, Debug)]
pub(super) struct Raw {
    pub value: Dec,
    pub int: String,
    pub frac: String,
    pub magnitude: i64,
}

/** Trailing zeros dropped from a fraction, at most `cut` of them. */
fn trim(frac: &mut String, cut: usize) {
    let mut cut = cut;
    while cut > 0 && frac.ends_with('0') {
        frac.pop();
        cut -= 1;
    }
}

/** ToRawFixed: round to `max` fraction digits by the increment, keeping at least `min`. */
pub(super) fn raw_fixed(x: &Dec, min: usize, max: usize, increment: u32, mode: Unsigned) -> Raw {
    let value = round(x, -(max as i64), increment, mode);
    let (int, mut frac) = value.fixed(max as i64);
    trim(&mut frac, max.saturating_sub(min));
    Raw {
        value,
        int,
        frac,
        magnitude: -(max as i64),
    }
}

/** ToRawPrecision: round to `max` significant digits, keeping at least `min`. */
pub(super) fn raw_precision(x: &Dec, min: usize, max: usize, mode: Unsigned) -> Raw {
    let max_i = max as i64;
    let (value, magnitude) = if x.is_zero() {
        (Dec::zero(), 1 - max_i)
    } else {
        let value = round(x, x.magnitude() - max_i + 1, 1, mode);
        let magnitude = value.magnitude() - max_i + 1;
        (value, magnitude)
    };
    let (int, mut frac) = value.fixed((-magnitude).max(0));
    trim(&mut frac, max.saturating_sub(min));
    Raw {
        value,
        int,
        frac,
        magnitude,
    }
}
