/**
 * CoreMedia sample buffers for the stream presenters: one coded frame's bytes copied into a block buffer and wrapped
 * with its timing and its track's format description, the shape VideoToolbox decodes and the audio renderer plays.
 * CoreMedia's C functions are linked here directly, as the decoder does with VideoToolbox: the format descriptions
 * are held as raw CoreFoundation pointers, and these are the calls that take them.
 */
use super::constants::{NO_ERROR, TIME_SCALE};
use crate::runtime::media::stream::Frame;
use objc2_core_media::CMTime;
use std::ffi::c_void;

/** One sample's timing for CoreMedia: duration, presentation and decode times. */
#[repr(C)]
struct SampleTiming {
    duration: CMTime,
    presentation: CMTime,
    decode: CMTime,
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    fn CMBlockBufferCreateWithMemoryBlock(
        allocator: *const c_void,
        memory: *mut c_void,
        capacity: usize,
        block_allocator: *const c_void,
        custom: *const c_void,
        offset: usize,
        length: usize,
        flags: u32,
        block: *mut *mut c_void,
    ) -> i32;
    fn CMBlockBufferReplaceDataBytes(
        source: *const u8,
        block: *mut c_void,
        offset: usize,
        length: usize,
    ) -> i32;
    fn CMSampleBufferCreateReady(
        allocator: *const c_void,
        block: *mut c_void,
        format: *mut c_void,
        samples: isize,
        timings: isize,
        timing: *const SampleTiming,
        sizes: isize,
        size: *const usize,
        sample: *mut *mut c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *const c_void);
}

/** Release a CoreFoundation object this module's callers created and nothing else holds. */
pub(super) unsafe fn release(object: *const c_void) {
    // SAFETY: the caller promises the object is live and this is its last reference.
    unsafe { CFRelease(object) }
}

/** One frame as a CMSampleBuffer over a block that holds a copy of its bytes; both are released when it drops. */
pub struct Sample {
    block: *mut c_void,
    buffer: *mut c_void,
}

impl Sample {
    /** A sample of `frame` described by `format`; refused with CoreMedia's status when a buffer cannot be made. */
    pub fn new(format: *mut c_void, frame: &Frame) -> Result<Self, i32> {
        let block = block_with(&frame.data)?;
        match sample_for(block, format, frame) {
            Ok(buffer) => Ok(Self { block, buffer }),
            Err(status) => {
                // SAFETY: the block was created above and is not used after this release.
                unsafe { CFRelease(block) };
                Err(status)
            }
        }
    }

    /** The CMSampleBuffer, live until this sample drops. */
    pub fn buffer(&self) -> *mut c_void {
        self.buffer
    }
}

impl Drop for Sample {
    /** The buffer and its block were created for this sample and are released exactly once here. */
    fn drop(&mut self) {
        // SAFETY: both pointers were created by `new` and are not used after this.
        unsafe {
            CFRelease(self.buffer);
            CFRelease(self.block);
        }
    }
}

/** A block buffer holding a copy of the sample's bytes. */
fn block_with(data: &[u8]) -> Result<*mut c_void, i32> {
    let mut block = std::ptr::null_mut();
    // SAFETY: a null memory argument asks CoreMedia to allocate `len` bytes it owns.
    let status = unsafe {
        CMBlockBufferCreateWithMemoryBlock(
            std::ptr::null(),
            std::ptr::null_mut(),
            data.len(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            data.len(),
            0,
            &mut block,
        )
    };
    if status != NO_ERROR {
        return Err(status);
    }
    // SAFETY: the block holds `data.len()` bytes and the source slice is that long.
    let copied = unsafe { CMBlockBufferReplaceDataBytes(data.as_ptr(), block, 0, data.len()) };
    if copied == NO_ERROR {
        Ok(block)
    } else {
        // SAFETY: the block was created above and is not used after this release.
        unsafe { CFRelease(block) };
        Err(copied)
    }
}

/** A sample buffer over the block, with the frame's timing and a single size. */
fn sample_for(block: *mut c_void, format: *mut c_void, frame: &Frame) -> Result<*mut c_void, i32> {
    let timing = SampleTiming {
        // SAFETY: CMTime construction from plain numbers has no preconditions.
        duration: unsafe { CMTime::with_seconds(frame.duration, TIME_SCALE) },
        presentation: unsafe { CMTime::with_seconds(frame.pts, TIME_SCALE) },
        decode: unsafe { CMTime::with_seconds(frame.decode, TIME_SCALE) },
    };
    let size = frame.data.len();
    let mut sample = std::ptr::null_mut();
    // SAFETY: the block, format and timing and size arrays are live for the call; CoreMedia retains what it keeps.
    let status = unsafe {
        CMSampleBufferCreateReady(
            std::ptr::null(),
            block,
            format,
            1,
            1,
            &timing,
            1,
            &size,
            &mut sample,
        )
    };
    if status == NO_ERROR {
        Ok(sample)
    } else {
        Err(status)
    }
}
