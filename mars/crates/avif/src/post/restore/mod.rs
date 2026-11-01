/** The loop restoration stage (spec 7.17): every restoration unit of a plane is filtered by its own Wiener or self-guided filter. The output starts as a copy of the CDEF planes, and only the blocks of units that ask for restoration are overwritten. Blocks are the runs of 4x4 units that share a unit and a 64-row stripe, which the spec allows because the result does not depend on the block size. */
mod sgr;
mod source;
mod wiener;

use crate::av1::{RESTORE_NONE, RESTORE_SGRPROJ, RESTORE_WIENER};
use crate::frame::FrameHeader;
use crate::recon::Plane;
use crate::sequence::Sequence;
use crate::tile::{FrameState, MI_SIZE};
use source::{Source, Window};

/** Stripes are 64 luma rows high and start eight rows above zero. */
const STRIPE_HEIGHT: isize = 64;
const STRIPE_OFFSET: isize = 8;

/** The restored planes of a frame: `cdef` with every block of a restored unit replaced. `deblocked` supplies the rows just outside each stripe, which the spec takes before CDEF. */
pub(crate) fn restore_frame(
    seq: &Sequence,
    hdr: &FrameHeader,
    fs: &FrameState,
    deblocked: &[Plane; 3],
    cdef: &[Plane; 3],
) -> [Plane; 3] {
    let mut out = cdef.clone();
    if !hdr.lr.uses_lr {
        return out;
    }
    for (plane, dst) in out.iter_mut().enumerate().take(seq.color.num_planes()) {
        if hdr.lr.types[plane] != RESTORE_NONE {
            restore_plane(seq, hdr, fs, plane, (deblocked, cdef), dst);
        }
    }
    out
}

/** Filter every block of one plane whose unit is restored. */
fn restore_plane(
    seq: &Sequence,
    hdr: &FrameHeader,
    fs: &FrameState,
    plane: usize,
    (deblocked, cdef): (&[Plane; 3], &[Plane; 3]),
    out: &mut Plane,
) {
    let (sub_x, sub_y) = subsampling(seq, plane);
    let units = &fs.lr[plane];
    let unit_size = hdr.lr.unit_size[plane] as usize;
    let (fh, uw) = (
        hdr.size.frame_height as usize,
        hdr.size.upscaled_width as usize,
    );
    let (plane_w, plane_h) = (round2(uw, sub_x), round2(fh, sub_y));
    let rows = runs(fh.div_ceil(MI_SIZE), |row| {
        let luma = row * MI_SIZE + STRIPE_OFFSET as usize;
        let stripe = luma / STRIPE_HEIGHT as usize;
        let unit_row = (luma >> sub_y) / unit_size;
        (stripe, unit_row.min(units.unit_rows - 1))
    });
    let cols = runs(uw.div_ceil(MI_SIZE), |col| {
        ((col * MI_SIZE) >> sub_x) / unit_size
    });
    let bd = u32::from(seq.color.bit_depth);
    for &(r0, r1, (stripe, unit_row)) in &rows {
        for &(c0, c1, unit_col) in &cols {
            let unit_col = unit_col.min(units.unit_cols - 1);
            let unit = units.units[unit_row * units.unit_cols + unit_col];
            let rect = block_rect((r0, r1), (c0, c1), (sub_x, sub_y), (plane_w, plane_h));
            if unit.kind == RESTORE_NONE || rect.2 == 0 || rect.3 == 0 {
                continue;
            }
            let start = (stripe as isize * STRIPE_HEIGHT - STRIPE_OFFSET) >> sub_y;
            let src = Source {
                cur: &deblocked[plane],
                cdef: &cdef[plane],
                end_x: plane_w as isize - 1,
                end_y: plane_h as isize - 1,
                stripe_start: start,
                stripe_end: start + (STRIPE_HEIGHT >> sub_y) - 1,
            };
            let win = Window::fetch(&src, rect.0, rect.1, rect.2, rect.3);
            if unit.kind == RESTORE_WIENER {
                wiener::filter_block(&win, out, rect, unit.wiener, bd);
            } else if unit.kind == RESTORE_SGRPROJ {
                let xqd = [unit.sgr_xqd[0], unit.sgr_xqd[1]];
                sgr::filter_block(
                    &win,
                    &cdef[plane],
                    out,
                    rect,
                    (usize::from(unit.sgr_set), xqd),
                    bd,
                );
            }
        }
    }
}

/** The plane rectangle (x0, y0, w, h) that a run of 4x4 rows and columns covers, clipped to the plane. */
fn block_rect(
    (r0, r1): (usize, usize),
    (c0, c1): (usize, usize),
    (sub_x, sub_y): (usize, usize),
    (plane_w, plane_h): (usize, usize),
) -> (isize, isize, usize, usize) {
    let (x0, y0) = ((c0 * MI_SIZE) >> sub_x, (r0 * MI_SIZE) >> sub_y);
    let x1 = ((c1 * MI_SIZE) >> sub_x).min(plane_w);
    let y1 = ((r1 * MI_SIZE) >> sub_y).min(plane_h);
    (
        x0 as isize,
        y0 as isize,
        x1.saturating_sub(x0),
        y1.saturating_sub(y0),
    )
}

/** Consecutive indices of `0..n` that share a key, as (start, end, key). Each run becomes one block. */
fn runs<K: PartialEq + Copy>(n: usize, key: impl Fn(usize) -> K) -> Vec<(usize, usize, K)> {
    let mut out: Vec<(usize, usize, K)> = Vec::new();
    for i in 0..n {
        let k = key(i);
        match out.last_mut() {
            Some(last) if last.2 == k => last.1 = i + 1,
            _ => out.push((i, i + 1, k)),
        }
    }
    out
}

/** Round2(x, n) for an unsigned value: halve with rounding, for a shift of n bits. */
fn round2(x: usize, n: usize) -> usize {
    if n == 0 { x } else { (x + (1 << (n - 1))) >> n }
}

/** The subsampling of a plane as (x, y) shifts. */
fn subsampling(seq: &Sequence, plane: usize) -> (usize, usize) {
    if plane == 0 {
        (0, 0)
    } else {
        (
            usize::from(seq.color.subsampling_x),
            usize::from(seq.color.subsampling_y),
        )
    }
}

#[cfg(test)]
mod tests;
