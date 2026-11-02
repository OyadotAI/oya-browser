/** The simplest intra predictors (spec 7.11.2.2 and 7.11.2.5): the Paeth predictor, which picks whichever of the left, above or corner samples is closest to a gradient estimate, and DC, which fills the block with the average of its available edges. */
use super::edges::Edges;

/** Paeth prediction: each sample takes the left, above or corner value nearest to above + left - corner, preferring left, then above. */
pub(super) fn paeth(e: &Edges, w: usize, h: usize) -> Vec<i32> {
    let corner = e.above(-1);
    let mut pred = vec![0i32; w * h];
    for i in 0..h {
        let left = e.left(i as isize);
        for j in 0..w {
            let above = e.above(j as isize);
            let base = above + left - corner;
            let (p_left, p_top, p_corner) = (
                (base - left).abs(),
                (base - above).abs(),
                (base - corner).abs(),
            );
            pred[i * w + j] = if p_left <= p_top && p_left <= p_corner {
                left
            } else if p_top <= p_corner {
                above
            } else {
                corner
            };
        }
    }
    pred
}

/** The sum of the first n entries of an edge, read from index 0. */
fn edge_sum(e: &Edges, n: usize, left: bool) -> i32 {
    (0..n as isize)
        .map(|i| if left { e.left(i) } else { e.above(i) })
        .sum()
}

/** The average of the available edges, rounded: both sides when both exist, otherwise the one that does, otherwise the mid grey of the bit depth. */
fn dc_value(e: &Edges, req: &super::Request) -> i32 {
    let (w, h) = (1usize << req.log2w, 1usize << req.log2h);
    let mid = 1i32 << (req.bit_depth - 1);
    match (req.have_left, req.have_above) {
        (true, true) => {
            let sum = edge_sum(e, w, false) + edge_sum(e, h, true) + ((w + h) >> 1) as i32;
            sum / (w + h) as i32
        }
        (true, false) => (edge_sum(e, h, true) + (h >> 1) as i32) >> req.log2h,
        (false, true) => (edge_sum(e, w, false) + (w >> 1) as i32) >> req.log2w,
        (false, false) => mid,
    }
}

/** DC prediction: every sample is the same average value (spec 7.11.2.5). */
pub(super) fn dc(e: &Edges, req: &super::Request) -> Vec<i32> {
    let (w, h) = (1usize << req.log2w, 1usize << req.log2h);
    vec![dc_value(e, req); w * h]
}
