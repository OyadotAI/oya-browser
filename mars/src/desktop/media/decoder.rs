/**
 * Hardware H.264 decoding for Media Source streams: VideoToolbox decodes each sample synchronously, and each decoded
 * picture is copied into a frame the painter draws, the same path AVPlayer frames take, so the agent's raster and
 * screenshots see the video. The format description comes from the avcC parameter sets, and each sample's bytes
 * are already length-prefixed NAL units, which is the layout VideoToolbox takes. VideoToolbox is linked here
 * directly: the bindings have no crate for it. The sample buffers it decodes are built in samples.rs.
 */
use super::constants::NO_ERROR;
use super::frames;
use super::samples::{Sample, release};
use crate::runtime::media::avc::AvcConfig;
use crate::runtime::media::stream::Frame;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSImage;
use objc2_core_media::CMTime;
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{NSDictionary, NSNumber, NSString};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;

/// VTDecodeFrameFlags: none set, so each decode completes before the call returns.
const SYNCHRONOUS: u32 = 0;
/// The pixel format frames.rs expects: 32-bit BGRA.
const BGRA: u32 = 0x4247_5241;

/** One decoded picture: its presentation time in seconds and the image the painter draws. */
struct Decoded {
    pts: f64,
    image: Retained<NSImage>,
}

/** VideoToolbox's output record: the callback and the context pointer it is given back. */
#[repr(C)]
struct OutputRecord {
    callback: extern "C" fn(*mut c_void, *mut c_void, i32, u32, *mut c_void, CMTime, CMTime),
    refcon: *mut c_void,
}

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    fn VTDecompressionSessionCreate(
        allocator: *const c_void,
        format: *const c_void,
        specification: *const c_void,
        attributes: *const c_void,
        output: *const OutputRecord,
        session: *mut *mut c_void,
    ) -> i32;
    fn VTDecompressionSessionDecodeFrame(
        session: *mut c_void,
        sample: *mut c_void,
        flags: u32,
        source: *mut c_void,
        info: *mut u32,
    ) -> i32;
    fn VTDecompressionSessionInvalidate(session: *mut c_void);
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    fn CMVideoFormatDescriptionCreateFromH264ParameterSets(
        allocator: *const c_void,
        count: usize,
        pointers: *const *const u8,
        sizes: *const usize,
        length_size: i32,
        format: *mut *mut c_void,
    ) -> i32;
}

/** A decoding session for one H.264 track: its format description, its VideoToolbox session, and the pictures it has produced. */
pub struct Decoder {
    format: *mut c_void,
    session: *mut c_void,
    /** Boxed so the address VideoToolbox holds stays put when the decoder moves. */
    output: Box<RefCell<VecDeque<Decoded>>>,
}

impl Decoder {
    /** A session for a track whose parameter sets are in `config`; refused when VideoToolbox will not take them. */
    pub fn new(config: &AvcConfig) -> Result<Self, i32> {
        let format = format_description(config)?;
        let output = Box::new(RefCell::new(VecDeque::new()));
        let record = OutputRecord {
            callback: deliver,
            refcon: (&*output as *const RefCell<VecDeque<Decoded>>) as *mut c_void,
        };
        let attributes = pixel_attributes();
        let mut session = std::ptr::null_mut();
        // SAFETY: every pointer names a live object for the call; VideoToolbox copies what it keeps.
        let status = unsafe {
            VTDecompressionSessionCreate(
                std::ptr::null(),
                format,
                std::ptr::null(),
                (&*attributes as *const NSDictionary<NSString, AnyObject>).cast(),
                &record,
                &mut session,
            )
        };
        if status != NO_ERROR {
            // SAFETY: the format was created above and is not used again.
            unsafe { release(format) };
            return Err(status);
        }
        Ok(Self {
            format,
            session,
            output,
        })
    }

