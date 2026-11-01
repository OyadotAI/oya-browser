/** Native canvas text and PNG encoding use short-lived OS resources over bounded inert pixels. */
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font, FontMetrics, PNG_BYTES, TEXT_UNITS};
use mars_core::protocol::{EngineError, EngineResult};

/// Application-injected drawing service; it owns no page state or native handles between calls.
pub struct NativeCanvas;

impl CanvasHost for NativeCanvas {
    /// macOS has an actual font rasterizer; other builds must not claim text support.
    fn available(&self) -> bool {
        cfg!(target_os = "macos")
    }
    /// Ascent and descent of the same face that measurement and painting resolve.
    fn font_metrics(&self, font: &Font) -> EngineResult<FontMetrics> {
        validate(font, "")?;
        let (ascent, descent) = super::font_box(face(font));
        Ok(FontMetrics { ascent, descent })
    }
    /// Both calls resolve through Eye's shared generic font selection.
    fn measure(&self, font: &Font, text: &str) -> EngineResult<f64> {
        validate(font, text)?;
        Ok(advance(text, font))
    }
    /// Rasterize glyphs into transparent pixels; the caller composites those pixels transactionally.
    fn text(
        &self,
        bitmap: &Bitmap,
        font: &Font,
        text: &str,
        at: [f64; 2],
        color: [u8; 4],
    ) -> EngineResult<Bitmap> {
        self.text_transformed(bitmap, font, text, at, color, Affine::IDENTITY)
    }
    /// Apply the drawing transform while rasterizing glyphs, before bitmap clipping.
    fn text_transformed(
        &self,
        bitmap: &Bitmap,
        font: &Font,
        text: &str,
        at: [f64; 2],
        color: [u8; 4],
        transform: Affine,
    ) -> EngineResult<Bitmap> {
        bitmap.validate()?;
        validate(font, text)?;
        if at.iter().any(|v| !v.is_finite()) || !transform.finite() {
            return Err(EngineError::invalid(
                "Invalid canvas text position or transform.",
            ));
        }
        if transform.inverse().is_none() {
            return Bitmap::new(bitmap.width, bitmap.height);
        }
        raster(bitmap, font, text, at, color, transform)
    }
    /// PNG always encodes the actual unpremultiplied pixels, without an environment-specific hash.
    fn png(&self, bitmap: &Bitmap) -> EngineResult<Vec<u8>> {
        bitmap.validate()?;
        let _encoding = mars_core::canvas::Allocation::new(PNG_BYTES)?;
        let mut output = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut output, bitmap.width, bitmap.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .and_then(|mut w| w.write_image_data(&bitmap.pixels))
                .map_err(|_| drawing_error())?;
        }
        if output.len() > PNG_BYTES {
            return Err(EngineError::limit("Canvas PNG bytes", PNG_BYTES));
        }
        Ok(output)
    }
}

/** Host checks are independent of the calling interpreter's validation. */
fn validate(font: &Font, text: &str) -> EngineResult<()> {
    if text.encode_utf16().count() > TEXT_UNITS
        || !font.size.is_finite()
        || font.size <= 0.0
        || font.size > f64::from(mars_core::canvas::EDGE)
        || !["serif", "sans-serif", "monospace"].contains(&font.family.as_str())
    {
        return Err(EngineError::invalid("Invalid bounded canvas text."));
    }
    Ok(())
}
/** Keep font measurement and native drawing on the same platform primitive. */
fn face(font: &Font) -> super::Face {
    let family = [
        ("serif", super::Family::Serif),
        ("sans-serif", super::Family::SansSerif),
        ("monospace", super::Family::Monospace),
    ]
    .into_iter()
    .find(|(name, _)| *name == font.family)
    .map(|(_, family)| family)
    .unwrap_or(super::Family::SansSerif);
    super::Face {
        family,
        weight: font.weight,
        italic: font.italic,
        spacing: font.letter_spacing,
        ..super::Face::system(font.size, false)
    }
}
/** Width of a run as painted: AppKit's letter spacing is inside the face, CSS word spacing is added per separator. */
fn advance(text: &str, font: &Font) -> f64 {
    let separators = text
        .encode_utf16()
        .filter(|unit| *unit == 0x20 || *unit == 0xa0)
        .count();
    super::measure(text, face(font)) + font.word_spacing * separators as f64
}
/** Native allocation/encoding refusal is observable, never represented as successful blank ink. */
fn drawing_error() -> EngineError {
    EngineError::unsupported("Native Canvas drawing failed.")
}

