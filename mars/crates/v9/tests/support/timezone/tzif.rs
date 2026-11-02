/** TZif v1-v4 records supply actual UTC transitions, second-precision local offsets and future POSIX rules. */
use super::Zone;
/** RFC 8536 headers precede each 32-bit and 64-bit data block. */
const HEADER: usize = 44;
/** Bound metadata before multiplying record widths or reserving transition storage. */
const MAX_TRANSITIONS: usize = 4096;
const MAX_TYPES: usize = 256;
const MAX_CHARACTERS: usize = 65_536;
const MAX_LEAPS: usize = 512;
const MAX_OFFSET: i32 = 86_400;
const MAX_POSIX: usize = 256;
const DATE_CLIP: f64 = 8.64e15;
/** Native timezone parsing returns diagnostics rather than panicking on truncated integer fields. */
fn invalid() -> String {
    "Invalid or unsupported TZif timezone data".into()
}
/** Counts are validated before any derived byte index is used. */
struct Header {
    version: u8,
    utc: usize,
    standard: usize,
    leaps: usize,
    times: usize,
    types: usize,
    chars: usize,
}
/** Read one network-order count without unchecked slicing. */
fn u32_at(bytes: &[u8], offset: usize) -> Result<usize, String> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ) as usize)
}
/** Both sections of modern TZif files use identical validated count constraints. */
fn header(bytes: &[u8], start: usize) -> Result<Header, String> {
    let bytes = bytes.get(start..start + HEADER).ok_or_else(invalid)?;
    if &bytes[..4] != b"TZif" || !matches!(bytes[4], 0 | b'2' | b'3' | b'4') {
        return Err(invalid());
    }
    let result = Header {
        version: bytes[4],
        utc: u32_at(bytes, 20)?,
        standard: u32_at(bytes, 24)?,
        leaps: u32_at(bytes, 28)?,
        times: u32_at(bytes, 32)?,
        types: u32_at(bytes, 36)?,
        chars: u32_at(bytes, 40)?,
    };
    if result.times > MAX_TRANSITIONS
        || result.types == 0
        || result.types > MAX_TYPES
        || result.chars == 0
        || result.chars > MAX_CHARACTERS
        || result.leaps > MAX_LEAPS
        || ![0, result.types].contains(&result.utc)
        || ![0, result.types].contains(&result.standard)
    {
        return Err(invalid());
    }
    Ok(result)
}
/** Every width and count is independently small before computing the section's physical byte length. */
fn size(header: &Header, width: usize) -> usize {
    header.times * (width + 1)
        + header.types * 6
        + header.chars
        + header.leaps * (width + 4)
        + header.standard
        + header.utc
}
/** The local time types as (offset seconds, is DST), and the one in force before the first transition. */
type LocalTypes = (Vec<(i32, bool)>, (i32, bool));
/** Transitions as (instant, offset seconds) with each one's DST flag. */
type Transitions = (Vec<(f64, i32)>, Vec<bool>);

