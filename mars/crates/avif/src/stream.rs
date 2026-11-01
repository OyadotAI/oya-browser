/** Stream-level header parsing for one AVIF item: finds the sequence header and the first frame header among its OBUs, falling back to the av1C configOBUs for the sequence, and checks the sequence agrees with the av1C box. */
use crate::av1::{OBU_FRAME, OBU_FRAME_HEADER, OBU_SEQUENCE_HEADER, OBU_TILE_GROUP};
use crate::bits::BitReader;
use crate::error::{Error, Result};
use crate::frame::{self, Parsed};
use crate::obu;
use crate::properties::Av1C;
use crate::sequence::{self, Sequence};

/** The sequence header and the first frame header of an item. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Headers {
    pub(crate) seq: Sequence,
    pub(crate) frame: Parsed,
}

/** Parse the headers of an AV1 item payload. The sequence header may appear in the payload or only in av1C's configOBUs. */
pub(crate) fn parse_headers(payload: &[u8], av1c: &Av1C) -> Result<Headers> {
    let obus = obu::split(payload)?;
    let seq = match find_kind(&obus, OBU_SEQUENCE_HEADER) {
        Some(bytes) => sequence::parse(bytes)?,
        None => {
            let config = obu::split(&av1c.config_obus)?;
            let bytes = find_kind(&config, OBU_SEQUENCE_HEADER)
                .ok_or(Error::Malformed("no sequence header"))?;
            sequence::parse(bytes)?
        }
    };
    check_av1c(&seq, av1c)?;
    let frame_obu = obus
        .iter()
        .find(|o| o.kind == OBU_FRAME || o.kind == OBU_FRAME_HEADER)
        .ok_or(Error::Malformed("no frame header"))?;
    let mut r = BitReader::new(frame_obu.payload);
    let frame = frame::parse(&seq, &mut r, frame_obu.temporal_id, frame_obu.spatial_id)?;
    Ok(Headers { seq, frame })
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;

/** The tile group payloads of the frame whose header is `frame`: the bytes after the header in a frame OBU, and every tile group OBU that follows the frame's header OBU, up to the next frame. */
pub(crate) fn tile_groups<'a>(
    payload: &'a [u8],
    frame: &frame::FrameHeader,
) -> Result<Vec<&'a [u8]>> {
    let mut groups = Vec::new();
    let mut in_frame = false;
    for o in obu::split(payload)? {
        match o.kind {
            OBU_FRAME if !in_frame => {
                in_frame = true;
                groups.push(
                    o.payload
                        .get(frame.header_bytes..)
                        .ok_or(Error::Truncated)?,
                );
            }
            OBU_FRAME_HEADER | OBU_FRAME => break,
            OBU_TILE_GROUP if in_frame => groups.push(o.payload),
            _ => {}
        }
    }
    Ok(groups)
}

/** The payload of the first OBU of `kind`, if there is one. */
fn find_kind<'a>(obus: &[obu::Obu<'a>], kind: u8) -> Option<&'a [u8]> {
    obus.iter().find(|o| o.kind == kind).map(|o| o.payload)
}

/** The av1C box must describe the same bit depth, monochrome flag and subsampling as the sequence header it accompanies. */
fn check_av1c(seq: &Sequence, av1c: &Av1C) -> Result<()> {
    let color = &seq.color;
    let depth_ok = match color.bit_depth {
        12 => av1c.high_bitdepth && av1c.twelve_bit,
        10 => av1c.high_bitdepth && !av1c.twelve_bit,
        _ => !av1c.high_bitdepth,
    };
    let layout_ok = color.mono_chrome == av1c.monochrome
        && color.subsampling_x == av1c.subsampling_x
        && color.subsampling_y == av1c.subsampling_y;
    if depth_ok && layout_ok {
        Ok(())
    } else {
        Err(Error::Malformed("av1C disagrees with the sequence header"))
    }
}
