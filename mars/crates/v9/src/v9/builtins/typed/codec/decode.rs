use super::base64::Tail;
/** Bounded base64 decoding commits complete validated quanta and leaves an unfittable quantum unread. */
use super::*;

/** A parse result retains prior bytes on syntax failure so setFromBase64 can expose those writes. */
pub(super) struct Decoded {
    pub bytes: Vec<u8>,
    pub read: usize,
    pub invalid: bool,
}

/** The cursor records original UTF-16 positions, including accepted ASCII whitespace. */
pub(super) fn parse(
    vm: &mut Vm,
    input: &JsString,
    url: bool,
    tail: Tail,
    capacity: usize,
) -> Res<Decoded> {
    let mut result = Decoded {
        bytes: Vec::new(),
        read: 0,
        invalid: false,
    };
    let mut cursor = 0;
    while result.bytes.len() < capacity {
        let mut chunk = Vec::with_capacity(4);
        while chunk.len() < 4 && cursor < input.len() {
            let unit = input.as_units()[cursor];
            cursor += 1;
            vm.charge(1)?;
            if !whitespace(unit) {
                chunk.push(unit);
            }
        }
        if chunk.is_empty() {
            result.read = cursor;
            break;
        }
        match quantum(&chunk, url, tail) {
            Quantum::Stop => break,
            Quantum::Invalid => {
                result.invalid = true;
                break;
            }
            Quantum::Bytes(bytes, padded) => {
                if bytes.len() > capacity - result.bytes.len() {
                    break;
                }
                if padded {
                    while cursor < input.len() && whitespace(input.as_units()[cursor]) {
                        cursor += 1;
                    }
                    if cursor < input.len() {
                        result.invalid = true;
                        break;
                    }
                }
                result.bytes.extend(bytes);
                result.read = cursor;
                if padded || chunk.len() < 4 {
                    break;
                }
            }
        }
    }
    Ok(result)
}

/** A partial unpadded quantum is allowed only in loose mode; strict mode also checks unused bits. */
fn quantum(chunk: &[u16], url: bool, tail: Tail) -> Quantum {
    let mut digits = Vec::new();
    let mut padding = 0;
    for unit in chunk {
        if *unit == u16::from(b'=') {
            padding += 1;
        } else {
            if padding > 0 {
                return Quantum::Invalid;
            }
            let Some(digit) = digit(*unit, url) else {
                return Quantum::Invalid;
            };
            digits.push(digit);
        }
    }
    if chunk.len() < 4 && tail == Tail::Stop {
        return Quantum::Stop;
    }
    if digits.len() < 2
        || padding > 2
        || padding > 0 && (chunk.len() != 4 || digits.len() + padding != 4)
        || chunk.len() < 4 && tail == Tail::Strict
    {
        return Quantum::Invalid;
    }
    if tail == Tail::Strict
        && ((digits.len() == 2 && digits[1] & 15 != 0) || (digits.len() == 3 && digits[2] & 3 != 0))
    {
        return Quantum::Invalid;
    }
    let word = digits
        .iter()
        .fold(0u32, |word, digit| (word << 6) | u32::from(*digit))
        << (6 * (4 - digits.len()));
    let bytes = (0..digits.len() - 1)
        .map(|index| (word >> (16 - 8 * index)) as u8)
        .collect();
    Quantum::Bytes(bytes, padding > 0)
}

/** Parsing distinguishes a tolerated incomplete tail from a malformed input. */
enum Quantum {
    Stop,
    Invalid,
    Bytes(Vec<u8>, bool),
}
/** Only five ASCII whitespace characters are ignored by base64 decoding. */
fn whitespace(unit: u16) -> bool {
    matches!(unit, 9 | 10 | 12 | 13 | 32)
}
/** Alphabet-specific final symbols never accept their counterparts from the other alphabet. */
fn digit(unit: u16, url: bool) -> Option<u8> {
    match unit {
        65..=90 => Some((unit - 65) as u8),
        97..=122 => Some((unit - 97 + 26) as u8),
        48..=57 => Some((unit - 48 + 52) as u8),
        43 if !url => Some(62),
        47 if !url => Some(63),
        45 if url => Some(62),
        95 if url => Some(63),
        _ => None,
    }
}
