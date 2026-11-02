/** Sample filtering for one position of an edge (spec 7.14.6): the masks that decide whether and how far the samples are smoothed, then the narrow filter or one of the two wide low-pass filters. Samples are addressed by signed offset from the edge: offset 0 is q0 and offset -1 is p0. */
use super::level::Strength;
use crate::recon::Plane;

/** How many samples each side of the edge a filter reads, for each filter size: the 16-wide filter reads seven, the 8-wide four and the 4-wide two. */
const REACH_16: usize = 7;
const REACH_8: usize = 4;
const REACH_4: usize = 2;
/** The loaded taps: a window of sixteen with offset zero at the centre. */
const TAPS: usize = 16;
const TAP_CENTER: usize = 8;
/** The narrow filter's constants: the rounding of the filter taps, and the offset that centres samples on zero. */
const NARROW_ROUND_HI: i32 = 4;
const NARROW_ROUND_LO: i32 = 3;
const NARROW_SHIFT: u32 = 3;
const CENTRE_VALUE: i32 = 0x80;
/** The wide filter's taps: the offsets with unit weight, and the two-tap weight of the centre. */
const WIDE_CENTRE_TAP: i32 = 2;
const WIDE_LOG2_16: u32 = 4;
const WIDE_LOG2_8: u32 = 3;
const WIDE_N_16: isize = 6;
const WIDE_N_LUMA_8: isize = 3;
const WIDE_N_CHROMA_8: isize = 2;
/** A filter length of 4 is the narrow filter; chroma never runs the 8-tap mask. */
const LEN_4: usize = 4;
const LEN_6: usize = 6;
const LEN_8: usize = 8;
const LEN_16: usize = 16;
/** The largest filter size with a flat mask over four samples. */
const SIZE_8: usize = 8;
const SIZE_16: usize = 16;
const SIZE_4: usize = 4;

/** Where the samples of one edge line are: the first sample's position and the step across the edge (dx, dy). */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sample {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) dx: usize,
    pub(super) dy: usize,
}

impl Sample {
    /** The plane position at a signed offset from the edge. */
    fn at(&self, offset: isize) -> (usize, usize) {
        let x = self.x as isize + offset * self.dx as isize;
        let y = self.y as isize + offset * self.dy as isize;
        (x as usize, y as usize)
    }
}

/** The filter's settings for one plane: the largest filter size allowed, whether the plane is chroma, and the bit depth. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cfg {
    pub(super) size: usize,
    pub(super) chroma: bool,
    pub(super) bit_depth: u32,
}

/** The samples of one edge line, loaded before any is written. Index `TAP_CENTER + k` holds offset k; offsets outside the loaded reach stay zero and are never read. */
struct Taps {
    v: [i32; TAPS],
}

impl Taps {
    /** Load the offsets within `reach` samples each side of the edge. */
    fn load(plane: &Plane, site: Sample, reach: usize) -> Self {
        let mut v = [0; TAPS];
        for offset in -(reach as isize)..reach as isize {
            let (x, y) = site.at(offset);
            v[(TAP_CENTER as isize + offset) as usize] = plane.get(x, y);
        }
        Self { v }
    }

    /** The sample at a signed offset from the edge. */
    fn at(&self, offset: isize) -> i32 {
        self.v[(TAP_CENTER as isize + offset) as usize]
    }

    /** The k-th sample on the q side (k = 0 is q0). */
    fn q(&self, k: usize) -> i32 {
        self.at(k as isize)
    }

    /** The k-th sample on the p side (k = 0 is p0). */
    fn p(&self, k: usize) -> i32 {
        self.at(-(k as isize) - 1)
    }
}

/** The four decisions the masks make for one sample line. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Masks {
    hev: bool,
    filter: bool,
    flat: bool,
    flat2: bool,
}

/** The sample filter process (spec 7.14.6.1): choose the narrow filter, the 8-wide or the 16-wide low-pass filter from the masks, or leave the line alone when the edge is not a smooth step. */
pub(super) fn filter_sample(plane: &mut Plane, site: Sample, cfg: Cfg, s: &Strength) {
    let t = Taps::load(plane, site, reach(cfg.size));
    let m = masks(&t, cfg, s);
    if !m.filter {
        return;
    }
    if cfg.size == SIZE_4 || !m.flat {
        narrow(plane, site, &t, m.hev, cfg.bit_depth);
    } else if cfg.size == SIZE_8 || !m.flat2 {
        wide(plane, site, &t, WIDE_LOG2_8, cfg.chroma);
    } else {
        wide(plane, site, &t, WIDE_LOG2_16, cfg.chroma);
    }
}

/** How many samples each side of the edge the filter of this size reads. */
fn reach(size: usize) -> usize {
    match size {
        SIZE_16 => REACH_16,
        SIZE_8 => REACH_8,
        _ => REACH_4,
    }
}

/** The filter length the mask uses (spec 7.14.6.2): the number of taps each side that the mask compares. */
fn filter_len(cfg: Cfg) -> usize {
    if cfg.size == SIZE_4 {
        LEN_4
    } else if cfg.chroma {
        LEN_6
    } else if cfg.size == SIZE_8 {
        LEN_8
    } else {
        LEN_16
    }
}

