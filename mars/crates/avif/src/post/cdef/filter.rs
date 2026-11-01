/** CDEF filter (spec 7.15.3): for one 8x8 block of one plane, a weighted sum of the primary taps along the block's direction and the secondary taps across it, each difference damped by how far it strays, then clamped to the range of the samples it saw. Taps outside the frame's mode-info grid are skipped. */
use crate::recon::Plane;

/** The offsets of the taps along each direction: for each of the two distances, the (row, column) step. */
const DIRECTIONS: [[[isize; 2]; 2]; 8] = [
    [[-1, 1], [-2, 2]],
    [[0, 1], [-1, 2]],
    [[0, 1], [0, 2]],
    [[0, 1], [1, 2]],
    [[1, 1], [2, 2]],
    [[1, 0], [2, 1]],
    [[1, 0], [2, 0]],
    [[1, 0], [2, -1]],
];
/** The tap weights of the primary and secondary filters, indexed by the strength's low bit and the distance. */
const PRI_TAPS: [[i32; 2]; 2] = [[4, 2], [3, 3]];
const SEC_TAPS: [[i32; 2]; 2] = [[2, 1], [2, 1]];
/** The rounding and scale of the summed correction: it is added as (8 + sum) >> 4. */
const SUM_ROUND: i32 = 8;
const SUM_SHIFT: u32 = 4;
/** The two directions a secondary tap is taken from, either side of the block's direction. */
const SEC_OFFSETS: [isize; 2] = [-2, 2];
/** Direction indices wrap around eight. */
const DIR_MASK: isize = 7;

/** The mode-info grid the taps must stay inside, and the subsampling of the plane being filtered. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Region {
    pub(super) mi_rows: isize,
    pub(super) mi_cols: isize,
    pub(super) sub_x: u32,
    pub(super) sub_y: u32,
}

impl Region {
    /** The sample at a plane position, or None when it lies outside the grid (spec cdef_get_at with is_inside_filter_region). */
    fn sample(&self, plane: &Plane, y: isize, x: isize) -> Option<i32> {
        let r = (y << self.sub_y) >> 2;
        let c = (x << self.sub_x) >> 2;
        let inside = (0..self.mi_rows).contains(&r) && (0..self.mi_cols).contains(&c);
        if inside && y >= 0 && x >= 0 {
            Some(plane.get(x as usize, y as usize))
        } else {
            None
        }
    }
}

/** One block to filter: its top-left sample in the plane, its size in that plane, and the filter settings. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Block {
    pub(super) x0: usize,
    pub(super) y0: usize,
    pub(super) w: usize,
    pub(super) h: usize,
    pub(super) pri: i32,
    pub(super) sec: i32,
    pub(super) damping: i32,
    pub(super) dir: usize,
    pub(super) coeff_shift: u32,
}

/** Filter one block from `src` into `dst` (spec 7.15.3). The sum starts at the sample itself, so a block with no usable taps keeps its value. */
pub(super) fn filter_block(src: &Plane, dst: &mut Plane, region: Region, b: Block) {
    for i in 0..b.h {
        for j in 0..b.w {
            let (y, x) = ((b.y0 + i) as isize, (b.x0 + j) as isize);
            let centre = src.get(x as usize, y as usize);
            let (sum, min, max) = taps_sum(src, region, b, (y, x), centre);
            let adjust = (SUM_ROUND + sum - i32::from(sum < 0)) >> SUM_SHIFT;
            dst.set(
                x as usize,
                y as usize,
                (centre + adjust).clamp(min, max) as u16,
            );
        }
    }
}

/** The weighted sum of the constrained differences over every tap of one sample, with the smallest and largest tap values seen (the clamp range). */
fn taps_sum(
    src: &Plane,
    region: Region,
    b: Block,
    at: (isize, isize),
    centre: i32,
) -> (i32, i32, i32) {
    let mut acc = Acc {
        sum: 0,
        min: centre,
        max: centre,
    };
    let tier = ((b.pri >> b.coeff_shift) & 1) as usize;
    for k in 0..2 {
        for sign in [-1isize, 1] {
            let p = tap(src, region, at, sign, DIRECTIONS[b.dir][k]);
            acc.add(p, PRI_TAPS[tier][k], b.pri, b.damping, centre);
            for off in SEC_OFFSETS {
                let dir = ((b.dir as isize + off) & DIR_MASK) as usize;
                let s = tap(src, region, at, sign, DIRECTIONS[dir][k]);
                acc.add(s, SEC_TAPS[tier][k], b.sec, b.damping, centre);
            }
        }
    }
    (acc.sum, acc.min, acc.max)
}

/** The running sum and clamp range of the taps of one sample. */
struct Acc {
    sum: i32,
    min: i32,
    max: i32,
}

impl Acc {
    /** Add one tap if it was available: its constrained difference, weighted, and its value to the clamp range. */
    fn add(&mut self, tap: Option<i32>, weight: i32, strength: i32, damping: i32, centre: i32) {
        if let Some(p) = tap {
            self.sum += weight * constrain(p - centre, strength, damping);
            self.min = self.min.min(p);
            self.max = self.max.max(p);
        }
    }
}

/** The sample at `distance` along `step` from the block sample, signed by `sign`, or None outside the grid. */
fn tap(
    src: &Plane,
    region: Region,
    at: (isize, isize),
    sign: isize,
    step: [isize; 2],
) -> Option<i32> {
    let y = at.0 + sign * step[0];
    let x = at.1 + sign * step[1];
    region.sample(src, y, x)
}

/** constrain(diff, threshold, damping) (spec 7.15.3): the difference, pulled towards zero by an amount that falls as the difference grows past the threshold. */
pub(super) fn constrain(diff: i32, threshold: i32, damping: i32) -> i32 {
    if threshold == 0 {
        return 0;
    }
    let adj = (damping - floor_log2(threshold as u32) as i32).max(0);
    let mag = diff.abs();
    let value = (threshold - (mag >> adj)).clamp(0, mag);
    if diff < 0 { -value } else { value }
}

/** FloorLog2: the position of the highest set bit. Callers pass a non-zero value. */
pub(super) fn floor_log2(v: u32) -> u32 {
    31 - v.leading_zeros()
}
