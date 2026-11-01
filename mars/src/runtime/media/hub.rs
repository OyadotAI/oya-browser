/**
 * Where the coordinator and the native players meet. Players live on the main thread (the OS
 * frameworks that decode and play require it); the coordinator runs on its worker. They share
 * only plain data through this hub: commands in page order, each player's fetched bytes, the
 * byte it needs next and the status it last observed. No OS object crosses threads.
 */
use super::bytes::Bytes;
use crate::v9::MediaCommand;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{LazyLock, Mutex, MutexGuard};

/// A player's identity: the page (`page-N`, unique per navigation) and its page-local number.
pub type PlayerKey = (String, u64);

/// What the native player last observed, written on the main thread each frame tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// The resource opened and its tracks and duration are known.
    pub ready: bool,
    /// A media error code (2 network, 3 decode, 4 unsupported) and message.
    pub error: Option<(u16, String)>,
    /// Seconds; infinite for a stream with no end.
    pub duration: f64,
    /// Natural video size in pixels; zero for audio.
    pub width: u32,
    pub height: u32,
    pub audio: bool,
    pub video: bool,
    /// The current media time in seconds.
    pub time: f64,
    /// Time is advancing (not paused, not stalled).
    pub playing: bool,
    /// Asked to play but waiting for data.
    pub waiting: bool,
    /// Playback reached the end.
    pub ended: bool,
    /// Seconds of media buffered, as [start, end] ranges.
    pub buffered: Vec<[f64; 2]>,
    /// Seeks completed since load, and where the last one landed.
    pub seeks: u64,
    pub seek_time: f64,
    /// Frames presented so far, for getVideoPlaybackQuality and requestVideoFrameCallback.
    pub frames: u64,
    /// The player predicts playback will not stall.
    pub likely: bool,
    /// Every byte of the resource is fetched.
    pub loaded: bool,
}

/// One player's shared record.
#[derive(Debug, Default)]
pub struct Shared {
    /// Where the bytes come from; empty for a Blob or a Media Source.
    pub url: String,
    /// The page that asked, for Referer and Sec-Fetch-Site.
    pub initiator: String,
    /// An `<audio>` element (Sec-Fetch-Dest: audio) rather than `<video>`.
    pub audio: bool,
    /// The element's node, where its frames and controls paint.
    pub node: usize,
    /// Fetched bytes, filled by the coordinator, read by the native loader.
    pub bytes: Bytes,
    /// The first byte the native loader is waiting for.
    pub need: Option<u64>,
    /// Status observed by the native player.
    pub status: Status,
    /// What the coordinator last told the page, so it sends only changes.
    pub reported: super::updates::Reported,
}

/// The hub: commands waiting for the main thread, and every live player's record.
#[derive(Debug, Default)]
pub struct Hub {
    pub commands: VecDeque<(PlayerKey, MediaCommand)>,
    pub players: BTreeMap<PlayerKey, Shared>,
}

/** The one hub of this process. */
static HUB: LazyLock<Mutex<Hub>> = LazyLock::new(|| Mutex::new(Hub::default()));

/** Lock the hub; a poisoned lock still holds consistent plain data, so it is recovered. */
pub fn lock() -> MutexGuard<'static, Hub> {
    HUB.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
