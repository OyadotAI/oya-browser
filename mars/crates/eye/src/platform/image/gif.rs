/** Isolate the first GIF image without asking native decoding to retain later animation frames. */
use super::{EngineResult, unsupported};

/** GIF screen descriptors include the signature and seven bytes of canvas/palette metadata. */
const HEADER: usize = 13;
/** Image descriptors include their separator and nine bytes of rectangle/palette metadata. */
const DESCRIPTOR: usize = 10;
/** A GIF graphics-control extension is exactly eight bytes including its block terminator. */
const GRAPHICS_CONTROL: usize = 8;

/** Full-canvas first images have unambiguous standalone pixels, palette, interlace and transparency. */
pub(super) fn first_frame(bytes: &[u8], width: u32, height: u32) -> EngineResult<Vec<u8>> {
    let header = bytes.get(..HEADER).ok_or_else(unsupported)?;
    if u32::from(u16::from_le_bytes([header[6], header[7]])) != width
        || u32::from(u16::from_le_bytes([header[8], header[9]])) != height
    {
        return Err(unsupported());
    }
    let start = HEADER + palette(header[10]);
    let prefix = bytes.get(..start).ok_or_else(unsupported)?;
    let mut cursor = start;
    let mut control = None;
    loop {
        match bytes.get(cursor).copied() {
            Some(0x21) => {
                let label = *bytes.get(cursor + 1).ok_or_else(unsupported)?;
                if label == 0xf9 {
                    let extension = bytes
                        .get(cursor..cursor + GRAPHICS_CONTROL)
                        .ok_or_else(unsupported)?;
                    if extension[2] != 4 || extension[7] != 0 {
                        return Err(unsupported());
                    }
                    control = Some(extension);
                    cursor += GRAPHICS_CONTROL;
                } else if label == 0x01 {
                    // Plain-text rendering before the first image is outside this image-only subset.
                    return Err(unsupported());
                } else {
                    cursor = sub_blocks(bytes, cursor + 2)?;
                }
            }
            Some(0x2c) => {
                let end = image_end(bytes, cursor, width, height)?;
                let mut frame = Vec::with_capacity(end + 1);
                frame.extend_from_slice(prefix);
                if let Some(control) = control {
                    frame.extend_from_slice(control);
                }
                frame.extend_from_slice(&bytes[cursor..end]);
                frame.push(0x3b);
                return Ok(frame);
            }
            _ => return Err(unsupported()),
        }
    }
}

/** Ignore later frames only after the complete first LZW payload and its terminator exist. */
fn image_end(bytes: &[u8], start: usize, width: u32, height: u32) -> EngineResult<usize> {
    let descriptor = bytes
        .get(start..start + DESCRIPTOR)
        .ok_or_else(unsupported)?;
    let word = |offset| {
        u32::from(u16::from_le_bytes([
            descriptor[offset],
            descriptor[offset + 1],
        ]))
    };
    if word(1) != 0 || word(3) != 0 || word(5) != width || word(7) != height {
        return Err(unsupported());
    }
    let code = start + DESCRIPTOR + palette(descriptor[9]);
    if !bytes.get(code).is_some_and(|code| (2..=8).contains(code))
        || bytes.get(code + 1).is_none_or(|size| *size == 0)
    {
        return Err(unsupported());
    }
    sub_blocks(bytes, code + 1)
}

/** Color-table byte counts derive from bounded three-bit palette size fields. */
fn palette(packed: u8) -> usize {
    if packed & 0x80 == 0 {
        0
    } else {
        3 * (1usize << ((packed & 7) + 1))
    }
}

/** Sub-block traversal is linear in admitted encoded bytes and refuses truncated payloads. */
fn sub_blocks(bytes: &[u8], mut cursor: usize) -> EngineResult<usize> {
    loop {
        let count = usize::from(*bytes.get(cursor).ok_or_else(unsupported)?);
        cursor += 1;
        if count == 0 {
            return Ok(cursor);
        }
        cursor = cursor
            .checked_add(count)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(unsupported)?;
    }
}
