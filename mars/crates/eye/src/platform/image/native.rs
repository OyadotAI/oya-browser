/** ImageIO and CoreGraphics decode one bounded raster into an explicitly described sRGB RGBA buffer. */
use super::{Bitmap, EngineResult, unsupported};
use mars_core::canvas::{Allocation, CHANNELS};
use std::{
    ffi::c_void,
    ptr::{NonNull, null},
};
mod ffi;
use ffi::*;

/** Decoder cache, color conversion, and draw scratch are admitted before native allocation. */
const DECODE_SURFACES: usize = 4;
/** Native number extraction uses a signed 64-bit destination. */
const SIGNED_64: isize = 4;
/** Explicit RGBA byte order and premultiplied trailing alpha, independent of host endianness. */
const RGBA_PREMULTIPLIED: u32 = (4 << 12) | 1;
/** ImageIO's complete status distinguishes a finished indexed image from truncated content. */
const COMPLETE: i32 = 0;
/** Bitmap channels use the canonical eight-bit canvas format. */
const COMPONENT_BITS: usize = 8;

/** Metadata and decoded dimensions must agree with the caller before any image buffer is published. */
pub(super) fn decode(bytes: &[u8], width: u32, height: u32) -> EngineResult<Bitmap> {
    // The public facade validates every container before invoking a decoder, even for direct Rust callers.
    let _encoded = Allocation::new(bytes.len() + 1)?;
    let isolated;
    let bytes = if bytes.starts_with(b"GIF") {
        isolated = super::gif::first_frame(bytes, width, height)?;
        isolated.as_slice()
    } else {
        super::webp::validate(bytes, width, height)?;
        bytes
    };
    let size = width as usize * height as usize * CHANNELS;
    let _scratch = Allocation::new(size * DECODE_SURFACES + bytes.len())?;
    // SAFETY: bounded input lives through the call; every Create/Copy result is immediately owned.
    unsafe {
        let data = Owned::new(CFDataCreate(null(), bytes.as_ptr(), bytes.len() as isize))?;
        let options = options()?;
        let source = Owned::new(CGImageSourceCreateWithData(data.ptr(), options.ptr()))?;
        if CGImageSourceGetCount(source.ptr()) != 1 {
            return Err(unsupported());
        }
        let properties = Owned::new(CGImageSourceCopyPropertiesAtIndex(
            source.ptr(),
            0,
            options.ptr(),
        ))?;
        if dimension(properties.ptr(), kCGImagePropertyPixelWidth)? != width
            || dimension(properties.ptr(), kCGImagePropertyPixelHeight)? != height
        {
            return Err(unsupported());
        }
        let image = Owned::new(CGImageSourceCreateImageAtIndex(
            source.ptr(),
            0,
            options.ptr(),
        ))?;
        if CGImageGetWidth(image.ptr()) != width as usize
            || CGImageGetHeight(image.ptr()) != height as usize
            || CGImageSourceGetStatusAtIndex(source.ptr(), 0) != COMPLETE
        {
            return Err(unsupported());
        }
        raster(image.ptr(), width, height)
    }
}

/** Decode immediately so corrupt compressed data fails before returning a lazy image; only one frame is cached. */
unsafe fn options() -> EngineResult<Owned> {
    // SAFETY: static CF keys/booleans live forever; standard callbacks retain them in the dictionary.
    unsafe {
        let keys = [
            kCGImageSourceShouldCache,
            kCGImageSourceShouldCacheImmediately,
            kCGImageSourceShouldAllowFloat,
        ];
        let values = [kCFBooleanTrue, kCFBooleanTrue, kCFBooleanFalse];
        Owned::new(CFDictionaryCreate(
            null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len() as isize,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        ))
    }
}

/** CF type and integer checks prevent malformed metadata from being cast into dimensions. */
unsafe fn dimension(properties: Ref, key: Ref) -> EngineResult<u32> {
    // SAFETY: properties is an owned ImageIO dictionary; a numeric value is type-checked before extraction.
    unsafe {
        let value = CFDictionaryGetValue(properties, key);
        let mut number = 0i64;
        if value.is_null()
            || CFGetTypeID(value) != CFNumberGetTypeID()
            || !CFNumberGetValue(value, SIGNED_64, (&mut number as *mut i64).cast())
        {
            return Err(unsupported());
        }
        u32::try_from(number).map_err(|_| unsupported())
    }
}

/** Draw into one admitted top-left scanline buffer and unpremultiply only after native ownership ends. */
unsafe fn raster(image: Ref, width: u32, height: u32) -> EngineResult<Bitmap> {
    let mut bitmap = Bitmap::new(width, height)?;
    // SAFETY: the mutable output pointer stays valid and uniquely owned until the context is released.
    unsafe {
        let color = Owned::new(CGColorSpaceCreateWithName(kCGColorSpaceSRGB))?;
        let context = Owned::new(CGBitmapContextCreate(
            bitmap.pixels.make_mut()?.as_mut_ptr().cast(),
            width as usize,
            height as usize,
            COMPONENT_BITS,
            width as usize * CHANNELS,
            color.ptr(),
            RGBA_PREMULTIPLIED,
        ))?;
        CGContextDrawImage(
            context.ptr(),
            Rect {
                origin: Point { x: 0.0, y: 0.0 },
                size: Size {
                    width: f64::from(width),
                    height: f64::from(height),
                },
            },
            image,
        );
        CGContextFlush(context.ptr());
        drop(context);
    }
    for pixel in bitmap.pixels.make_mut()?.chunks_exact_mut(CHANNELS) {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = if alpha == 0 {
                0
            } else {
                ((u32::from(*channel) * u32::from(u8::MAX) + alpha / 2) / alpha)
                    .min(u32::from(u8::MAX)) as u8
            };
        }
    }
    Ok(bitmap)
}

/** All native Create/Copy objects share CoreFoundation ownership and are released on every exit. */
struct Owned(NonNull<c_void>);
impl Owned {
    /** A null native result is a decode failure, never an empty successful image. */
    fn new(pointer: Ref) -> EngineResult<Self> {
        NonNull::new(pointer.cast_mut())
            .map(Self)
            .ok_or_else(unsupported)
    }
    /** Borrow an opaque native object only while its owning guard lives. */
    fn ptr(&self) -> Ref {
        self.0.as_ptr().cast_const()
    }
}
impl Drop for Owned {
    /** CFRelease also releases CGImage, CGContext, and CGColorSpace create-rule objects. */
    fn drop(&mut self) {
        unsafe { CFRelease(self.ptr()) }
    }
}
