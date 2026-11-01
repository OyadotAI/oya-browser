/** The CDEF stage (spec 7.15): every 8x8 block of a frame whose 64x64 unit carries a CDEF strength is deringed along its detected edge direction, using the deblocked samples as input and writing to a fresh copy of the planes. Blocks with no strength, or whose four 4x4 units are all skipped, keep their deblocked samples. */
mod direction;
mod filter;

use crate::frame::FrameHeader;
use crate::recon::Plane;
use crate::sequence::Sequence;
use crate::tile::{FrameState, MI_SIZE};
use filter::{Block, Region, floor_log2};

/** A CDEF block is eight luma samples, which is two 4x4 units each way. */
const BLOCK_MI: usize = 2;
const BLOCK_SAMPLES: usize = 8;
/** The largest variance scale a primary strength is boosted by, as a floor-log2 cap. */
const VAR_STR_MAX: u32 = 12;
/** Variance is measured after dropping this many bits, and the primary boost is (pri * (4 + scale) + 8) >> 4. */
const VAR_DROP_SHIFT: u32 = 6;
const PRI_BASE: i32 = 4;
const PRI_ROUND: i32 = 8;
const PRI_SHIFT: u32 = 4;
/** The damping of chroma is one less than luma's. */
const CHROMA_DAMPING_STEP: i32 = 1;

/** The direction a chroma block takes from the luma direction, indexed by the subsampling (spec Cdef_Uv_Dir). */
const UV_DIR: [[[usize; 8]; 2]; 2] = [
    [[0, 1, 2, 3, 4, 5, 6, 7], [1, 2, 2, 2, 3, 4, 6, 0]],
    [[7, 0, 2, 4, 5, 6, 6, 6], [0, 1, 2, 3, 4, 5, 6, 7]],
];

/** The CDEF output planes of a frame: the deblocked planes with every filtered 8x8 block replaced. The input is `fs.planes`, which stays untouched because later stages read it. */
pub(crate) fn cdef_frame(seq: &Sequence, hdr: &FrameHeader, fs: &FrameState) -> [Plane; 3] {
    let mut out = fs.planes.clone();
    let (rows, cols) = (hdr.size.mi_rows as usize, hdr.size.mi_cols as usize);
    for r in (0..rows).step_by(BLOCK_MI) {
        for c in (0..cols).step_by(BLOCK_MI) {
            let idx = fs.cdef.get(r, c);
            if idx >= 0 {
                filter_block(seq, hdr, fs, &mut out, (r, c), idx as usize);
            }
        }
    }
    out
}

/** The CDEF block process (spec 7.15.1) for the 8x8 block at 4x4 position (r, c) with strength index `idx`. */
fn filter_block(
    seq: &Sequence,
    hdr: &FrameHeader,
    fs: &FrameState,
    out: &mut [Plane; 3],
    at: (usize, usize),
    idx: usize,
) {
    if skipped(fs, at) {
        return;
    }
    let (r, c) = at;
    let bd = u32::from(seq.color.bit_depth);
    let cs = bd - 8;
    let (y_dir, var) = direction::search(&fs.planes[0], r, c, bd);
    let cdef = &hdr.cdef;
    let pri = i32::from(cdef.y_pri[idx]) << cs;
    let sec = i32::from(cdef.y_sec[idx]) << cs;
    let dir = if pri == 0 { 0 } else { y_dir };
    let pri = adjust_primary(pri, var);
    let damping = i32::from(cdef.damping) + cs as i32;
    let luma = Block {
        x0: c * MI_SIZE,
        y0: r * MI_SIZE,
        w: BLOCK_SAMPLES,
        h: BLOCK_SAMPLES,
        pri,
        sec,
        damping,
        dir,
        coeff_shift: cs,
    };
    filter::filter_block(&fs.planes[0], &mut out[0], region(hdr, 0, 0), luma);
    if seq.color.num_planes() > 1 {
        filter_chroma(seq, hdr, fs, out, at, idx, y_dir);
    }
}

/** The chroma planes of one block: their own strengths, the direction remapped for the subsampling, and one less damping. */
fn filter_chroma(
    seq: &Sequence,
    hdr: &FrameHeader,
    fs: &FrameState,
    out: &mut [Plane; 3],
    at: (usize, usize),
    idx: usize,
    y_dir: usize,
) {
    let (sub_x, sub_y) = (
        usize::from(seq.color.subsampling_x),
        usize::from(seq.color.subsampling_y),
    );
    let cs = u32::from(seq.color.bit_depth) - 8;
    let cdef = &hdr.cdef;
    let pri = i32::from(cdef.uv_pri[idx]) << cs;
    let sec = i32::from(cdef.uv_sec[idx]) << cs;
    let dir = if pri == 0 {
        0
    } else {
        UV_DIR[sub_x][sub_y][y_dir]
    };
    let damping = i32::from(cdef.damping) + cs as i32 - CHROMA_DAMPING_STEP;
    for (plane, dst) in out.iter_mut().enumerate().skip(1) {
        let block = Block {
            x0: (at.1 * MI_SIZE) >> sub_x,
            y0: (at.0 * MI_SIZE) >> sub_y,
            w: BLOCK_SAMPLES >> sub_x,
            h: BLOCK_SAMPLES >> sub_y,
            pri,
            sec,
            damping,
            dir,
            coeff_shift: cs,
        };
        filter::filter_block(&fs.planes[plane], dst, region(hdr, sub_x, sub_y), block);
    }
}

/** The primary strength after the variance boost (spec 7.15.1 steps 4 and 5): a flat block gets none, a busy one a stronger filter. */
fn adjust_primary(pri: i32, var: i64) -> i32 {
    if var == 0 {
        return 0;
    }
    let scaled = var >> VAR_DROP_SHIFT;
    let var_str = if scaled != 0 {
        floor_log2(scaled as u32).min(VAR_STR_MAX) as i32
    } else {
        0
    };
    (pri * (PRI_BASE + var_str) + PRI_ROUND) >> PRI_SHIFT
}

/** Whether all four 4x4 units of the 8x8 block are skipped, which exempts the block from filtering. */
fn skipped(fs: &FrameState, (r, c): (usize, usize)) -> bool {
    fs.mi.at(r, c).skip
        && fs.mi.at(r, c + 1).skip
        && fs.mi.at(r + 1, c).skip
        && fs.mi.at(r + 1, c + 1).skip
}

/** The tap region of one plane: the whole mode-info grid, with that plane's subsampling. */
fn region(hdr: &FrameHeader, sub_x: usize, sub_y: usize) -> Region {
    Region {
        mi_rows: hdr.size.mi_rows as isize,
        mi_cols: hdr.size.mi_cols as isize,
        sub_x: sub_x as u32,
        sub_y: sub_y as u32,
    }
}

#[cfg(test)]
mod tests;
