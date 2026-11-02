/** Container admission narrows native GIF/WebP decode to one bounded, fully described image. */
use super::ImageAsset;
use crate::protocol::{EngineError, EngineResult};
use mars_core::canvas::{Bitmap, CHANNELS, EDGE};

/** Animated GIFs expose their first full-canvas frame; WebP animation remains unsupported. AVIF is
decoded by Mars's own decoder, which owns every bound it reads from the file. */
pub(super) fn decode(asset: &ImageAsset) -> EngineResult<Bitmap> {
    if asset.bytes.len() > crate::constants::IMAGE_BYTES {
        return Err(unsupported());
    }
    let dimensions = super::super::dimensions(&asset.mime, &asset.bytes)?;
    let (width, height) = (dimensions.width, dimensions.height);
    if width == 0
        || height == 0
        || width > EDGE
        || height > EDGE
        || u64::from(width) * u64::from(height) * CHANNELS as u64 != asset.decoded
    {
        return Err(unsupported());
    }
    if asset.mime == "image/avif" {
        return super::super::avif::bitmap(&asset.bytes);
    }
    crate::platform::decode_canvas_image(&asset.bytes, width, height)
}

/** Native-source format failures remain value-free and cannot become header-only successful pixels. */
fn unsupported() -> EngineError {
    EngineError::unsupported("Canvas image container or animation frame is unsupported.")
}
