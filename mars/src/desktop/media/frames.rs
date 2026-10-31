/** Decoded video frames as AppKit images: a BGRA pixel buffer copied row by row into a bitmap the painter draws like any replaced image. */
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{NSBitmapFormat, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetHeight, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::NSSize;

/// Bytes per BGRA pixel.
const PIXEL: usize = 4;
/// Bits per channel and per pixel of the bitmap.
const CHANNEL_BITS: isize = 8;
const PIXEL_BITS: isize = 32;

/** Copy one BGRA frame into an image; None for an empty or unreadable buffer. */
pub(super) fn image(buffer: &CVPixelBuffer) -> Option<Retained<NSImage>> {
    let (width, height) = (CVPixelBufferGetWidth(buffer), CVPixelBufferGetHeight(buffer));
    if width == 0 || height == 0 {
        return None;
    }
    // SAFETY: the base address is valid while locked; the bitmap owns its own copy afterwards.
    unsafe {
        if CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) != 0 {
            return None;
        }
        let rep = bitmap(width, height);
        let copied = rep.as_ref().and_then(|rep| copy(buffer, rep, width, height));
        CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly);
        copied?;
        let rep = rep?;
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(width as f64, height as f64));
        image.addRepresentation(&rep);
        Some(image)
    }
}

/** An empty bitmap in plain RGBA order. AppKit read the frames' own BGRA layout, declared as
little-endian alpha-first, as big-endian ARGB and painted blue skies purple, so the copy below
reorders each pixel instead. */
fn bitmap(width: usize, height: usize) -> Option<Retained<NSBitmapImageRep>> {
    // SAFETY: AppKit allocates the planes for the given dimensions.
    unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width as isize,
            height as isize,
            CHANNEL_BITS,
            PIXEL as isize,
            true,
            false,
            NSDeviceRGBColorSpace,
            NSBitmapFormat::empty(),
            (width * PIXEL) as isize,
            PIXEL_BITS,
        )
    }
}

/** Copy every row; the source stride may be wider than the image. */
unsafe fn copy(buffer: &CVPixelBuffer, rep: &NSBitmapImageRep, width: usize, height: usize) -> Option<()> {
    let source = CVPixelBufferGetBaseAddress(buffer) as *const u8;
    let stride = CVPixelBufferGetBytesPerRow(buffer);
    let target = rep.bitmapData();
    if source.is_null() || target.is_null() || stride < width * PIXEL {
        return None;
    }
    let row = width * PIXEL;
    for y in 0..height {
        // SAFETY: both rows lie within their locked/owned allocations.
        let (from, to) = unsafe {
            (
                std::slice::from_raw_parts(source.add(y * stride), row),
                std::slice::from_raw_parts_mut(target.add(y * row), row),
            )
        };
        for (out, pixel) in to.chunks_exact_mut(PIXEL).zip(from.chunks_exact(PIXEL)) {
            // BGRA in, RGBA out.
            out.copy_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Some(())
}
