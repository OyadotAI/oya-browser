/**
 * One native player: AVFoundation demuxes and decodes (VideoToolbox and AudioToolbox
 * underneath), plays audio through the OS with the element's volume, mute and rate, and keeps
 * audio and video on one media clock; video frames are pulled from an output for Mars to paint
 * into the element's box. It lives on the main thread, reads bytes only through its loader, and
 * reports what it observes as plain status in the hub.
 */
use super::clock::Clock;
use super::loader::Loader;
use super::stream_video::{StreamVideo, video_lane};
use crate::runtime::media::constants::KEEP_BEHIND;
use crate::runtime::media::hub::{Shared, Status};
use crate::runtime::media::stream::{Append, Stream};
use crate::v9::MediaSettings;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::NSImage;
use objc2_av_foundation::{
    AVAudioTimePitchAlgorithmTimeDomain, AVAudioTimePitchAlgorithmVarispeed, AVMediaTypeAudio,
    AVPlayer, AVPlayerActionAtItemEnd, AVPlayerItem, AVPlayerItemOutput, AVPlayerItemStatus,
    AVPlayerItemVideoOutput, AVPlayerTimeControlStatus, AVURLAsset, NSValueAVFoundationExtensions,
};
use objc2_core_media::{CMTime, CMTimeFlags};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL, NSValue};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Seek tolerance and media-time precision: Chrome seeks frame-accurately (zero tolerance).
const TIMESCALE: i32 = 600;
/// How close to the duration counts as having reached the end, in seconds.
const END_EPSILON: f64 = 0.05;
/// MediaError codes.
const DECODE_ERROR: u16 = 3;
const UNSUPPORTED: u16 = 4;

/** The AVFoundation objects of a started player. */
struct Engine {
    player: Retained<AVPlayer>,
    item: Retained<AVPlayerItem>,
    asset: Retained<AVURLAsset>,
    output: Retained<AVPlayerItemVideoOutput>,
}

/// One element's player.
pub(super) struct Native {
    pub settings: MediaSettings,
    pub node: usize,
    pub captions: Vec<String>,
    /// The latest frame, as an image the painter draws.
    pub frame: Option<Retained<NSImage>>,
    /// The frame changed since the last paint.
    pub fresh: bool,
    loader: Retained<Loader>,
    engine: Option<Engine>,
    /// Frames pulled from the output so far.
    frames: u64,
    /// Seeks asked for, and the latest one AVFoundation finished (shared with its callback).
    seeks: u64,
    finished: Arc<AtomicU64>,
    seek_time: f64,
    /// A seek waits here until the player exists (preload none).
    pending_seek: Option<(f64, bool)>,
    /// A player for a Media Source stream, which has no fetchable URL.
    stream: bool,
    /// The segments a Media Source stream has been given, per source buffer.
    pub media: Stream,
    /// The playhead of a Media Source stream.
    clock: Clock,
    /// The video decoder of a Media Source stream.
    video: StreamVideo,
}

