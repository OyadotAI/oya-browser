/*!
 * The sound of a Media Source player: the first audio lane's AAC frames are handed to an AVSampleBufferAudioRenderer
 * a little ahead of the playhead, and the renderer plays them on the player's clock through the OS at the element's
 * volume and mute. The format description is built from the track's AudioSpecificConfig, wrapped as the MPEG-4
 * elementary stream descriptor the OS decoder reads its configuration from (what FFmpeg and Chromium hand it too).
 * Only AAC-LC is opened; another object type leaves the stream silent rather than open a renderer it would fail in.
 */
use super::clock::Clock;
use super::constants::{
    AAC_FRAMES_PER_PACKET, AAC_LC, AOT_SHIFT, DECODER_CONFIG_RATES, DESC_DECODER_CONFIG,
    DESC_DECODER_SPECIFIC, DESC_ES, DESC_SL_CONFIG, FORMAT_AAC, LENGTH_CONTINUES,
    LENGTH_GROUP_BITS, LENGTH_GROUP_MASK, LENGTH_GROUPS, LOOKAHEAD, NO_ERROR,
    OBJECT_TYPE_MPEG4_AUDIO, SL_MP4, STREAM_TYPE_AUDIO,
};
use super::samples::{Sample, release};
use crate::runtime::media::stream::{Frame, Lane, Stream};
use crate::v9::MediaSettings;
use mars_mp4::TrackKind;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_av_foundation::{
    AVQueuedSampleBufferRendering, AVQueuedSampleBufferRenderingStatus, AVSampleBufferAudioRenderer,
};
use objc2_core_media::CMSampleBuffer;
use std::collections::VecDeque;
use std::ffi::c_void;

/** CoreAudio's stream description; only the fields a compressed format fills are nonzero. */
#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    fn CMAudioFormatDescriptionCreate(
        allocator: *const c_void,
        description: *const AudioStreamBasicDescription,
        layout_size: usize,
        layout: *const c_void,
        cookie_size: usize,
        cookie: *const c_void,
        extensions: *const c_void,
        format: *mut *mut c_void,
    ) -> i32;
}

/** Why no format description was made: the object type is not AAC-LC, or CoreMedia refused with a status. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioError {
    /** The AudioSpecificConfig names an object type other than AAC-LC. */
    Unsupported,
    /** CoreMedia's status for a description it would not create. */
    Refused(i32),
}

/** An AAC format description CoreMedia owns, released with its holder. */
pub struct AudioFormat(*mut c_void);

impl AudioFormat {
    /** A description for an AAC-LC track from its AudioSpecificConfig, channel count and sample rate. */
    pub fn aac(config: &[u8], channels: u16, sample_rate: u32) -> Result<Self, AudioError> {
        if config.first().map(|first| first >> AOT_SHIFT) != Some(AAC_LC) {
            return Err(AudioError::Unsupported);
        }
        let description = AudioStreamBasicDescription {
            sample_rate: f64::from(sample_rate),
            format_id: FORMAT_AAC,
            format_flags: u32::from(AAC_LC),
            bytes_per_packet: 0,
            frames_per_packet: AAC_FRAMES_PER_PACKET,
            bytes_per_frame: 0,
            channels_per_frame: u32::from(channels),
            bits_per_channel: 0,
            reserved: 0,
        };
        let cookie = esds_cookie(config);
        let mut format = std::ptr::null_mut();
        // SAFETY: the description and cookie are live for the call; CoreMedia copies both.
        let status = unsafe {
            CMAudioFormatDescriptionCreate(
                std::ptr::null(),
                &description,
                0,
                std::ptr::null(),
                cookie.len(),
                cookie.as_ptr().cast(),
                std::ptr::null(),
                &mut format,
            )
        };
        if status == NO_ERROR {
            Ok(Self(format))
        } else {
            Err(AudioError::Refused(status))
        }
    }
}

impl Drop for AudioFormat {
    /** The description was created for this holder and nothing else keeps it. */
    fn drop(&mut self) {
        // SAFETY: created by `aac` and released once here.
        unsafe { release(self.0) };
    }
}

/** A stream's audio renderer, the configuration its format was built from, and how far it has been fed. */
#[derive(Default)]
pub struct StreamAudio {
    renderer: Option<Retained<AVSampleBufferAudioRenderer>>,
    format: Option<AudioFormat>,
    /** The track configuration the format was built from; a different one means the frames were replaced. */
    config: Option<Vec<u8>>,
    /** The decode time of the last frame handed to the renderer; None until a run starts at the playhead. */
    fed: Option<f64>,
    /** The element's volume and mute, applied to the renderer when it opens and whenever they change. */
    volume: f32,
    muted: bool,
}

