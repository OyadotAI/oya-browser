/** Bounded system image decoding returns inert straight-alpha pixels, never native handles or animation state. */
use mars_core::{
    canvas::{Bitmap, EDGE},
    constants::IMAGE_BYTES,
    protocol::{EngineError, EngineResult},
};
#[cfg(any(test, target_os = "macos"))]
mod gif;
#[cfg(target_os = "macos")]
mod native;
#[cfg(test)]
mod tests;
#[cfg(any(test, target_os = "macos"))]
mod webp;

/** Decode static WebP or an animated GIF's first full-canvas image into admitted straight-alpha pixels. */
pub fn decode_canvas_image(bytes: &[u8], width: u32, height: u32) -> EngineResult<Bitmap> {
    let gif = bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a");
    let webp = bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP");
    if bytes.len() > IMAGE_BYTES
        || (!gif && !webp)
        || width == 0
        || height == 0
        || width > EDGE
        || height > EDGE
    {
        return Err(unsupported());
    }
    decode(bytes, width, height)
}

/** macOS ImageIO owns format-specific decompression within the admitted single-frame request. */
#[cfg(target_os = "macos")]
fn decode(bytes: &[u8], width: u32, height: u32) -> EngineResult<Bitmap> {
    native::decode(bytes, width, height)
}

/** Other targets do not claim native format support or synthesize blank image pixels. */
#[cfg(not(target_os = "macos"))]
fn decode(_: &[u8], _: u32, _: u32) -> EngineResult<Bitmap> {
    Err(unsupported())
}

/** Native failures remain value-free and distinct from successful transparent images. */
fn unsupported() -> EngineError {
    EngineError::unsupported("Native Canvas image format or frame is unsupported.")
}
