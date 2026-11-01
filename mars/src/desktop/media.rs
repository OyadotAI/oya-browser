/**
 * The native media players of this window, on the main thread. Each frame tick applies the
 * commands pages sent (through the coordinator's hub), lets every player serve bytes, observe
 * its state and pull a new video frame, and says which element boxes need repainting. The
 * painter reads frames, controls state and captions from here.
 */
mod audio;
mod clock;
mod constants;
mod decoder;
mod frames;
mod loader;
mod player;
mod samples;
mod stream_video;
mod types;

use crate::runtime::media::hub::{self, PlayerKey, Status};
use crate::runtime::media::stream::Append;
use crate::v9::MediaCommand;
use objc2::MainThreadMarker;
use player::Native;
use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    /** Every live player, by page and player number. Main thread only. */
    static PLAYERS: RefCell<BTreeMap<PlayerKey, Native>> = const { RefCell::new(BTreeMap::new()) };
}

/// What the painter needs of one element's player.
pub(super) struct View {
    pub frame: Option<objc2::rc::Retained<objc2_app_kit::NSImage>>,
    pub controls: bool,
    pub video: bool,
    pub paused: bool,
    pub muted: bool,
    pub captions: Vec<String>,
    pub status: Status,
}

/** One frame tick; returns the (page, node) boxes whose frame changed. */
pub(super) fn tick(mtm: MainThreadMarker) -> Vec<(String, usize)> {
    let commands: Vec<(PlayerKey, MediaCommand)> = hub::lock().commands.drain(..).collect();
    PLAYERS.with_borrow_mut(|players| {
        for (key, command) in commands {
            apply(mtm, players, key, command);
        }
        if players.is_empty() {
            return Vec::new();
        }
        let host_time = host_seconds();
        let mut hub = hub::lock();
        let mut changed = Vec::new();
        for (key, native) in players.iter_mut() {
            let Some(shared) = hub.players.get_mut(key) else {
                continue;
            };
            native.tick(shared, host_time);
            if std::mem::take(&mut native.fresh)
                || shared.status.playing && native.settings.controls
            {
                changed.push((key.0.clone(), native.node));
            }
        }
        changed
    })
}

/** Apply one page command to its player. */
fn apply(
    mtm: MainThreadMarker,
    players: &mut BTreeMap<PlayerKey, Native>,
    key: PlayerKey,
    command: MediaCommand,
) {
    if let MediaCommand::Load {
        node,
        settings,
        stream,
        ..
    } = command
    {
        players.insert(key, Native::new(mtm, node, settings, stream));
        return;
    }
    let Some(native) = players.get_mut(&key) else {
        return;
    };
    match command {
        MediaCommand::Settings { settings, .. } => native.set(mtm, settings),
        MediaCommand::Seek { time, fast, .. } => {
            native.seek(time, fast);
            native.start_if_allowed(mtm);
        }
        MediaCommand::Captions { lines, .. } => native.captions = lines,
        MediaCommand::Append {
            buffer,
            bytes,
            init_len,
            offset,
            window_start,
            window_end,
            ..
        } => native.append(
            buffer,
            Append {
                bytes: &bytes,
                init_len,
                offset,
                window: (window_start, window_end),
            },
        ),
        MediaCommand::Remove {
            buffer, start, end, ..
        } => native.media.remove(buffer, start, end),
        MediaCommand::Bind { node, .. } => native.node = node,
        MediaCommand::EndOfStream { .. } => native.end_of_stream(),
        MediaCommand::Unload { .. } => {
            players.remove(&key);
        }
        MediaCommand::Load { .. } => {}
    }
}

/** The painter's view of the element `node` of page `page`, if it has a player. */
pub(super) fn view(page: &str, node: usize) -> Option<View> {
    PLAYERS.with_borrow(|players| {
        let (key, native) = players
            .iter()
            .find(|((id, _), native)| id == page && native.node == node)?;
        let status = hub::lock()
            .players
            .get(key)
            .map(|shared| shared.status.clone())
            .unwrap_or_default();
        Some(View {
            frame: native.frame.clone(),
            controls: native.settings.controls,
            video: native.settings.video,
            paused: native.settings.paused,
            muted: native.settings.muted || native.settings.volume == 0.0,
            captions: native.captions.clone(),
            status,
        })
    })
}

/** The host clock in seconds, the timeline AVFoundation maps item time onto. */
fn host_seconds() -> f64 {
    let ticks = objc2_core_video::CVGetCurrentHostTime() as f64;
    ticks / objc2_core_video::CVGetHostClockFrequency()
}

/** Test support: the players ticked and read from a test thread, since no window runs under cargo test. */
#[cfg(feature = "test-support")]
pub mod testing {
    use objc2::MainThreadMarker;

    /**
     * Tick every player once. The test thread stands in for the main thread: no window exists, and the
     * frameworks a stream player uses (VideoToolbox and the sample buffer renderers) do not require it.
     */
    pub fn tick() {
        // SAFETY: see above; nothing a stream player calls asserts the main thread.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        super::tick(mtm);
    }

    /** The frame the first player of `page` shows, as RGBA rows: (width, height, bytes). */
    pub fn frame(page: &str) -> Option<(usize, usize, Vec<u8>)> {
        super::PLAYERS.with_borrow(|players| {
            players
                .iter()
                .find(|((id, _), _)| id == page)
                .and_then(|(_, native)| native.frame.as_deref())
                .and_then(super::frames::pixels)
        })
    }
}