/** The hev, filter, flat and flat2 masks of a sample line (spec 7.14.6.2). */
fn masks(t: &Taps, cfg: Cfg, s: &Strength) -> Masks {
    let shift = cfg.bit_depth - 8;
    let len = filter_len(cfg);
    let thresh = s.thresh << shift;
    let limit = s.limit << shift;
    let blimit = s.blimit << shift;
    let flat_limit = 1 << shift;
    Masks {
        hev: hev_mask(t, thresh),
        filter: filter_mask(t, len, limit, blimit),
        flat: cfg.size >= SIZE_8 && flat_mask(t, len, flat_limit),
        flat2: cfg.size >= SIZE_16 && flat2_mask(t, flat_limit),
    }
}

/** High edge variance: either side steps by more than the threshold between its first two samples. */
fn hev_mask(t: &Taps, thresh: i32) -> bool {
    (t.p(1) - t.p(0)).abs() > thresh || (t.q(1) - t.q(0)).abs() > thresh
}

/** The filter runs only when the samples on each side vary by no more than the limits, and the step across the edge is within the blimit. */
fn filter_mask(t: &Taps, len: usize, limit: i32, blimit: i32) -> bool {
    let near = (t.p(1) - t.p(0)).abs() <= limit
        && (t.q(1) - t.q(0)).abs() <= limit
        && (t.p(0) - t.q(0)).abs() * 2 + (t.p(1) - t.q(1)).abs() / 2 <= blimit;
    let p2q2 =
        len < LEN_6 || ((t.p(2) - t.p(1)).abs() <= limit && (t.q(2) - t.q(1)).abs() <= limit);
    let p3q3 =
        len < LEN_8 || ((t.p(3) - t.p(2)).abs() <= limit && (t.q(3) - t.q(2)).abs() <= limit);
    near && p2q2 && p3q3
}

/** Flat over the samples nearest the edge: each sample is within one unit of the sample at the edge. */
fn flat_mask(t: &Taps, len: usize, limit: i32) -> bool {
    let near = [1, 2]
        .iter()
        .all(|&k| (t.p(k) - t.p(0)).abs() <= limit && (t.q(k) - t.q(0)).abs() <= limit);
    let far = len < LEN_8
        || [3]
            .iter()
            .all(|&k| (t.p(k) - t.p(0)).abs() <= limit && (t.q(k) - t.q(0)).abs() <= limit);
    near && far
}

/** Flat over the outer samples, four to six from the edge, which the 16-wide filter needs. */
fn flat2_mask(t: &Taps, limit: i32) -> bool {
    [4, 5, 6]
        .iter()
        .all(|&k| (t.p(k) - t.p(0)).abs() <= limit && (t.q(k) - t.q(0)).abs() <= limit)
}

/** Write a value at a signed offset from the edge. */
fn put(plane: &mut Plane, site: Sample, offset: isize, value: i32) {
    let (x, y) = site.at(offset);
    plane.set(x, y, value as u16);
}

/** The narrow filter (spec 7.14.6.3): adjust the two samples nearest the edge on each side, and the outer pair too unless the edge has high variance. */
fn narrow(plane: &mut Plane, site: Sample, t: &Taps, hev: bool, bit_depth: u32) {
    let half = 1 << (bit_depth - 1);
    let clamp = |v: i32| v.clamp(-half, half - 1);
    let off = CENTRE_VALUE << (bit_depth - 8);
    let (ps1, ps0) = (t.p(1) - off, t.p(0) - off);
    let (qs0, qs1) = (t.q(0) - off, t.q(1) - off);
    let base = if hev { clamp(ps1 - qs1) } else { 0 };
    let f = clamp(base + 3 * (qs0 - ps0));
    let f1 = clamp(f + NARROW_ROUND_HI) >> NARROW_SHIFT;
    let f2 = clamp(f + NARROW_ROUND_LO) >> NARROW_SHIFT;
    put(plane, site, 0, clamp(qs0 - f1) + off);
    put(plane, site, -1, clamp(ps0 + f2) + off);
    if !hev {
        let outer = (f1 + 1) >> 1;
        put(plane, site, 1, clamp(qs1 - outer) + off);
        put(plane, site, -2, clamp(ps1 + outer) + off);
    }
}

/** The wide filter (spec 7.14.6.4): a low-pass over the flat region, with 2^log2 summed weights, written back for offsets -n to n-1. */
fn wide(plane: &mut Plane, site: Sample, t: &Taps, log2: u32, chroma: bool) {
    let n = wide_taps(log2, chroma);
    let n2 = if log2 == WIDE_LOG2_8 && !chroma { 0 } else { 1 };
    let mut out = [0i32; TAPS];
    for i in -n..n {
        let sum: i32 = (-n..=n)
            .map(|j| {
                let p = (i + j).clamp(-(n + 1), n);
                let tap = if j.abs() <= n2 { WIDE_CENTRE_TAP } else { 1 };
                t.at(p) * tap
            })
            .sum();
        out[(i + n) as usize] = (sum + (1 << (log2 - 1))) >> log2;
    }
    for i in -n..n {
        put(plane, site, i, out[(i + n) as usize]);
    }
}

/** The number of taps each side of the edge that the wide filter changes. */
fn wide_taps(log2: u32, chroma: bool) -> isize {
    if log2 == WIDE_LOG2_16 {
        WIDE_N_16
    } else if chroma {
        WIDE_N_CHROMA_8
    } else {
        WIDE_N_LUMA_8
    }
}
