/** Static WebP container and bitstream dimensions must agree before ImageIO can allocate pixels. */
use super::{EngineResult, unsupported};

/** RIFF container header length, including the WEBP form type. */
const HEADER: usize = 12;
/** Every RIFF chunk has a four-byte type and four-byte length. */
const CHUNK_HEADER: usize = 8;
/** VP8X's animation bit requires animated-canvas composition outside this slice. */
const ANIMATION: u8 = 2;

/** Permit one static lossy/lossless image with optional alpha and metadata, never embedded frames. */
pub(super) fn validate(bytes: &[u8], width: u32, height: u32) -> EngineResult<()> {
    let declared = bytes.get(4..8).ok_or_else(unsupported)?;
    if u32::from_le_bytes(declared.try_into().map_err(|_| unsupported())?) as usize + CHUNK_HEADER
        != bytes.len()
    {
        return Err(unsupported());
    }
    let mut cursor = HEADER;
    let mut images = 0;
    let mut alpha = false;
    while cursor < bytes.len() {
        let header = bytes
            .get(cursor..cursor + CHUNK_HEADER)
            .ok_or_else(unsupported)?;
        let length =
            u32::from_le_bytes(header[4..8].try_into().map_err(|_| unsupported())?) as usize;
        let start = cursor + CHUNK_HEADER;
        let end = start.checked_add(length).ok_or_else(unsupported)?;
        let body = bytes.get(start..end).ok_or_else(unsupported)?;
        match &header[..4] {
            b"VP8X" => {
                if cursor != HEADER
                    || length != 10
                    || body[0] & ANIMATION != 0
                    || (1 + triple(&body[4..7]), 1 + triple(&body[7..10])) != (width, height)
                {
                    return Err(unsupported());
                }
            }
            b"VP8 " | b"VP8L" => {
                images += 1;
                if images != 1
                    || dimensions(&header[..4], body)? != (width, height)
                    || (&header[..4] == b"VP8L" && alpha)
                {
                    return Err(unsupported());
                }
            }
            b"ALPH" => {
                if alpha || images != 0 || body.is_empty() {
                    return Err(unsupported());
                }
                alpha = true;
            }
            b"ANIM" | b"ANMF" => return Err(unsupported()),
            _ => {}
        }
        cursor = end
            .checked_add(length & 1)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(unsupported)?;
    }
    if images == 1 {
        Ok(())
    } else {
        Err(unsupported())
    }
}

/** Check the actual compressed image header, not only VP8X's potentially misleading canvas header. */
fn dimensions(kind: &[u8], body: &[u8]) -> EngineResult<(u32, u32)> {
    match kind {
        b"VP8 " if body.len() >= 10 && body.get(3..6) == Some(&[0x9d, 0x01, 0x2a]) => Ok((
            u32::from(u16::from_le_bytes([body[6], body[7]]) & 0x3fff),
            u32::from(u16::from_le_bytes([body[8], body[9]]) & 0x3fff),
        )),
        b"VP8L" if body.len() >= 5 && body[0] == 0x2f => {
            let bits = u32::from_le_bytes(body[1..5].try_into().map_err(|_| unsupported())?);
            Ok((1 + (bits & 0x3fff), 1 + ((bits >> 14) & 0x3fff)))
        }
        _ => Err(unsupported()),
    }
}

/** WebP's little-endian canvas dimensions use exactly three bytes. */
fn triple(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16
}
