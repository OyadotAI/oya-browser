/** BigInt literal property names become exact decimal strings without requiring a runtime BigInt. */
use crate::protocol::{EngineError, EngineResult};

/** Nine decimal digits fit in one limb and leave room for a hexadecimal multiply in u64. */
const DECIMAL_BASE: u64 = 1_000_000_000;
/** Padding matches the limb radix when writing all but the highest word. */
const DECIMAL_WIDTH: usize = 9;
/** Conversion work remains bounded even for a single excessively long hexadecimal token. */
const CONVERSION_WORK: usize = 10_000_000;

/** Scanner-validated literal digits include an optional radix prefix but no separators or suffix. */
pub(in crate::v9) fn bigint_property_key(digits: &str) -> EngineResult<String> {
    let (radix, digits) = match digits.get(..2) {
        Some("0x" | "0X") => (16, &digits[2..]),
        Some("0o" | "0O") => (8, &digits[2..]),
        Some("0b" | "0B") => (2, &digits[2..]),
        _ => return Ok(digits.to_string()),
    };
    let mut words = vec![0u64];
    let mut work = 0usize;
    for digit in digits.chars() {
        accumulate(
            &mut words,
            radix,
            digit.to_digit(radix as u32).unwrap_or(0) as u64,
            &mut work,
        )?;
    }
    let mut output = words.pop().unwrap_or(0).to_string();
    for word in words.iter().rev() {
        output.push_str(&format!("{word:0DECIMAL_WIDTH$}"));
    }
    Ok(output)
}

/** Multiplying by the input radix retains every digit without floating point conversion. */
fn accumulate(
    words: &mut Vec<u64>,
    radix: u64,
    mut carry: u64,
    work: &mut usize,
) -> EngineResult<()> {
    *work = work.saturating_add(words.len());
    if *work > CONVERSION_WORK {
        return Err(EngineError::limit(
            "BigInt property conversion operations",
            CONVERSION_WORK,
        ));
    }
    for word in words.iter_mut() {
        let value = *word * radix + carry;
        *word = value % DECIMAL_BASE;
        carry = value / DECIMAL_BASE;
    }
    if carry != 0 {
        words.push(carry);
    }
    Ok(())
}
