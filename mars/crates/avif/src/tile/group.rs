/** Tile group syntax (spec tile_group_obu, 5.11.1): which tiles one tile group OBU carries, and the byte range each tile occupies inside it. The last tile of a group takes the rest of the payload; the others carry an explicit little-endian size. */
use crate::bits::BitReader;
use crate::error::{Error, Result};
use crate::frame::FrameHeader;

/** One tile of a tile group: its number in raster order over the frame's tiles, and its bytes. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TileData<'a> {
    pub(crate) num: usize,
    pub(crate) data: &'a [u8],
}

/** Split one tile group payload into its tiles. The payload is the whole tile_group_obu, after the frame header in a frame OBU. */
pub(crate) fn split_tiles<'a>(hdr: &FrameHeader, group: &'a [u8]) -> Result<Vec<TileData<'a>>> {
    let tiles = &hdr.tiles;
    let total = (tiles.cols * tiles.rows) as usize;
    let mut r = BitReader::new(group);
    let explicit_range = total > 1 && r.flag()?;
    let (start, end) = if explicit_range {
        let bits = tiles.cols_log2 + tiles.rows_log2;
        (r.f(bits)? as usize, r.f(bits)? as usize)
    } else {
        (0, total.saturating_sub(1))
    };
    if total == 0 || end < start || end >= total {
        return Err(Error::Malformed("tile group range"));
    }
    r.byte_align();
    let mut pos = r.bit_pos() / 8;
    let size_bytes = tiles.tile_size_bytes as usize;
    let mut out = Vec::with_capacity(end - start + 1);
    for num in start..=end {
        let size = if num == end {
            group.len().checked_sub(pos).ok_or(Error::Truncated)?
        } else {
            let coded = read_le(group, pos, size_bytes)?;
            pos += size_bytes;
            coded.checked_add(1).ok_or(Error::Malformed("tile size"))?
        };
        let end_pos = pos.checked_add(size).ok_or(Error::Malformed("tile size"))?;
        let data = group.get(pos..end_pos).ok_or(Error::Truncated)?;
        out.push(TileData { num, data });
        pos = end_pos;
    }
    Ok(out)
}

/** An unsigned little-endian value of `width` bytes at `pos`. Width zero reads nothing and yields zero. */
fn read_le(data: &[u8], pos: usize, width: usize) -> Result<usize> {
    let bytes = data.get(pos..pos + width).ok_or(Error::Truncated)?;
    Ok(bytes
        .iter()
        .rev()
        .fold(0usize, |acc, &byte| (acc << 8) | usize::from(byte)))
}
