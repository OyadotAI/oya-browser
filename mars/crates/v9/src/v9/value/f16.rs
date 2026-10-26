/** IEEE-754 binary16 conversion is shared by typed arrays, DataView, Math and Wasm SIMD. */
/** Round discarded mantissa bits without a host half-float dependency. */
fn rounded(bits: u64, shift: u32) -> u64 {
    let base = bits >> shift;
    let remainder = bits & ((1u64 << shift) - 1);
    let halfway = 1u64 << (shift - 1);
    base + u64::from(remainder > halfway || remainder == halfway && base & 1 != 0)
}
/** Conversion from binary64 avoids double rounding for the f64 demotion instruction. */
pub(in crate::v9) fn encode(number: f64) -> u16 {
    let bits = number.to_bits();
    let sign = ((bits >> 48) & 0x8000) as u16;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    if exponent == 0x7ff {
        return sign
            | 0x7c00
            | if mantissa == 0 {
                0
            } else {
                ((mantissa >> 42) as u16) | 0x0200
            };
    }
    let target = exponent - 1023 + 15;
    if target >= 31 {
        return sign | 0x7c00;
    }
    if target < -10 {
        return sign;
    }
    if target <= 0 {
        return sign | rounded(mantissa | (1u64 << 52), (43 - target) as u32) as u16;
    }
    let rounded = rounded(mantissa, 42);
    let combined = ((target as u64) << 10) + rounded;
    if combined >= 0x7c00 {
        sign | 0x7c00
    } else {
        sign | combined as u16
    }
}
/** Every finite half value has an exact binary64 representation, including signed zero. */
pub(in crate::v9) fn decode(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 10) & 31;
    let mantissa = bits & 1023;
    match exponent {
        0 => sign * f64::from(mantissa) * (2.0f64).powi(-24),
        31 => {
            if mantissa == 0 {
                sign * f64::INFINITY
            } else {
                f64::from_bits(
                    (u64::from(bits & 0x8000) << 48)
                        | 0x7ff0_0000_0000_0000
                        | (u64::from(mantissa) << 42),
                )
            }
        }
        _ => sign * (1.0 + f64::from(mantissa) / 1024.0) * (2.0f64).powi(i32::from(exponent) - 15),
    }
}
