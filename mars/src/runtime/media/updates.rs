/**
 * What changed in a player since the page last heard, as the update records the page's media
 * script turns into events: metadata, progress, time (four times a second while playing, as
 * HTML's timeupdate cadence asks), waiting and playing, seek completion, the end and errors.
 */
use super::hub::Status;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// How often a playing element hears its time: HTML says every 15 to 250 ms; Chrome uses 250.
pub const TIME_EVERY: Duration = Duration::from_millis(250);
/// How often buffered ranges are re-sent while they grow; each send costs a script step.
pub const PROGRESS_EVERY: Duration = Duration::from_millis(500);

/// What the page was last told about one player.
#[derive(Debug, Default)]
pub struct Reported {
    metadata: bool,
    size: (u32, u32),
    duration: Option<f64>,
    buffered: Vec<[f64; 2]>,
    loaded: bool,
    likely: bool,
    progress_at: Option<Instant>,
    playing: bool,
    waiting: bool,
    seeks: u64,
    ended: bool,
    error: bool,
    time_at: Option<Instant>,
    frames: u64,
}

/** The updates that bring the page up to `status`, recording what was sent. */
pub fn diff(player: u64, status: &Status, sent: &mut Reported, now: Instant) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some((code, message)) = status.error.as_ref().filter(|_| !sent.error) {
        sent.error = true;
        out.push(json!({"player":player,"kind":"error","code":code,"message":message}));
        return out;
    }
    if !status.ready || sent.error {
        return out;
    }
    metadata(player, status, sent, &mut out);
    progress(player, status, sent, now, &mut out);
    seeks(player, status, sent, &mut out);
    playback(player, status, sent, now, &mut out);
    out
}

/** The first metadata, and later size or duration changes. */
fn metadata(player: u64, status: &Status, sent: &mut Reported, out: &mut Vec<Value>) {
    let size = (status.width, status.height);
    let duration = status.duration;
    if sent.metadata && sent.size == size && sent.duration == Some(duration) {
        return;
    }
    let first = !sent.metadata;
    sent.metadata = true;
    sent.size = size;
    sent.duration = Some(duration);
    let duration = duration.is_finite().then_some(duration);
    let mut update = json!({"player":player,"kind":"metadata","duration":duration,
        "width":size.0,"height":size.1,"audio":status.audio,"video":status.video,"enough":status.likely});
    if !first {
        update["enough"] = Value::Bool(sent.likely);
    }
    out.push(update);
}

/** Buffered ranges and whether the whole resource or enough of it is here. */
fn progress(player: u64, status: &Status, sent: &mut Reported, now: Instant, out: &mut Vec<Value>) {
    let changed = sent.buffered != status.buffered
        || sent.loaded != status.loaded
        || sent.likely != status.likely;
    let due = sent.progress_at.is_none_or(|at| now >= at + PROGRESS_EVERY)
        || status.loaded != sent.loaded;
    if !changed || !due {
        return;
    }
    sent.buffered.clone_from(&status.buffered);
    sent.loaded = status.loaded;
    sent.likely = status.likely;
    sent.progress_at = Some(now);
    out.push(
        json!({"player":player,"kind":"progress","buffered":status.buffered,
        "idle":status.loaded,"enough":status.likely}),
    );
}

/** A completed seek lands at its target. */
fn seeks(player: u64, status: &Status, sent: &mut Reported, out: &mut Vec<Value>) {
    if status.seeks > sent.seeks {
        sent.seeks = status.seeks;
        sent.ended = false;
        out.push(json!({"player":player,"kind":"seeked","time":status.seek_time}));
    }
}

/** Stalls and resumes, the time while playing, frame counts and the end. */
fn playback(player: u64, status: &Status, sent: &mut Reported, now: Instant, out: &mut Vec<Value>) {
    if status.waiting != sent.waiting {
        sent.waiting = status.waiting;
        let kind = if status.waiting { "waiting" } else { "playing" };
        out.push(json!({"player":player,"kind":kind}));
    }
    let due = sent.time_at.is_none_or(|at| now >= at + TIME_EVERY);
    if status.playing != sent.playing || status.playing && due {
        sent.playing = status.playing;
        sent.time_at = Some(now);
        out.push(
            json!({"player":player,"kind":"time","time":status.time,"playing":status.playing}),
        );
        if status.frames != sent.frames {
            sent.frames = status.frames;
            out.push(
                json!({"player":player,"kind":"frames","frames":status.frames,"time":status.time}),
            );
        }
    }
    if status.ended && !sent.ended {
        sent.ended = true;
        sent.playing = false;
        out.push(json!({"player":player,"kind":"ended","time":status.time}));
    } else if !status.ended {
        sent.ended = false;
    }
}
