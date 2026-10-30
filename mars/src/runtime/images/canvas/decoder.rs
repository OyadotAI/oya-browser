/** PNG/JPEG and bounded native GIF/WebP decoders supply actual straight-alpha RGBA canvas image pixels. */
use super::*;
use mars_core::canvas::{Allocation, CHANNELS, EDGE};
use std::io::Cursor;

/** Decoder scratch and orientation copies share the same process allocation accounting as canvases. */
const SCRATCH_COPIES: usize = 2;

/** No new decoder or native handle crosses the service boundary. */
pub(super) fn decode(asset: &ImageAsset) -> EngineResult<Bitmap> {
    let bytes = usize::try_from(asset.decoded).map_err(|_| unsupported())?;
    if bytes > EDGE as usize * EDGE as usize * CHANNELS {
        return Err(unsupported());
    }
    if matches!(asset.mime.as_str(), "image/gif" | "image/webp") {
        return super::formats::decode(asset);
    }
    let _scratch = Allocation::new(bytes.saturating_mul(SCRATCH_COPIES))?;
    match asset.mime.as_str() {
        "image/png" => png(&asset.bytes, bytes),
        "image/jpeg" => jpeg(&asset.bytes, bytes),
        _ => Err(unsupported()),
    }
}

/** Bounds are checked from the decoder header before allocating its output. */
fn admitted(width: u32, height: u32, expected: usize) -> EngineResult<Bitmap> {
    if width == 0
        || height == 0
        || width > EDGE
        || height > EDGE
        || width as usize * height as usize * CHANNELS != expected
    {
        return Err(unsupported());
    }
    Bitmap::new(width, height)
}

/** Palette/transparency expansion and 16-bit stripping produce the existing eight-bit pixel format. */
fn png(bytes: &[u8], expected: usize) -> EngineResult<Bitmap> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| unsupported())?;
    let mut bitmap = admitted(reader.info().width, reader.info().height, expected)?;
    if reader.output_buffer_size() > bitmap.pixels.len() {
        return Err(unsupported());
    }
    let mut bytes = vec![0; reader.output_buffer_size()];
    let frame = reader.next_frame(&mut bytes).map_err(|_| unsupported())?;
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => CHANNELS,
        _ => return Err(unsupported()),
    };
    expand(&bytes[..frame.buffer_size()], channels, &mut bitmap)?;
    Ok(bitmap)
}

/** JPEG orientation follows the same EXIF mapping as the ordinary image painter. */
fn jpeg(bytes: &[u8], expected: usize) -> EngineResult<Bitmap> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.read_info().map_err(|_| unsupported())?;
    let info = decoder.info().ok_or_else(unsupported)?;
    let mut bitmap = admitted(u32::from(info.width), u32::from(info.height), expected)?;
    let channels = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        _ => return Err(unsupported()),
    };
    let decoded = decoder.decode().map_err(|_| unsupported())?;
    expand(&decoded, channels, &mut bitmap)?;
    let orientation = crate::runtime::exif::orientation(bytes);
    let (upright, width, height) =
        crate::runtime::exif::upright(&bitmap.pixels, bitmap.width, bitmap.height, orientation);
    bitmap.width = width;
    bitmap.height = height;
    bitmap.pixels.make_mut()?.copy_from_slice(&upright);
    Ok(bitmap)
}

/** Decode channel layouts explicitly; malformed output never reaches the sampling loop. */
fn expand(bytes: &[u8], channels: usize, bitmap: &mut Bitmap) -> EngineResult<()> {
    if bytes.len() != bitmap.pixels.len() / CHANNELS * channels {
        return Err(unsupported());
    }
    for (input, output) in bytes
        .chunks_exact(channels)
        .zip(bitmap.pixels.make_mut()?.chunks_exact_mut(CHANNELS))
    {
        let rgba = match channels {
            1 => [input[0], input[0], input[0], u8::MAX],
            2 => [input[0], input[0], input[0], input[1]],
            3 => [input[0], input[1], input[2], u8::MAX],
            _ => [input[0], input[1], input[2], input[3]],
        };
        output.copy_from_slice(&rgba);
    }
    Ok(())
}

/** A loaded image may still be outside this bounded canvas-source implementation. */
fn unsupported() -> EngineError {
    EngineError::unsupported("Decoded Canvas image format or dimensions are unsupported.")
}