impl Native {
    /** A player for a freshly loaded element; AVFoundation starts once it may fetch. */
    pub fn new(mtm: MainThreadMarker, node: usize, settings: MediaSettings, stream: bool) -> Self {
        let mut native = Self {
            settings,
            node,
            captions: Vec::new(),
            frame: None,
            fresh: false,
            loader: Loader::new(mtm),
            engine: None,
            frames: 0,
            seeks: 0,
            finished: Arc::new(AtomicU64::new(0)),
            seek_time: 0.0,
            pending_seek: None,
            stream,
            media: Stream::default(),
            clock: Clock::default(),
            video: StreamVideo::default(),
        };
        native.start_if_allowed(mtm);
        native
    }
    /** `preload=none` defers every fetch until the page plays or seeks. */
    pub fn start_if_allowed(&mut self, mtm: MainThreadMarker) {
        let deferred =
            self.settings.preload == "none" && self.settings.paused && self.pending_seek.is_none();
        if self.engine.is_some() || deferred || self.stream {
            return;
        }
        self.engine = Some(start(mtm, &self.loader));
        self.apply();
        if let Some((time, fast)) = self.pending_seek.take() {
            self.seek(time, fast);
        }
    }
    /** A segment the script accepted for one source buffer; a refused one is logged and buffers nothing. */
    pub fn append(&mut self, buffer: u32, append: Append<'_>) {
        if let Err(error) = self.media.append(buffer, append) {
            crate::runtime::media::debug(|| format!("append refused: {error}"));
        }
    }
    /** New settings from the page. */
    pub fn set(&mut self, mtm: MainThreadMarker, settings: MediaSettings) {
        self.settings = settings;
        self.start_if_allowed(mtm);
        self.apply();
    }
    /** Play or pause at the page's rate, volume and pitch handling. */
    fn apply(&self) {
        let Some(engine) = &self.engine else { return };
        let settings = &self.settings;
        // SAFETY: AVPlayer and AVPlayerItem setters, on the main thread that owns them.
        unsafe {
            engine.player.setVolume(settings.volume as f32);
            engine.player.setMuted(settings.muted);
            let pitch = if settings.preserves_pitch {
                AVAudioTimePitchAlgorithmTimeDomain
            } else {
                AVAudioTimePitchAlgorithmVarispeed
            };
            if let Some(pitch) = pitch {
                engine.item.setAudioTimePitchAlgorithm(pitch);
            }
            if settings.paused || settings.rate == 0.0 {
                engine.player.pause();
            } else if engine.player.rate() != settings.rate as f32 {
                engine.player.setRate(settings.rate as f32);
            }
        }
    }
    /** Seek; the hub hears of it once AVFoundation finishes the latest seek. */
    pub fn seek(&mut self, time: f64, fast: bool) {
        if self.stream {
            self.seeks += 1;
            self.seek_time = time;
            self.clock.seek(time, super::host_seconds());
            self.video.reset();
            return;
        }
        let Some(engine) = &self.engine else {
            self.pending_seek = Some((time, fast));
            return;
        };
        self.seeks += 1;
        self.seek_time = time;
        let serial = self.seeks;
        let finished = self.finished.clone();
        let tolerance = if fast {
            seconds(f64::INFINITY)
        } else {
            seconds(0.0)
        };
        let done = block2::RcBlock::new(move |complete: Bool| {
            if complete.as_bool() {
                finished.fetch_max(serial, Ordering::AcqRel);
            }
        });
        // SAFETY: the completion block is retained by AVFoundation until it runs.
        unsafe {
            engine
                .player
                .seekToTime_toleranceBefore_toleranceAfter_completionHandler(
                    seconds(time),
                    tolerance,
                    tolerance,
                    &done,
                );
        }
        self.apply();
    }
    /** One frame tick: serve bytes, observe the player, and pull a new frame when there is one. */
    pub fn tick(&mut self, shared: &mut Shared, host_time: f64) {
        self.loader.serve(shared);
        if self.stream {
            self.tick_stream(shared, host_time);
            return;
        }
        let Some(engine) = &self.engine else {
            return;
        };
        shared.status = observe(
            engine,
            shared,
            self.seeks,
            &self.finished,
            self.seek_time,
            self.frames,
        );
        if !self.settings.video {
            return;
        }
        // SAFETY: the output belongs to this item; pixel buffers are read under their lock.
        let item_time = unsafe { engine.output.itemTimeForHostTime(host_time) };
        if unsafe { engine.output.hasNewPixelBufferForItemTime(item_time) } {
            let buffer = unsafe {
                engine
                    .output
                    .copyPixelBufferForItemTime_itemTimeForDisplay(item_time, std::ptr::null_mut())
            };
            if let Some(image) = buffer.and_then(|buffer| super::frames::image(&buffer)) {
                self.frame = Some(image);
                self.fresh = true;
                self.frames += 1;
            }
        }
    }
}

/** One tick of a Media Source player: move its playhead, evict what is behind it, and publish its state. */
impl Native {
    fn tick_stream(&mut self, shared: &mut Shared, host_time: f64) {
        self.clock.set_running(!self.settings.paused, host_time);
        let playhead = self.clock.time(host_time);
        self.media.evict_behind(playhead);
        self.video.forget_before(playhead - KEEP_BEHIND);
        if let Some(picture) = self.video.step(&self.media, playhead) {
            self.frame = Some(picture);
            self.fresh = true;
            self.frames += 1;
        }
        let size = video_lane(&self.media).map(|lane| (lane.track.width, lane.track.height));
        let (width, height) = size.unwrap_or((0, 0));
        shared.status = Status {
            time: playhead,
            error: shared.status.error.clone(),
            playing: self.clock.running(),
            seeks: self.seeks,
            seek_time: self.seek_time,
            frames: self.frames,
            stored: self.media.buffers(),
            video: width > 0 && height > 0,
            width,
            height,
            ..Status::default()
        };
    }
}

/** Build the asset over the private URL, the item with a BGRA frame output, and the player. */
fn start(mtm: MainThreadMarker, loader: &Loader) -> Engine {
    // SAFETY: AVFoundation construction on the main thread; the loader outlives the asset
    // because the Native owning both drops the engine first.
    unsafe {
        let url = NSURL::URLWithString(&NSString::from_str("mars-media://resource/media"))
            .expect("constant URL parses");
        let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
        asset.resourceLoader().setDelegate_queue(
            Some(ProtocolObject::from_ref(loader)),
            Some(dispatch2::DispatchQueue::main()),
        );
        let item = AVPlayerItem::playerItemWithAsset(&asset, mtm);
        let output = AVPlayerItemVideoOutput::initWithPixelBufferAttributes(
            AVPlayerItemVideoOutput::alloc(),
            Some(&pixel_attributes()),
        );
        item.addOutput(&output as &AVPlayerItemOutput);
        let player = AVPlayer::playerWithPlayerItem(Some(&item), mtm);
        player.setAutomaticallyWaitsToMinimizeStalling(false);
        player.setActionAtItemEnd(AVPlayerActionAtItemEnd::Pause);
        Engine {
            player,
            item,
            asset,
            output,
        }
    }
}

