/** Output (spec 7.18 and the AVIF colour rules): crops the reconstructed planes to the picture, upsamples chroma, converts YUV to 8-bit RGBA with the picture's matrix and range, scales deeper samples down to 8 bits, and takes alpha from the alpha item when there is one. */
mod matrix;

use crate::error::{Error, Result};
use crate::recon::Plane;
use crate::sequence::Sequence;
use matrix::{Matrix, Range};

/** Full opacity, the alpha of a picture with no alpha item. */
const OPAQUE: u8 = 255;

/** The part of the decoded frame that is shown: its top-left sample and its size, in luma samples. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rect {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) w: usize,
    pub(crate) h: usize,
}

/** The RGBA rows of the rectangle `rect` of a decoded frame. Chroma of 4:2:0 and 4:2:2 is replicated across the samples it covers (nearest-sample upsampling). An empty rectangle or one that leaves the frame's index range is Malformed. */
pub(crate) fn rgba(
    seq: &Sequence,
    rect: Rect,
    planes: &[Plane; 3],
    alpha: Option<&Plane>,
) -> Result<Vec<u8>> {
    let color = &seq.color;
    let range = Range {
        full: color.color_range,
        bit_depth: u32::from(color.bit_depth),
    };
    let matrix = Matrix::from_code(color.matrix_coefficients);
    let (sub_x, sub_y) = (
        usize::from(color.subsampling_x),
        usize::from(color.subsampling_y),
    );
    let layout = Layout {
        sub_x,
        sub_y,
        mono: seq.color.num_planes() == 1,
        matrix,
        last: last_chroma(rect, sub_x, sub_y)?,
    };
    let out_len = rect
        .w
        .checked_mul(rect.h)
        .and_then(|px| px.checked_mul(4))
        .ok_or(Error::Limit("output size"))?;
    let mut out = Vec::with_capacity(out_len);
    for y in rect.y..rect.y + rect.h {
        for x in rect.x..rect.x + rect.w {
            let [r, g, b] = pixel(planes, (x, y), layout, range);
            let a = alpha.map_or(OPAQUE, |plane| scale_to_8(plane.get(x, y), range.bit_depth));
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    Ok(out)
}

/** The last chroma column and row the rectangle reads, after subsampling. A rectangle with no samples has no last sample, so it is refused. */
fn last_chroma(rect: Rect, sub_x: usize, sub_y: usize) -> Result<(usize, usize)> {
    let last_x = last_index(rect.x, rect.w)?;
    let last_y = last_index(rect.y, rect.h)?;
    Ok((last_x >> sub_x, last_y >> sub_y))
}

/** The index of the final sample of a run of `len` samples starting at `start`, with every step checked. */
fn last_index(start: usize, len: usize) -> Result<usize> {
    start
        .checked_add(len)
        .and_then(|end| end.checked_sub(1))
        .filter(|_| len > 0)
        .ok_or(Error::Malformed("empty output rectangle"))
}

/** How picture samples map to planes: the chroma subsampling, whether the picture is monochrome, and its matrix. */
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    sub_x: usize,
    sub_y: usize,
    mono: bool,
    matrix: Matrix,
    /** The last chroma sample column and row that the picture reads. */
    last: (usize, usize),
}

/** The 8-bit RGB of one picture sample: its luma, and the chroma of the sample that covers it. A monochrome picture has no chroma and is grey. */
fn pixel(planes: &[Plane; 3], (x, y): (usize, usize), layout: Layout, range: Range) -> [u8; 3] {
    let luma = range.luma(planes[0].get(x, y));
    if layout.mono {
        let grey = to_byte(luma);
        return [grey, grey, grey];
    }
    let u = chroma_at(&planes[1], (x, y), layout);
    let v = chroma_at(&planes[2], (x, y), layout);
    let rgb = match layout.matrix {
        Matrix::Identity => {
            let max = range.max();
            [v / max, luma, u / max]
        }
        Matrix::Weights { kr, kb } => {
            from_weights(luma, range.chroma_f(u), range.chroma_f(v), kr, kb)
        }
    };
    rgb.map(to_byte)
}

/** The chroma code at a luma position, interpolated bilinearly between the chroma samples whose centres surround it. A chroma sample is centred on the luma samples it covers, so a luma position sits between two chroma centres when the plane is subsampled. */
fn chroma_at(plane: &Plane, (x, y): (usize, usize), layout: Layout) -> f64 {
    let (x0, x1, wx) = chroma_taps(x, layout.sub_x, layout.last.0);
    let (y0, y1, wy) = chroma_taps(y, layout.sub_y, layout.last.1);
    let at = |cx: usize, cy: usize| f64::from(plane.get(cx, cy));
    let top = at(x0, y0) * (1.0 - wx) + at(x1, y0) * wx;
    let bottom = at(x0, y1) * (1.0 - wx) + at(x1, y1) * wx;
    top * (1.0 - wy) + bottom * wy
}

/** The two chroma positions around a luma position along one axis, and the weight of the second. Without subsampling the position is its own sample. */
fn chroma_taps(pos: usize, shift: usize, last: usize) -> (usize, usize, f64) {
    if shift == 0 {
        return (pos, pos, 0.0);
    }
    let centre = (pos as f64 + 0.5) / (1u32 << shift) as f64 - 0.5;
    let low = centre.floor();
    let clamp = |v: f64| (v.max(0.0) as usize).min(last);
    (clamp(low), clamp(low + 1.0), centre - low)
}

/** R, G and B from normalised Y, Cb and Cr with the luma weights of the matrix. */
fn from_weights(y: f64, cb: f64, cr: f64, kr: f64, kb: f64) -> [f64; 3] {
    let kg = 1.0 - kr - kb;
    [
        y + 2.0 * (1.0 - kr) * cr,
        y - 2.0 * kb * (1.0 - kb) / kg * cb - 2.0 * kr * (1.0 - kr) / kg * cr,
        y + 2.0 * (1.0 - kb) * cb,
    ]
}

/** A normalised channel to an 8-bit code, clamped to the range. */
fn to_byte(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/** A sample at `bit_depth` bits scaled to 8 bits, rounded. */
fn scale_to_8(v: i32, bit_depth: u32) -> u8 {
    let max = (1u32 << bit_depth) - 1;
    let code = v.clamp(0, max as i32) as u32;
    ((code * 255 + max / 2) / max) as u8
}

#[cfg(test)]
mod tests;
