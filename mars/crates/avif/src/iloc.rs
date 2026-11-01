/** The iloc box: where each item's bytes are, and the reader that copies them out, bounded to the bytes that exist. */
use crate::boxes::full_box;
use crate::constants::{
    ILOC_METHOD_MASK, ILOC_VERSION_WITH_METHOD, ILOC_WIDTHS, LOCATION_FILE, LOCATION_IDAT,
    MAX_EXTENTS, MAX_ITEMS, NIBBLE_BITS, NIBBLE_MASK,
};
use crate::error::{Error, Result};
use crate::reader::Reader;

/** A contiguous byte range, relative to the location's base. */
#[derive(Clone, Copy)]
pub(crate) struct Extent {
    pub(crate) offset: u64,
    pub(crate) length: u64,
}

/** Where one item's bytes live: a construction method, a base offset and the extents to concatenate. */
pub(crate) struct Location {
    pub(crate) id: u32,
    pub(crate) method: u8,
    pub(crate) data_reference_index: u16,
    pub(crate) base: u64,
    pub(crate) extents: Vec<Extent>,
}

/** Parse every location in an iloc box. Versions 0 to 2 differ in id width and in the method and index fields. */
pub(crate) fn parse(body: &[u8]) -> Result<Vec<Location>> {
    let (version, _, rest) = full_box(body)?;
    let mut r = Reader::new(rest);
    let sizes = r.byte()?;
    let offset_size = usize::from(sizes >> NIBBLE_BITS);
    let length_size = usize::from(sizes & NIBBLE_MASK);
    let sizes2 = r.byte()?;
    let base_size = usize::from(sizes2 >> NIBBLE_BITS);
    let index_size = if version >= ILOC_VERSION_WITH_METHOD {
        usize::from(sizes2 & NIBBLE_MASK)
    } else {
        0
    };
    for width in [offset_size, length_size, base_size, index_size] {
        if !ILOC_WIDTHS.contains(&width) {
            return Err(Error::Malformed("iloc field width"));
        }
    }
    let count = if version < 2 {
        usize::from(r.u16()?)
    } else {
        r.u32()? as usize
    };
    if count > MAX_ITEMS {
        return Err(Error::Limit("iloc item count"));
    }
    let widths = [offset_size, length_size, base_size, index_size];
    let mut locations = Vec::new();
    let mut extents_seen = 0usize;
    for _ in 0..count {
        locations.push(parse_location(&mut r, version, widths, &mut extents_seen)?);
    }
    Ok(locations)
}

/** One item's entry in iloc, with its widths already validated. The extent total is shared across locations so the whole box stays bounded. */
fn parse_location(
    r: &mut Reader<'_>,
    version: u8,
    widths: [usize; 4],
    extents_seen: &mut usize,
) -> Result<Location> {
    let [offset_size, length_size, base_size, index_size] = widths;
    let id = if version < 2 {
        u32::from(r.u16()?)
    } else {
        r.u32()?
    };
    let method = if version >= ILOC_VERSION_WITH_METHOD {
        (r.u16()? & ILOC_METHOD_MASK) as u8
    } else {
        LOCATION_FILE
    };
    let data_reference_index = r.u16()?;
    let base = r.uint(base_size)?;
    let count = usize::from(r.u16()?);
    let widths = [offset_size, length_size, index_size];
    let extents = read_extents(r, count, widths, extents_seen)?;
    Ok(Location {
        id,
        method,
        data_reference_index,
        base,
        extents,
    })
}

/** Read `count` extents. The count is checked against the bytes that remain and the running total before any capacity is reserved. Zero-width extents cost no input bytes, so only one is allowed per location. */
fn read_extents(
    r: &mut Reader<'_>,
    count: usize,
    [offset_size, length_size, index_size]: [usize; 3],
    extents_seen: &mut usize,
) -> Result<Vec<Extent>> {
    if count > MAX_EXTENTS || *extents_seen + count > MAX_EXTENTS {
        return Err(Error::Limit("extent count"));
    }
    let per_extent = index_size + offset_size + length_size;
    if per_extent == 0 {
        if count > 1 {
            return Err(Error::Limit("zero-width extent count"));
        }
    } else if count
        .checked_mul(per_extent)
        .is_none_or(|bytes| bytes > r.remaining())
    {
        return Err(Error::Truncated);
    }
    *extents_seen += count;
    let mut extents = Vec::with_capacity(count);
    for _ in 0..count {
        if index_size > 0 {
            r.uint(index_size)?; // extent_index picks among external files; Mars reads only the one file.
        }
        let offset = r.uint(offset_size)?;
        let length = r.uint(length_size)?;
        extents.push(Extent { offset, length });
    }
    Ok(extents)
}

/** Concatenate an item's extents from `base`, which is the file for method 0 and the idat payload for method 1. */
pub(crate) fn read(base: &[u8], location: &Location) -> Result<Vec<u8>> {
    if location.method != LOCATION_FILE && location.method != LOCATION_IDAT {
        return Err(Error::Unsupported("item construction method"));
    }
    if location.data_reference_index != 0 {
        return Err(Error::Unsupported("external item data"));
    }
    let mut out = Vec::new();
    for extent in &location.extents {
        let start = location
            .base
            .checked_add(extent.offset)
            .ok_or(Error::Malformed("extent offset"))?;
        let end = start
            .checked_add(extent.length)
            .ok_or(Error::Malformed("extent length"))?;
        let (start, end) = (to_index(start)?, to_index(end)?);
        let bytes = base.get(start..end).ok_or(Error::Truncated)?;
        if out.len() + bytes.len() > base.len() {
            return Err(Error::Limit("item size"));
        }
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

/** A file offset as an index, refusing values the platform cannot address. */
fn to_index(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| Error::Malformed("extent offset"))
}