/** AppKit owns the bitmap for this call; pixels are copied out before its autorelease pool closes. */
#[cfg(target_os = "macos")]
fn raster(
    bitmap: &Bitmap,
    font: &Font,
    text: &str,
    at: [f64; 2],
    color: [u8; 4],
    transform: Affine,
) -> EngineResult<Bitmap> {
    use objc2::{AnyThread, rc::autoreleasepool};
    use objc2_app_kit::{
        NSAttributedStringNSExtendedStringDrawing, NSBitmapImageRep, NSColor,
        NSDeviceRGBColorSpace, NSGraphicsContext, NSStringDrawingOptions,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    if !bitmap.drawable() {
        return Bitmap::new(bitmap.width, bitmap.height);
    }
    let _native = mars_core::canvas::Allocation::new(bitmap.pixels.len())?;
    autoreleasepool(|_| {
        // SAFETY: bounded dimensions and explicit stride ask AppKit to own a complete RGBA allocation.
        let rep = unsafe { NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),std::ptr::null_mut(),bitmap.width as isize,bitmap.height as isize,8,4,true,false,NSDeviceRGBColorSpace,(bitmap.width*4) as isize,32)
        }.ok_or_else(drawing_error)?;
        let context =
            NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep).ok_or_else(drawing_error)?;
        let pixels = rep.bitmapData();
        if pixels.is_null() {
            return Err(drawing_error());
        }
        // SAFETY: AppKit allocated the validated length and retains ownership through this scope.
        unsafe {
            std::ptr::write_bytes(pixels, 0, bitmap.pixels.len());
        }
        let native_transform = text_transform(transform, bitmap.height)?;
        NSGraphicsContext::saveGraphicsState_class();
        let saved = GraphicsState;
        NSGraphicsContext::setCurrentContext(Some(&context));
        objc2_core_graphics::CGContext::concat_ctm(Some(&context.CGContext()), native_transform);
        let ink = NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(color[0]) / 255.0,
            f64::from(color[1]) / 255.0,
            f64::from(color[2]) / 255.0,
            f64::from(color[3]) / 255.0,
        );
        let run = super::spaced(text, face(font), &ink, false, font.word_spacing);
        run.drawWithRect_options_context(
            NSRect::new(
                NSPoint::new(at[0], f64::from(bitmap.height) - at[1]),
                NSSize::new(advance(text, font) + font.size, font.size),
            ),
            NSStringDrawingOptions(0),
            None,
        );
        context.flushGraphics();
        drop(saved);
        // SAFETY: the owned rep still contains exactly width*height*4 bytes with the chosen stride.
        let source = unsafe { std::slice::from_raw_parts(pixels, bitmap.pixels.len()) };
        let mut result = Bitmap::new(bitmap.width, bitmap.height)?;
        for (out, pixel) in result
            .pixels
            .make_mut()?
            .chunks_exact_mut(4)
            .zip(source.chunks_exact(4))
        {
            out[3] = pixel[3];
            for channel in 0..3 {
                out[channel] = if pixel[3] == 0 {
                    0
                } else {
                    (u32::from(pixel[channel]) * 255 / u32::from(pixel[3])).min(255) as u8
                };
            }
        }
        Ok(result)
    })
}
/** Other hosts report absence instead of drawing synthetic text bars into fingerprints. */
#[cfg(not(target_os = "macos"))]
fn raster(
    _: &Bitmap,
    _: &Font,
    _: &str,
    _: [f64; 2],
    _: [u8; 4],
    _: Affine,
) -> EngineResult<Bitmap> {
    Err(drawing_error())
}

/** AppKit's bottom-left coordinates conjugate the top-left Canvas transform around the bitmap. */
#[cfg(target_os = "macos")]
fn text_transform(
    transform: Affine,
    height: u32,
) -> EngineResult<objc2_core_foundation::CGAffineTransform> {
    let [a, b, c, d, e, f] = transform.0;
    let height = f64::from(height);
    let [tx, ty] = [c * height + e, (1.0 - d) * height - f];
    if !tx.is_finite() || !ty.is_finite() {
        return Err(drawing_error());
    }
    Ok(objc2_core_foundation::CGAffineTransform {
        a,
        b: -b,
        c: -c,
        d,
        tx,
        ty,
    })
}

/** Every exit, including unwinding, restores the caller's native graphics state. */
#[cfg(target_os = "macos")]
struct GraphicsState;
#[cfg(target_os = "macos")]
impl Drop for GraphicsState {
    fn drop(&mut self) {
        objc2_app_kit::NSGraphicsContext::restoreGraphicsState_class();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /** Glyphs initially outside the bitmap must transform into view before clipping. */
    #[test]
    fn transformed_native_text_rasterizes_before_bitmap_clipping() {
        let bitmap = Bitmap::new(96, 48).unwrap();
        let font = Font {
            size: 18.0,
            family: "sans-serif".into(),
            ..Font::default()
        };
        let expected = NativeCanvas
            .text(&bitmap, &font, "MMMM", [10.0, 24.0], [255; 4])
            .unwrap();
        let actual = NativeCanvas
            .text_transformed(
                &bitmap,
                &font,
                "MMMM",
                [-30.0, 24.0],
                [255; 4],
                Affine([1.0, 0.0, 0.0, 1.0, 40.0, 0.0]),
            )
            .unwrap();
        assert!(expected.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
        assert_eq!(actual, expected);
    }

    /** Canvas positive vertical translation moves ink down despite AppKit's flipped convention. */
    #[test]
    fn transformed_native_text_uses_canvas_coordinates_and_restores_graphics_state() {
        let bitmap = Bitmap::new(96, 48).unwrap();
        let font = Font {
            size: 18.0,
            family: "sans-serif".into(),
            ..Font::default()
        };
        let expected = NativeCanvas
            .text(&bitmap, &font, "Ink", [12.0, 30.0], [255, 0, 0, 128])
            .unwrap();
        let actual = NativeCanvas
            .text_transformed(
                &bitmap,
                &font,
                "Ink",
                [2.0, 24.0],
                [255, 0, 0, 128],
                Affine([1.0, 0.0, 0.0, 1.0, 10.0, 6.0]),
            )
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            NativeCanvas
                .text(&bitmap, &font, "Ink", [12.0, 30.0], [255, 0, 0, 128])
                .unwrap(),
            expected
        );
    }
}