/** Signed offsets preserve historical LMT seconds; abbreviations must name an actual NUL-terminated string. */
fn types(bytes: &[u8], header: &Header) -> Result<LocalTypes, String> {
    let start = header.times * 9;
    let names = bytes
        .get(start + header.types * 6..start + header.types * 6 + header.chars)
        .ok_or_else(invalid)?;
    let mut offsets = Vec::with_capacity(header.types);
    let mut initial = None;
    for index in 0..header.types {
        let field = bytes
            .get(start + index * 6..start + index * 6 + 6)
            .ok_or_else(invalid)?;
        let offset = i32::from_be_bytes(field[..4].try_into().map_err(|_| invalid())?);
        if offset.unsigned_abs() > MAX_OFFSET as u32
            || field[4] > 1
            || !names
                .get(field[5] as usize..)
                .is_some_and(|name| name.contains(&0))
        {
            return Err(invalid());
        }
        if field[4] == 0 && initial.is_none() {
            initial = Some((offset, false));
        }
        offsets.push((offset, field[4] != 0));
    }
    let initial = initial.unwrap_or(offsets[0]);
    Ok((offsets, initial))
}
/** UTC instants are strictly ordered; transition indices can only select validated local-time records. */
fn transitions(
    bytes: &[u8],
    header: &Header,
    offsets: &[(i32, bool)],
    initial: &mut (i32, bool),
) -> Result<Transitions, String> {
    let mut result = Vec::with_capacity(header.times);
    let mut daylight = Vec::with_capacity(header.times);
    let mut previous = None;
    for index in 0..header.times {
        let raw = bytes.get(index * 8..index * 8 + 8).ok_or_else(invalid)?;
        let seconds = i64::from_be_bytes(raw.try_into().map_err(|_| invalid())?);
        if previous.is_some_and(|previous| previous >= seconds) {
            return Err(invalid());
        }
        previous = Some(seconds);
        let offset = *offsets
            .get(bytes[header.times * 8 + index] as usize)
            .ok_or_else(invalid)?;
        let instant = seconds as f64 * 1000.0;
        if instant < -DATE_CLIP {
            *initial = offset;
        }
        if instant.abs() <= DATE_CLIP {
            result.push((instant, offset.0));
            daylight.push(offset.1);
        }
    }
    Ok((result, daylight))
}
/** Future rules stay inert strings for the VM's pure POSIX-rule parser; surrounding newlines are mandatory. */
fn footer(bytes: &[u8]) -> Result<Option<String>, String> {
    let inner = bytes
        .strip_prefix(b"\n")
        .and_then(|value| value.strip_suffix(b"\n"))
        .ok_or_else(invalid)?;
    if inner.len() > MAX_POSIX || inner.contains(&b'\n') || !inner.is_ascii() {
        return Err(invalid());
    }
    if inner.is_empty() {
        return Ok(None);
    }
    String::from_utf8(inner.to_vec())
        .map(Some)
        .map_err(|_| invalid())
}
/** Modern files use their 64-bit section; v1 is widened without inventing future transitions. */
pub(super) fn parse(bytes: &[u8]) -> Result<Zone, String> {
    let first = header(bytes, 0)?;
    let first_end = HEADER + size(&first, 4);
    if bytes.len() < first_end {
        return Err(invalid());
    }
    if first.version == 0 {
        return legacy(bytes, &first);
    }
    let second = header(bytes, first_end)?;
    if second.version != first.version || second.leaps != 0 {
        return Err(invalid());
    }
    let start = first_end + HEADER;
    let end = start + size(&second, 8);
    let data = bytes.get(start..end).ok_or_else(invalid)?;
    let (offsets, mut initial) = types(data, &second)?;
    let (transitions, daylight) = transitions(data, &second, &offsets, &mut initial)?;
    Ok(Zone {
        display_names: None,
        initial: initial.0,
        initial_daylight: initial.1,
        daylight,
        transitions,
        posix: footer(bytes.get(end..).ok_or_else(invalid)?)?,
    })
}
/** Widen only time fields in v1, leaving type records, abbreviation bytes and flags unchanged. */
fn legacy(bytes: &[u8], header: &Header) -> Result<Zone, String> {
    if header.leaps != 0 || bytes.len() != HEADER + size(header, 4) {
        return Err(invalid());
    }
    let bytes = &bytes[HEADER..];
    let mut data = Vec::with_capacity(size(header, 8));
    for field in bytes[..header.times * 4].chunks_exact(4) {
        let seconds = i32::from_be_bytes(field.try_into().map_err(|_| invalid())?);
        data.extend_from_slice(&(seconds as i64).to_be_bytes());
    }
    data.extend_from_slice(&bytes[header.times * 4..]);
    let (offsets, mut initial) = types(&data, header)?;
    let (transitions, daylight) = transitions(&data, header, &offsets, &mut initial)?;
    Ok(Zone {
        display_names: None,
        initial: initial.0,
        initial_daylight: initial.1,
        daylight,
        transitions,
        posix: None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    /** Hermetic v1 fixture has real second-precision offsets and one actual transition. */
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; HEADER];
        bytes[..4].copy_from_slice(b"TZif");
        for (index, count) in [(32, 1u32), (36, 2), (40, 8)] {
            bytes[index..index + 4].copy_from_slice(&count.to_be_bytes());
        }
        bytes.extend_from_slice(&1000i32.to_be_bytes());
        bytes.push(1);
        for (offset, dst, name) in [(1234i32, 0, 0), (-3600, 1, 4)] {
            bytes.extend_from_slice(&offset.to_be_bytes());
            bytes.extend_from_slice(&[dst, name]);
        }
        bytes.extend_from_slice(b"LMT\0DST\0");
        bytes
    }
    #[test]
    /** Local offsets retain their historical seconds rather than rounding to minutes. */
    fn preserves_actual_seconds_and_transition_types() {
        let zone = parse(&fixture()).unwrap();
        assert_eq!(zone.initial, 1234);
        assert!(!zone.initial_daylight);
        assert_eq!(zone.daylight, [true]);
        assert_eq!(zone.transitions, [(1_000_000.0, -3600)]);
        assert_eq!(zone.posix, None);
    }
    #[test]
    /** Modern files select real 64-bit transitions and preserve their POSIX future rules verbatim. */
    fn modern_sections_keep_second_precision_and_future_rules() {
        let first = fixture();
        let mut bytes = first.clone();
        bytes[4] = b'3';
        let mut second = first[..HEADER].to_vec();
        second[4] = b'3';
        second.extend_from_slice(&5_000_000_000i64.to_be_bytes());
        second.extend_from_slice(&first[HEADER + 4..]);
        bytes.extend_from_slice(&second);
        bytes.extend_from_slice(b"\nEST5EDT,M3.2.0,M11.1.0\n");
        let zone = parse(&bytes).unwrap();
        assert_eq!(zone.transitions, [(5_000_000_000_000.0, -3600)]);
        assert_eq!(zone.posix.as_deref(), Some("EST5EDT,M3.2.0,M11.1.0"));
        bytes.pop();
        assert!(parse(&bytes).is_err());
    }
    #[test]
    /** Every truncated prefix fails safely before any indexed read. */
    fn truncated_input_never_panics() {
        let bytes = fixture();
        for length in 0..bytes.len() {
            assert!(parse(&bytes[..length]).is_err());
        }
    }
    #[test]
    /** Forged count headers and nonexistent local-time types remain input errors. */
    fn malformed_counts_and_types_are_rejected() {
        let mut bytes = fixture();
        bytes[HEADER + 4] = 2;
        assert!(parse(&bytes).is_err());
        bytes = fixture();
        bytes[32..36].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse(&bytes).is_err());
    }
}
