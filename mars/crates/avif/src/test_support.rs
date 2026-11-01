/** Test support shared by the decoder's unit tests: the Airbnb fixture decoded to its frame state, the JPEG of the same picture as luma, and the PSNR between them. Compiled only for tests. */
use crate::container;
use crate::frame::{FrameHeader, Parsed};
use crate::recon::Plane;
use crate::sequence::Sequence;
use crate::stream;
use crate::tile::{self, FrameState};

/** The Airbnb CDN AVIF: 240x180, 8-bit 4:2:0. */
/** Official AVIF vectors, decoded by the CDEF tests: 8-bit 4:2:0 with CDEF on, and 4:4:4 with CDEF on. */
pub(crate) const CHIMERA: &[u8] = include_bytes!("../tests/fixtures/chimera-8bit.avif");
pub(crate) const MEXICO: &[u8] = include_bytes!("../tests/fixtures/mexico-yuv444.avif");
pub(crate) const AIRBNB: &[u8] = include_bytes!("../tests/fixtures/airbnb-240.avif");

/** The JPEG of the Airbnb picture, which the AVIF fixture was made from. It is decoded only here, to check the reconstruction against it. */
pub(crate) const AIRBNB_JPEG: &[u8] = include_bytes!("../tests/fixtures/airbnb-240.jpg");

/** Peak signal-to-noise ratio of two equal-length 8-bit planes, in decibels. */
pub(crate) fn psnr(got: &[f64], want: &[f64]) -> f64 {
    let mse = got
        .iter()
        .zip(want)
        .map(|(g, w)| (g - w).powi(2))
        .sum::<f64>()
        / got.len() as f64;
    10.0 * (255.0f64 * 255.0 / mse).log10()
}

/** The luma of the JPEG as full-range Y (JFIF weights), one value per picture sample in raster order, with the picture's width and height. */
pub(crate) fn jpeg_luma() -> (Vec<f64>, usize, usize) {
    let mut decoder = jpeg_decoder::Decoder::new(AIRBNB_JPEG);
    let rgb = decoder.decode().expect("the Airbnb JPEG decodes");
    let info = decoder.info().expect("the JPEG has a header");
    let luma = rgb
        .chunks_exact(3)
        .map(|p| 0.299 * f64::from(p[0]) + 0.587 * f64::from(p[1]) + 0.114 * f64::from(p[2]))
        .collect();
    (luma, usize::from(info.width), usize::from(info.height))
}

/** A fixture decoded to its frame state, with the sequence and frame headers that describe it. */
pub(crate) struct Decoded {
    pub(crate) seq: Sequence,
    pub(crate) hdr: FrameHeader,
    pub(crate) fs: FrameState,
}

/** Decode every tile of a fixture's only frame, keeping the headers. Panics on any error, because a fixture that does not decode is a broken test. */
pub(crate) fn decode_fixture(bytes: &[u8]) -> Decoded {
    let picture = container::open(bytes).expect("the container opens");
    let payload = picture
        .payload(picture.primary)
        .expect("the primary item reads");
    let headers = stream::parse_headers(&payload, &picture.av1c).expect("the headers parse");
    let Parsed::Frame(frame) = headers.frame else {
        panic!("the fixture has a show-existing frame");
    };
    let hdr = *frame;
    let groups = stream::tile_groups(&payload, &hdr).expect("the tile groups split");
    let fs = tile::decode_frame(&headers.seq, &hdr, &groups).expect("the tiles decode");
    Decoded {
        seq: headers.seq,
        hdr,
        fs,
    }
}

/** The visible luma of a plane as f64 samples in raster order, for a frame of the given size. */
pub(crate) fn picture_luma(plane: &Plane, hdr: &FrameHeader) -> Vec<f64> {
    let (width, height) = (
        hdr.size.frame_width as usize,
        hdr.size.frame_height as usize,
    );
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| f64::from(plane.get(x, y)))
        .collect()
}
