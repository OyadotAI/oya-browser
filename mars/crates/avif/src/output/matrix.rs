/** The colour matrix and range of a picture (the AVIF colr box and the AV1 colour config): the luma weights that turn Y, Cb and Cr into RGB, or the identity mapping of RGB coded as 4:4:4. A picture with no matrix named is treated as BT.601, the common default. */
use crate::av1::MC_IDENTITY;

/** The matrix-coefficient codes that name a non-default matrix (ITU-T H.273). */
const MC_BT_709: u8 = 1;
const MC_BT_2020_NCL: u8 = 9;

/** The luma weights of red and blue for a matrix; green takes the rest. */
const BT601: (f64, f64) = (0.299, 0.114);
const BT709: (f64, f64) = (0.2126, 0.0722);
const BT2020: (f64, f64) = (0.2627, 0.0593);

/** How YUV becomes RGB: through a luma-weighted matrix, or as G, B, R planes read straight across. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Matrix {
    Weights { kr: f64, kb: f64 },
    Identity,
}

impl Matrix {
    /** The matrix a matrix-coefficient code names. */
    pub(super) fn from_code(code: u8) -> Self {
        let (kr, kb) = match code {
            MC_IDENTITY => return Self::Identity,
            MC_BT_709 => BT709,
            MC_BT_2020_NCL => BT2020,
            _ => BT601,
        };
        Self::Weights { kr, kb }
    }
}

/** The sample range: limited (video) range keeps headroom and footroom, full range uses every code. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Range {
    pub(super) full: bool,
    pub(super) bit_depth: u32,
}

impl Range {
    /** Normalise a luma sample to 0..=1. */
    pub(super) fn luma(self, y: i32) -> f64 {
        if self.full {
            f64::from(y) / self.max()
        } else {
            let (lo, span) = self.limited_span(16.0, 219.0);
            (f64::from(y) - lo) / span
        }
    }

    /** Normalise a chroma code to -0.5..=0.5 around zero. The code may be fractional after interpolation. */
    pub(super) fn chroma_f(self, c: f64) -> f64 {
        if self.full {
            (c - self.max() / 2.0 - 0.5) / self.max()
        } else {
            let (mid, span) = self.limited_span(128.0, 224.0);
            (c - mid) / span
        }
    }

    /** The largest code at this bit depth. */
    pub(super) fn max(self) -> f64 {
        f64::from((1u32 << self.bit_depth) - 1)
    }

    /** The offset and span of limited range at this bit depth, for a value at 8 bits. */
    fn limited_span(self, offset8: f64, span8: f64) -> (f64, f64) {
        let scale = f64::from(1u32 << (self.bit_depth - 8));
        (offset8 * scale, span8 * scale)
    }
}