impl StreamAudio {
    /** Whether a format is open, so the player can report that its audio is ready. */
    pub fn ready(&self) -> bool {
        self.format.is_some()
    }

    /** The renderer's failure, if it reported one, for the page's decode error. */
    pub fn error(&self) -> Option<String> {
        let renderer = self.renderer.as_ref()?;
        // SAFETY: getters on the renderer this stream holds.
        unsafe {
            (renderer.status() == AVQueuedSampleBufferRenderingStatus::Failed).then(|| {
                renderer
                    .error()
                    .map(|error| error.localizedDescription().to_string())
                    .unwrap_or_else(|| "audio renderer failed".into())
            })
        }
    }

    /** Drop what the renderer holds and start the next feed at the playhead, as a seek needs. */
    pub fn flush(&mut self) {
        if let Some(renderer) = &self.renderer {
            // SAFETY: a call on the renderer this stream holds.
            unsafe { renderer.flush() };
        }
        self.fed = None;
    }

    /** The element's volume and mute, applied now if the renderer is open and at open otherwise. */
    pub fn apply(&mut self, settings: &MediaSettings) {
        self.volume = settings.volume as f32;
        self.muted = settings.muted;
        if let Some(renderer) = &self.renderer {
            // SAFETY: setters on the renderer this stream holds.
            unsafe {
                renderer.setVolume(self.volume);
                renderer.setMuted(self.muted);
            }
        }
    }

    /** One step: open the format once the track is known, open the renderer on `clock`, and feed it ahead of `playhead`. */
    pub fn step(&mut self, stream: &Stream, playhead: f64, clock: &Clock) {
        let Some(lane) = audio_lane(stream) else {
            return;
        };
        if self.config.as_deref() != Some(lane.track.config.as_slice()) {
            self.flush();
            self.format = AudioFormat::aac(
                &lane.track.config,
                lane.track.channels,
                lane.track.sample_rate,
            )
            .ok();
            self.config = Some(lane.track.config.clone());
        }
        let Some(format) = &self.format else { return };
        let renderer = self
            .renderer
            .get_or_insert_with(|| open(clock, self.volume, self.muted));
        feed(renderer, format, &lane.frames, &mut self.fed, playhead);
    }
}

/** A renderer at the element's volume and mute, attached to the clock. */
fn open(clock: &Clock, volume: f32, muted: bool) -> Retained<AVSampleBufferAudioRenderer> {
    // SAFETY: construction and setters on a fresh renderer.
    let renderer = unsafe { AVSampleBufferAudioRenderer::new() };
    unsafe {
        renderer.setVolume(volume);
        renderer.setMuted(muted);
    }
    clock.attach(ProtocolObject::from_ref(&*renderer));
    renderer
}

/** Hand the renderer every frame after the last fed one up to the lookahead, while it takes more; a fresh run starts at the frame under the playhead. */
fn feed(
    renderer: &AVSampleBufferAudioRenderer,
    format: &AudioFormat,
    frames: &VecDeque<Frame>,
    fed: &mut Option<f64>,
    playhead: f64,
) {
    for frame in frames.range(first_to_feed(frames, *fed, playhead)..) {
        // SAFETY: a getter on a live renderer.
        if frame.decode > playhead + LOOKAHEAD || !unsafe { renderer.isReadyForMoreMediaData() } {
            break;
        }
        if let Ok(sample) = Sample::new(format.0, frame) {
            // SAFETY: the sample is live for the call; the renderer retains it.
            unsafe { renderer.enqueueSampleBuffer(&*(sample.buffer() as *const CMSampleBuffer)) };
            *fed = Some(frame.decode);
        }
    }
}

/** Where feeding continues: after the last fed frame, or at the frame that covers the playhead on a fresh run. */
fn first_to_feed(frames: &VecDeque<Frame>, fed: Option<f64>, playhead: f64) -> usize {
    match fed {
        Some(last) => frames.partition_point(|frame| frame.decode <= last),
        None => frames.partition_point(|frame| frame.pts + frame.duration <= playhead),
    }
}

/** The first audio lane of a stream. */
fn audio_lane(stream: &Stream) -> Option<&Lane> {
    stream.lanes().find(|lane| lane.kind() == TrackKind::Audio)
}