/** Ask for 32-bit BGRA frames, which map straight onto an AppKit bitmap. */
fn pixel_attributes() -> Retained<NSDictionary<NSString, AnyObject>> {
    // SAFETY: CFString and NSString are toll-free bridged; the key is a static constant.
    let key: &NSString = unsafe {
        &*(objc2_core_video::kCVPixelBufferPixelFormatTypeKey
            as *const objc2_core_foundation::CFString as *const NSString)
    };
    let format = NSNumber::new_u32(objc2_core_video::kCVPixelFormatType_32BGRA);
    let value: &AnyObject = &format;
    NSDictionary::from_slices(&[key], &[value])
}

/** What the player shows now. */
fn observe(
    engine: &Engine,
    shared: &Shared,
    seeks: u64,
    finished: &AtomicU64,
    seek_time: f64,
    frames: u64,
) -> Status {
    // SAFETY: AVFoundation getters on the main thread.
    unsafe {
        let item = &engine.item;
        let mut status = Status {
            frames,
            seek_time,
            ..Status::default()
        };
        let state = item.status();
        if state == AVPlayerItemStatus::Failed {
            let code = if shared.status.ready {
                DECODE_ERROR
            } else {
                UNSUPPORTED
            };
            let message = item
                .error()
                .map(|error| error.localizedDescription().to_string())
                .unwrap_or_else(|| "MEDIA_ELEMENT_ERROR: Format error".into());
            let error = shared.bytes.failed.clone().unwrap_or((code, message));
            status.error = Some(error);
            return status;
        }
        if state != AVPlayerItemStatus::ReadyToPlay {
            if let Some(failed) = &shared.bytes.failed {
                status.error = Some(failed.clone());
            }
            return status;
        }
        status.ready = true;
        status.duration = time_seconds(item.duration());
        let size = item.presentationSize();
        status.width = size.width.round() as u32;
        status.height = size.height.round() as u32;
        status.video = status.width > 0 && status.height > 0;
        status.audio = has_audio(&engine.asset);
        status.time = time_seconds(engine.player.currentTime()).max(0.0);
        let control = engine.player.timeControlStatus();
        status.playing = control == AVPlayerTimeControlStatus::Playing;
        status.waiting = control == AVPlayerTimeControlStatus::WaitingToPlayAtSpecifiedRate;
        status.ended = status.duration.is_finite()
            && status.time >= status.duration - END_EPSILON
            && !status.playing;
        status.buffered = ranges(&item.loadedTimeRanges());
        status.likely = item.isPlaybackLikelyToKeepUp();
        status.loaded = loaded(shared);
        let done = finished.load(Ordering::Acquire);
        status.seeks = if done == seeks {
            seeks
        } else {
            shared.status.seeks.min(done)
        };
        status
    }
}

/** Whether the asset has an audio track. The synchronous accessor is safe here: the item is
ready, so its tracks are loaded and the call cannot block. */
#[allow(deprecated)]
unsafe fn has_audio(asset: &AVURLAsset) -> bool {
    // SAFETY: tracks are loaded once the item is ready to play.
    unsafe { AVMediaTypeAudio }
        .is_some_and(|audio| unsafe { asset.tracksWithMediaType(audio) }.count() > 0)
}

/** Whether every byte of the resource is held. */
fn loaded(shared: &Shared) -> bool {
    let Some(length) = shared.bytes.length else {
        return false;
    };
    shared
        .bytes
        .spans()
        .first()
        .is_some_and(|span| span.0 == 0 && span.1 >= length)
}

/** Time ranges as [start, end] seconds. */
fn ranges(values: &objc2_foundation::NSArray<NSValue>) -> Vec<[f64; 2]> {
    values
        .iter()
        .map(|value| {
            // SAFETY: AVFoundation fills loadedTimeRanges with CMTimeRange values.
            let range = unsafe { value.CMTimeRangeValue() };
            let start = time_seconds(range.start);
            [start, start + time_seconds(range.duration)]
        })
        .filter(|[start, end]| start.is_finite() && end.is_finite())
        .collect()
}

/** Seconds of a CMTime; infinite when indefinite or positively infinite. */
fn time_seconds(time: CMTime) -> f64 {
    if time.flags.contains(CMTimeFlags::Indefinite)
        || time.flags.contains(CMTimeFlags::PositiveInfinity)
    {
        return f64::INFINITY;
    }
    let value = unsafe { time.seconds() };
    if value.is_nan() { 0.0 } else { value }
}

/** A CMTime of `value` seconds; infinity stays infinity (an unbounded seek tolerance). */
fn seconds(value: f64) -> CMTime {
    if value.is_infinite() {
        // SAFETY: a static constant.
        return unsafe { objc2_core_media::kCMTimePositiveInfinity };
    }
    unsafe { CMTime::with_seconds(value, TIMESCALE) }
}

impl Drop for Native {
    /** An unloaded element falls silent at once, before AVFoundation releases its objects. */
    fn drop(&mut self) {
        if let Some(engine) = &self.engine {
            // SAFETY: main-thread AVPlayer calls on a player this Native owns.
            unsafe {
                engine.player.pause();
                engine.player.replaceCurrentItemWithPlayerItem(None);
            }
        }
    }
}