    /** Decode one sample. Its bytes are copied into a block the sample buffer owns for the call. */
    pub fn decode(&mut self, frame: &Frame) -> Result<(), i32> {
        let sample = Sample::new(self.format, frame)?;
        let mut info = 0;
        // SAFETY: the session and sample are live; no source pointer is needed for a synchronous decode.
        let status = unsafe {
            VTDecompressionSessionDecodeFrame(
                self.session,
                sample.buffer(),
                SYNCHRONOUS,
                std::ptr::null_mut(),
                &mut info,
            )
        };
        if status == NO_ERROR {
            Ok(())
        } else {
            Err(status)
        }
    }

    /**
     * The newest decoded picture at or before `time`; pictures older than it are dropped, and so are pictures more than
     * `horizon` seconds past it, which only a broken timeline produces. None when nothing new is due.
     */
    pub fn picture_at(&mut self, time: f64, horizon: f64) -> Option<Retained<NSImage>> {
        let mut queue = self.output.borrow_mut();
        let mut newest = None;
        while queue.front().is_some_and(|picture| picture.pts <= time) {
            newest = queue.pop_front();
        }
        queue.retain(|picture| picture.pts <= time + horizon);
        newest.map(|picture| picture.image)
    }

    /** Drop every picture waiting to be shown. The player drops the whole decoder on a seek or a replaced track (`StreamVideo::reset`); tests drain the queue in place to go on feeding. */
    #[cfg(test)]
    pub fn clear(&self) {
        self.output.borrow_mut().clear();
    }

    /** Drop the pictures before `cut`, which eviction has removed the frames of. */
    pub fn forget_before(&self, cut: f64) {
        self.output
            .borrow_mut()
            .retain(|picture| picture.pts >= cut);
    }

    /** The pictures waiting to be shown. */
    pub fn pending(&self) -> usize {
        self.output.borrow().len()
    }
}

impl Drop for Decoder {
    /** Invalidate the session before releasing it, so no callback can run into freed memory. */
    fn drop(&mut self) {
        // SAFETY: the session and format are the ones this decoder created and nothing else holds.
        unsafe {
            VTDecompressionSessionInvalidate(self.session);
            release(self.session);
            release(self.format);
        }
    }
}

/** VideoToolbox's output callback: copy the picture into an image now, since the buffer is only valid here. */
extern "C" fn deliver(
    refcon: *mut c_void,
    _source: *mut c_void,
    status: i32,
    _info: u32,
    image: *mut c_void,
    pts: CMTime,
    _duration: CMTime,
) {
    if status != NO_ERROR || image.is_null() || refcon.is_null() {
        return;
    }
    // SAFETY: refcon is the RefCell the decoder boxed, alive until the session is invalidated; the decode that
    // calls back runs on this thread, so no other borrow is open.
    let output = unsafe { &*(refcon as *const RefCell<VecDeque<Decoded>>) };
    // SAFETY: VideoToolbox passes a live image buffer for the duration of the callback.
    let buffer = unsafe { &*(image as *const CVPixelBuffer) };
    // SAFETY: a CMTime from VideoToolbox is valid to read as seconds.
    let pts = unsafe { pts.seconds() };
    if !pts.is_finite() {
        return;
    }
    if let Some(picture) = frames::image(buffer) {
        output.borrow_mut().push_back(Decoded {
            pts,
            image: picture,
        });
    }
}

/** The CMVideoFormatDescription built from the parameter sets, which VideoToolbox needs to open a session. */
fn format_description(config: &AvcConfig) -> Result<*mut c_void, i32> {
    let sets: Vec<&Vec<u8>> = config.sps.iter().chain(config.pps.iter()).collect();
    let pointers: Vec<*const u8> = sets.iter().map(|set| set.as_ptr()).collect();
    let sizes: Vec<usize> = sets.iter().map(|set| set.len()).collect();
    let mut format = std::ptr::null_mut();
    // SAFETY: the pointer and size arrays describe live slices of the same length for the call.
    let status = unsafe {
        CMVideoFormatDescriptionCreateFromH264ParameterSets(
            std::ptr::null(),
            sets.len(),
            pointers.as_ptr(),
            sizes.as_ptr(),
            config.nal_length_size as i32,
            &mut format,
        )
    };
    if status == NO_ERROR {
        Ok(format)
    } else {
        Err(status)
    }
}

