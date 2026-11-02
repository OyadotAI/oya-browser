/** Minimal system framework ABI declarations keep ImageIO an OS facility rather than a new decoder dependency. */
use std::ffi::c_void;
pub(super) type Ref = *const c_void;
/** CGRect's C ABI is two pairs of CGFloat on supported 64-bit macOS targets. */
#[repr(C)]
pub(super) struct Rect {
    pub origin: Point,
    pub size: Size,
}
/** Native user-space point coordinates. */
#[repr(C)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}
/** Native user-space dimensions. */
#[repr(C)]
pub(super) struct Size {
    pub width: f64,
    pub height: f64,
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub(super) fn CFDataCreate(allocator: Ref, bytes: *const u8, length: isize) -> Ref;
    pub(super) fn CFDictionaryCreate(
        allocator: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        keys_callbacks: Ref,
        values_callbacks: Ref,
    ) -> Ref;
    pub(super) fn CFDictionaryGetValue(dictionary: Ref, key: Ref) -> Ref;
    pub(super) fn CFGetTypeID(value: Ref) -> usize;
    pub(super) fn CFNumberGetTypeID() -> usize;
    pub(super) fn CFNumberGetValue(number: Ref, kind: isize, value: *mut c_void) -> bool;
    pub(super) fn CFRelease(value: Ref);
    pub(super) static kCFBooleanTrue: Ref;
    pub(super) static kCFBooleanFalse: Ref;
    pub(super) static kCFTypeDictionaryKeyCallBacks: c_void;
    pub(super) static kCFTypeDictionaryValueCallBacks: c_void;
}

#[link(name = "ImageIO", kind = "framework")]
unsafe extern "C" {
    pub(super) fn CGImageSourceCreateWithData(data: Ref, options: Ref) -> Ref;
    pub(super) fn CGImageSourceGetCount(source: Ref) -> usize;
    pub(super) fn CGImageSourceCopyPropertiesAtIndex(
        source: Ref,
        index: usize,
        options: Ref,
    ) -> Ref;
    pub(super) fn CGImageSourceCreateImageAtIndex(source: Ref, index: usize, options: Ref) -> Ref;
    pub(super) fn CGImageSourceGetStatusAtIndex(source: Ref, index: usize) -> i32;
    pub(super) static kCGImagePropertyPixelWidth: Ref;
    pub(super) static kCGImagePropertyPixelHeight: Ref;
    pub(super) static kCGImageSourceShouldCache: Ref;
    pub(super) static kCGImageSourceShouldCacheImmediately: Ref;
    pub(super) static kCGImageSourceShouldAllowFloat: Ref;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub(super) fn CGImageGetWidth(image: Ref) -> usize;
    pub(super) fn CGImageGetHeight(image: Ref) -> usize;
    pub(super) fn CGColorSpaceCreateWithName(name: Ref) -> Ref;
    pub(super) fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        stride: usize,
        color: Ref,
        flags: u32,
    ) -> Ref;
    pub(super) fn CGContextDrawImage(context: Ref, rect: Rect, image: Ref);
    pub(super) fn CGContextFlush(context: Ref);
    pub(super) static kCGColorSpaceSRGB: Ref;
}
