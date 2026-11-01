/** Chroma-from-luma prediction (spec 7.11.5): the chroma block's DC prediction is corrected by the scaled deviation of the co-located reconstructed luma from its mean, so that chroma follows the luma texture. */
use crate::recon::Plane;
use crate::recon::math::{clip3, round2, round2_signed};

/** Fractional bits the subsampled luma is kept at, and the shift of the alpha scale. */
const LUMA_FRAC_BITS: u32 = 3;
const ALPHA_BITS: u32 = 6;

/** The place and the luma limits of one chroma transform block: its top-left sample in chroma, its size in log2, the subsampling, and the luma area that was actually decoded (the spec's MaxLumaW and MaxLumaH). */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CflBlock {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) log2w: u32,
    pub(crate) log2h: u32,
    pub(crate) sub_x: usize,
    pub(crate) sub_y: usize,
    pub(crate) max_luma_w: usize,
    pub(crate) max_luma_h: usize,
}

/** The subsampled luma of a block with three fractional bits, and its mean. Positions past the decoded luma repeat the last decoded column or row. */
fn subsampled_luma(luma: &Plane, b: &CflBlock) -> (Vec<i64>, i64) {
    let (w, h) = (1usize << b.log2w, 1usize << b.log2h);
    let mut l = vec![0i64; w * h];
    let mut total = 0i64;
    for i in 0..h {
        let luma_y = ((b.y + i) << b.sub_y).min(b.max_luma_h - (1 << b.sub_y));
        for j in 0..w {
            let luma_x = ((b.x + j) << b.sub_x).min(b.max_luma_w - (1 << b.sub_x));
            let mut t = 0i64;
            for dy in 0..=b.sub_y {
                for dx in 0..=b.sub_x {
                    t += i64::from(luma.get(luma_x + dx, luma_y + dy));
                }
            }
            let v = t << (LUMA_FRAC_BITS as usize - b.sub_x - b.sub_y);
            l[i * w + j] = v;
            total += v;
        }
    }
    let mean = round2(total, b.log2w + b.log2h);
    (l, mean)
}

/** Add the scaled luma deviation to the chroma block's prediction, which must already hold its DC prediction. `alpha` is the signed CFL alpha of the plane and `clip` the largest sample value. */
pub(crate) fn predict(chroma: &mut Plane, luma: &Plane, b: &CflBlock, alpha: i32, clip: i32) {
    let (w, h) = (1usize << b.log2w, 1usize << b.log2h);
    let (l, mean) = subsampled_luma(luma, b);
    for i in 0..h {
        for j in 0..w {
            let (x, y) = (b.x + j, b.y + i);
            let dc = chroma.get(x, y) as i64;
            let scaled = round2_signed(i64::from(alpha) * (l[i * w + j] - mean), ALPHA_BITS);
            chroma.set(x, y, clip3(0, i64::from(clip), dc + scaled) as u16);
        }
    }
}
