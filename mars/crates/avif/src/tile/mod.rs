/** AV1 tile decoding for intra frames (spec 5.11 and the parsing parts of 8.3): the tile group syntax, the partition and block trees, the mode info, palette, transform size, residual and coefficient syntax, and the dequantisation of each transform block. The symbol stream of each tile is checked to end in step with the bitstream. Prediction and reconstruction are the next stage. */
mod block;
mod coeff_ctx;
mod coeffs;
mod constants;
mod decoded;
mod decoder;
mod dequant;
mod grid;
mod group;
mod intra_info;
mod intra_mode;
mod lr;
mod palette;
mod partition;
mod residual;
mod samples;
mod scan;
mod scan_tables;
mod state;
// The inter-frame tables are generated with the rest, for inter syntax that is not built yet.
#[allow(dead_code)]
mod tables;
mod tx_size;
mod tx_type;

#[cfg(test)]
mod tests;

pub(crate) use constants::{MAX_LOOP_FILTER, MI_SIZE, SGRPROJ_PRJ_BITS};
pub(crate) use grid::MiInfo;
pub(crate) use state::FrameState;
pub(crate) use tables::{
    NUM_4X4_BLOCKS_HIGH, NUM_4X4_BLOCKS_WIDE, SGR_PARAMS, SUBSAMPLED_SIZE, TX_HEIGHT, TX_WIDTH,
};

use crate::error::{Error, Result};
use crate::frame::FrameHeader;
use crate::sequence::Sequence;
use decoder::{TileBounds, TileDecoder};

/** Decode every tile of an intra frame from its tile group payloads, returning the frame's state. Each tile must appear exactly once across the groups. */
pub(crate) fn decode_frame(
    seq: &Sequence,
    hdr: &FrameHeader,
    groups: &[&[u8]],
) -> Result<FrameState> {
    let mut fs = FrameState::new(seq, hdr);
    let total = (hdr.tiles.cols * hdr.tiles.rows) as usize;
    let mut seen = vec![false; total];
    for group in groups {
        for tile in group::split_tiles(hdr, group)? {
            if seen[tile.num] {
                return Err(Error::Malformed("tile repeated"));
            }
            seen[tile.num] = true;
            let bounds = tile_bounds(hdr, tile.num);
            TileDecoder::new(seq, hdr, &mut fs, bounds, tile.data)?.decode()?;
        }
    }
    if seen.iter().any(|&done| !done) {
        return Err(Error::Malformed("missing tile"));
    }
    Ok(fs)
}

/** The mode-info rectangle of tile `num`, in raster order over the frame's tile grid. */
fn tile_bounds(hdr: &FrameHeader, num: usize) -> TileBounds {
    let cols = hdr.tiles.cols as usize;
    let (row, col) = (num / cols, num % cols);
    let rows_at = &hdr.tiles.mi_row_starts;
    let cols_at = &hdr.tiles.mi_col_starts;
    TileBounds {
        mi_row_start: rows_at[row] as usize,
        mi_row_end: rows_at[row + 1] as usize,
        mi_col_start: cols_at[col] as usize,
        mi_col_end: cols_at[col + 1] as usize,
    }
}
