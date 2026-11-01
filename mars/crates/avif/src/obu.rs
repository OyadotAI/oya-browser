/** OBU framing (spec section 5.3): the low-overhead bitstream split into typed payloads, with leb128 sizes checked against the bytes present. */
use crate::av1::{
    LEB128_BITS, LEB128_MAX_BYTES, LEB128_MORE, LEB128_VALUE_BITS, MAX_OBUS, OBU_EXTENSION_BIT,
    OBU_EXTENSION_LEN, OBU_FORBIDDEN_BIT, OBU_HAS_SIZE_BIT, OBU_HEADER_LEN, OBU_SPATIAL_MASK,
    OBU_SPATIAL_SHIFT, OBU_TEMPORAL_SHIFT, OBU_TYPE_MASK, OBU_TYPE_SHIFT,
};
use crate::error::{Error, Result};

/** One OBU: its type, its layer ids from the extension header, and its payload bytes. */
#[derive(Clone, Copy, Debug)]
pub(crate) struct Obu<'a> {
    pub(crate) kind: u8,
    pub(crate) temporal_id: u8,
    pub(crate) spatial_id: u8,
    pub(crate) payload: &'a [u8],
}

/** Read one leb128 value at `pos`, advancing it. Values must fit in 32 bits and use at most eight bytes. */
pub(crate) fn leb128(data: &[u8], pos: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    for index in 0..LEB128_MAX_BYTES {
        let byte = *data.get(*pos).ok_or(Error::Truncated)?;
        *pos += 1;
        value |= u64::from(byte & !LEB128_MORE) << (index as u32 * LEB128_BITS);
        if byte & LEB128_MORE == 0 {
            return if value >> LEB128_VALUE_BITS == 0 {
                Ok(value)
            } else {
                Err(Error::Malformed("leb128 value"))
            };
        }
    }
    Err(Error::Malformed("leb128 length"))
}

/** Split a low-overhead OBU stream into OBUs. Each OBU's size comes from its size field, or runs to the end when it has none. */
pub(crate) fn split(data: &[u8]) -> Result<Vec<Obu<'_>>> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() {
        if out.len() == MAX_OBUS {
            return Err(Error::Limit("OBU count"));
        }
        out.push(next_obu(data, &mut pos)?);
    }
    Ok(out)
}

/** Read the OBU that starts at `pos`, advancing `pos` past its payload. */
fn next_obu<'a>(data: &'a [u8], pos: &mut usize) -> Result<Obu<'a>> {
    let header = *data.get(*pos).ok_or(Error::Truncated)?;
    if header & OBU_FORBIDDEN_BIT != 0 {
        return Err(Error::Malformed("OBU forbidden bit"));
    }
    *pos += OBU_HEADER_LEN;
    let kind = (header >> OBU_TYPE_SHIFT) & OBU_TYPE_MASK;
    let (temporal_id, spatial_id) = if header & OBU_EXTENSION_BIT != 0 {
        *pos += OBU_EXTENSION_LEN;
        extension_ids(*data.get(*pos - OBU_EXTENSION_LEN).ok_or(Error::Truncated)?)
    } else {
        (0, 0)
    };
    let size = if header & OBU_HAS_SIZE_BIT != 0 {
        usize::try_from(leb128(data, pos)?).map_err(|_| Error::Malformed("OBU size"))?
    } else {
        data.len().saturating_sub(*pos)
    };
    let end = pos.checked_add(size).ok_or(Error::Malformed("OBU size"))?;
    let payload = data.get(*pos..end).ok_or(Error::Truncated)?;
    *pos = end;
    Ok(Obu {
        kind,
        temporal_id,
        spatial_id,
        payload,
    })
}

/** temporal_id and spatial_id from the one-byte OBU extension header. */
fn extension_ids(byte: u8) -> (u8, u8) {
    (
        byte >> OBU_TEMPORAL_SHIFT,
        (byte >> OBU_SPATIAL_SHIFT) & OBU_SPATIAL_MASK,
    )
}