/** The destination attributes asking VideoToolbox for BGRA pictures. */
fn pixel_attributes() -> Retained<NSDictionary<NSString, AnyObject>> {
    let key = NSString::from_str("PixelFormatType");
    let format = NSNumber::new_u32(BGRA);
    let value: &AnyObject = &format;
    NSDictionary::from_slices(&[&*key], &[value])
}

#[cfg(test)]
mod tests {
    /**
     * Decoding the fixture through VideoToolbox: the track's parameter sets open a session, every sample decodes, and
     * pictures come back at the stream's size. The fixture is tests/fixtures/tiny-h264.mp4, made with
     * `ffmpeg -f lavfi -i testsrc=size=64x48:rate=10 -t 1 -c:v libx264 -profile:v baseline -pix_fmt yuv420p -g 5
     * -movflags frag_keyframe+empty_moov+default_base_moof`.
     */
    use super::Decoder;
    use crate::runtime::media::avc::parse_avcc;
    use crate::runtime::media::stream::{Append, Stream};

    const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/tiny-h264.mp4");

    /** The fixture's first track, appended whole into a stream, so the test decodes what the stream holds. */
    fn fixture_stream() -> Stream {
        let boxes = mars_mp4::complete_boxes(FIXTURE).unwrap();
        let init_len = boxes
            .iter()
            .find(|span| &span.kind == b"moof")
            .unwrap()
            .start;
        let mut stream = Stream::default();
        stream
            .append(
                1,
                Append {
                    bytes: FIXTURE,
                    init_len,
                    offset: 0.0,
                    window: (0.0, None),
                },
            )
            .unwrap();
        stream
    }

    /** Samples of the fixture decode to pictures: the session opens from the avcC and every frame comes back. */
    #[test]
    fn fixture_samples_decode_to_pictures() {
        let stream = fixture_stream();
        let lane = stream
            .lanes()
            .find(|lane| lane.track.codec.starts_with("avc1"))
            .unwrap();
        let config = parse_avcc(&lane.track.config).unwrap();
        let mut decoder = Decoder::new(&config).unwrap();
        for frame in &lane.frames {
            decoder.decode(frame).unwrap();
        }
        assert_eq!(decoder.pending(), lane.frames.len());
    }

    /** A picture is shown once its time arrives, and the latest due one is the one returned. */
    #[test]
    fn pictures_are_shown_at_their_presentation_time() {
        let stream = fixture_stream();
        let lane = stream
            .lanes()
            .find(|lane| lane.track.codec.starts_with("avc1"))
            .unwrap();
        let mut decoder = Decoder::new(&parse_avcc(&lane.track.config).unwrap()).unwrap();
        for frame in &lane.frames {
            decoder.decode(frame).unwrap();
        }
        assert!(decoder.picture_at(-1.0, 1.0).is_none());
        assert!(decoder.picture_at(0.05, 1.0).is_some());
        assert!(decoder.pending() < lane.frames.len());
    }

    /** Clearing drops every picture waiting to be shown, so a seek or a replaced track never shows stale frames. */
    #[test]
    fn clearing_drops_every_pending_picture() {
        let stream = fixture_stream();
        let lane = stream
            .lanes()
            .find(|lane| lane.track.codec.starts_with("avc1"))
            .unwrap();
        let mut decoder = Decoder::new(&parse_avcc(&lane.track.config).unwrap()).unwrap();
        for frame in &lane.frames {
            decoder.decode(frame).unwrap();
        }
        assert!(decoder.pending() > 0);
        decoder.clear();
        assert_eq!(decoder.pending(), 0);
    }
}
