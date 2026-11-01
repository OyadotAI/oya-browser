/** AVIF bytes through Mars's own decoder (crate mars-avif): detection by the HEIF brand, the size
from the container, and the straight-alpha bitmap every painter reads. */
use super::invalid;
use crate::protocol::{EngineError, EngineResult};
use mars_core::canvas::Bitmap;

/** Byte offset of the `ftyp` box type; the box size occupies the four bytes before it. */
const FTYP_AT: usize = 4;
/** Byte offset of the major brand, which follows the `ftyp` box type. */
const BRAND_AT: usize = 8;
/** End of the four-byte major brand: `avif` for stills, `avis` for image sequences. */
const BRAND_END: usize = 12;
/** The error code of a file that uses an AVIF feature Mars does not build yet. The runtime asks
the server once more without AVIF offered, so the page still gets its WebP or JPEG. */
pub(crate) const UNSUPPORTED: &str = "image_avif_unsupported";
/** Hint carried with an unsupported AVIF; the retry is what makes the image show. */
const UNSUPPORTED_HINT: &str = "Mars asks the server again without offering AVIF.";

/** Whether the bytes are an AVIF file, judged by the `ftyp` box's major brand. */
pub(crate) fn is_avif(bytes: &[u8]) -> bool {
    bytes.get(FTYP_AT..BRAND_AT) == Some(&b"ftyp"[..])
        && matches!(bytes.get(BRAND_AT..BRAND_END), Some(b"avif" | b"avis"))
}

/** The output size, read from the container alone; the caller applies the page's bounds before any decode. */
pub(crate) fn dimensions(bytes: &[u8]) -> EngineResult<(u32, u32)> {
    mars_avif::dimensions(bytes).map_err(error)
}

/** The full decoded picture as 8-bit straight-alpha RGBA. */
pub(crate) fn decode(bytes: &[u8]) -> EngineResult<mars_avif::Image> {
    mars_avif::decode(bytes).map_err(error)
}

/** The decoded picture in the canvas bitmap form the painters share; the pixel count must match the picture. */
pub(crate) fn bitmap(bytes: &[u8]) -> EngineResult<Bitmap> {
    let image = decode(bytes)?;
    let mut bitmap = Bitmap::new(image.width, image.height)?;
    let pixels = bitmap.pixels.make_mut()?;
    if pixels.len() != image.rgba.len() {
        return Err(invalid());
    }
    pixels.copy_from_slice(&image.rgba);
    Ok(bitmap)
}

/** A refused feature keeps its own code so the caller can retry; broken or oversized files fail
like any other undecodable image, with no detail from the bytes. */
fn error(failure: mars_avif::Error) -> EngineError {
    match failure {
        mars_avif::Error::Unsupported(_) => EngineError::new(
            UNSUPPORTED,
            "This AVIF uses a feature Mars does not decode yet.",
            UNSUPPORTED_HINT,
        ),
        _ => invalid(),
    }
}