/** The magic cookie the OS decoder reads: an ES descriptor holding a DecoderConfig with the AudioSpecificConfig and an SLConfig. */
fn esds_cookie(config: &[u8]) -> Vec<u8> {
    let mut decoder = vec![OBJECT_TYPE_MPEG4_AUDIO, STREAM_TYPE_AUDIO];
    decoder.extend(std::iter::repeat_n(0, DECODER_CONFIG_RATES));
    decoder.extend(descriptor(DESC_DECODER_SPECIFIC, config));
    let mut es = vec![0, 0, 0];
    es.extend(descriptor(DESC_DECODER_CONFIG, &decoder));
    es.extend(descriptor(DESC_SL_CONFIG, &[SL_MP4]));
    descriptor(DESC_ES, &es)
}

/** A tagged descriptor: the tag, the body's length in four continued 7-bit groups, then the body. */
fn descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    let length = body.len() as u32;
    let mut out = vec![tag];
    for group in (0..LENGTH_GROUPS).rev() {
        let bits = (length >> (group * LENGTH_GROUP_BITS) & LENGTH_GROUP_MASK) as u8;
        out.push(if group == 0 {
            bits
        } else {
            bits | LENGTH_CONTINUES
        });
    }
    out.extend_from_slice(body);
    out
}

#[cfg(test)]
mod tests {
    /**
     * The renderer against the fixture tests/fixtures/tiny-aac.mp4: one second of a 440 Hz sine at 48 kHz, mono,
     * made with `ffmpeg -f lavfi -i sine=frequency=440:sample_rate=48000 -t 1 -c:a aac
     * -movflags frag_keyframe+empty_moov+default_base_moof`.
     */
    use super::{AudioError, AudioFormat, Clock, StreamAudio, esds_cookie, first_to_feed};
    use crate::runtime::media::stream::{Append, Stream};

    const FIXTURE: &[u8] = include_bytes!("../../../tests/fixtures/tiny-aac.mp4");

    /** The fixture appended whole into a stream. */
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

    /** The fixture's track opens a format and a renderer that takes every frame of the first second without failing. */
    #[test]
    fn the_fixture_track_opens_a_renderer_that_takes_its_frames() {
        let stream = fixture_stream();
        let clock = Clock::new();
        let mut audio = StreamAudio::default();
        audio.step(&stream, 0.0, &clock);
        assert!(audio.ready());
        assert_eq!(audio.error(), None);
        let lane = stream.lanes().next().unwrap();
        let fed = audio.fed.expect("frames were fed");
        assert!(fed > 0.5, "{fed}");
        assert!(lane.frames.iter().any(|frame| frame.decode == fed));
    }

    /** A configuration of another object type is refused, so no renderer is opened for it. */
    #[test]
    fn another_object_type_is_refused() {
        let he_aac = [0x2b, 0x92, 0x08, 0x00];
        assert!(matches!(
            AudioFormat::aac(&he_aac, 2, 44_100),
            Err(AudioError::Unsupported)
        ));
    }

    /** The cookie is an ES descriptor around a DecoderConfig that carries the config, then an SLConfig. */
    #[test]
    fn the_cookie_wraps_the_config_in_an_elementary_stream_descriptor() {
        let cookie = esds_cookie(&[0x11, 0x90]);
        assert_eq!(cookie.len(), 39);
        assert_eq!(
            &cookie[..15],
            &[
                0x03, 0x80, 0x80, 0x80, 0x22, 0, 0, 0, 0x04, 0x80, 0x80, 0x80, 0x14, 0x40, 0x15
            ]
        );
        assert_eq!(
            &cookie[26..],
            &[
                0x05, 0x80, 0x80, 0x80, 0x02, 0x11, 0x90, 0x06, 0x80, 0x80, 0x80, 0x01, 0x02
            ]
        );
    }

    /** A fresh run starts at the frame under the playhead, and a continued run after the last fed frame. */
    #[test]
    fn feeding_starts_under_the_playhead_and_continues_after_the_last_fed_frame() {
        let stream = fixture_stream();
        let frames = &stream.lanes().next().unwrap().frames;
        let fresh = first_to_feed(frames, None, 0.5);
        assert!(fresh > 0);
        assert!(frames[fresh].pts + frames[fresh].duration > 0.5);
        assert!(frames[fresh - 1].pts + frames[fresh - 1].duration <= 0.5);
        let continued = first_to_feed(frames, Some(frames[3].decode), 0.5);
        assert_eq!(continued, 4);
    }
}
