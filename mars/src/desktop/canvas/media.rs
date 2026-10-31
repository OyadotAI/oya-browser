/**
 * Media element boxes: the current video frame fitted by `object-fit` and `object-position`
 * (the poster, a background layer, shows until the first frame), the captions of showing text
 * tracks, and native controls (play/pause, elapsed and total time, a scrubber and mute) like
 * Chromium's panel. Clicks on the controls are answered by the coordinator, which uses the same
 * geometry (`control_action`).
 */
use super::{DisplayItem, rect};
use crate::platform;
use objc2_app_kit::{NSAttributedStringNSStringDrawing, NSBezierPath, NSColor};
use objc2_foundation::NSPoint;

/// Height of the video control bar and the audio panel's content row, in points.
pub(crate) const BAR: f64 = 40.0;
/// Width of the play button and the mute button at each end of the bar.
pub(crate) const BUTTON: f64 = 36.0;
/// Width reserved for the time text.
const TIME_TEXT: f64 = 96.0;
/// Font sizes of the time text and of captions.
const TIME_FONT: f64 = 12.0;
const CAPTION_FONT: f64 = 16.0;
/// Captions keep this far above the bottom (or the control bar).
const CAPTION_GAP: f64 = 8.0;
/// Inner padding of a caption's backdrop.
const CAPTION_PAD: f64 = 4.0;
/// Opacity of the control bar and caption backdrops.
const SHADE: f64 = 0.6;
const CAPTION_SHADE: f64 = 0.8;
/// Thickness of the scrubber track.
const TRACK: f64 = 4.0;
/// Seconds per minute and per hour, for the time text.
const MINUTE: u64 = 60;
const HOUR: u64 = 3600;

/** Paint one media element's frame, captions and controls into its box. */
pub(super) fn paint(
    page: &str,
    item: &DisplayItem,
    shape: &super::paint::Shape,
    painter: &super::paint::Painter,
    alpha: f64,
) {
    let Some(view) = super::super::media::view(page, item.local_id) else {
        return;
    };
    if let Some(frame) = &view.frame {
        super::background::paint_replaced_image(item, shape, painter, frame, alpha);
    }
    let [x, y, width, height] =
        item.content_box
            .unwrap_or([item.x, item.y, item.width, item.height]);
    let bar = match (view.controls, view.video) {
        (false, _) => 0.0,
        (true, true) => BAR.min(height),
        (true, false) => height,
    };
    captions(&view.captions, [x, y, width, height - bar]);
    if view.controls {
        controls(&view, [x, y + height - bar, width, bar]);
    }
}

/** Caption lines, centered near the bottom on a dark backdrop, the last line lowest. */
fn captions(lines: &[String], [x, y, width, height]: [f64; 4]) {
    let mut bottom = y + height - CAPTION_GAP;
    for line in lines.iter().rev().flat_map(|text| text.lines().rev()) {
        let text = platform::colored(line, CAPTION_FONT, false, &NSColor::whiteColor());
        let size = text.size();
        let left = x + (width - size.width) / 2.0;
        let top = bottom - size.height;
        NSColor::blackColor()
            .colorWithAlphaComponent(CAPTION_SHADE)
            .setFill();
        NSBezierPath::fillRect(rect(
            left - CAPTION_PAD,
            top,
            size.width + 2.0 * CAPTION_PAD,
            size.height,
        ));
        text.drawAtPoint(NSPoint::new(left, top));
        bottom = top - CAPTION_PAD;
    }
}

/** The control bar: play or pause, time, scrubber, mute. */
fn controls(view: &super::super::media::View, [x, y, width, height]: [f64; 4]) {
    NSColor::blackColor()
        .colorWithAlphaComponent(SHADE)
        .setFill();
    NSBezierPath::fillRect(rect(x, y, width, height));
    let white = NSColor::whiteColor();
    white.setFill();
    let middle = y + height / 2.0;
    if view.paused || view.status.ended {
        triangle(x + BUTTON / 2.0, middle);
    } else {
        bars(x + BUTTON / 2.0, middle);
    }
    let time = format!(
        "{} / {}",
        clock(view.status.time),
        clock(view.status.duration)
    );
    let text = platform::colored(&time, TIME_FONT, false, &white);
    let text_height = text.size().height;
    text.drawAtPoint(NSPoint::new(x + BUTTON, middle - text_height / 2.0));
    let track = [x + BUTTON + TIME_TEXT, x + width - BUTTON];
    if track[1] > track[0] {
        scrubber(track, middle, view.status.time, view.status.duration);
    }
    white.setFill();
    speaker(x + width - BUTTON / 2.0, middle, view.muted);
}

/** A play triangle centered on a point. */
fn triangle(cx: f64, cy: f64) {
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(cx - 5.0, cy - 7.0));
    path.lineToPoint(NSPoint::new(cx + 7.0, cy));
    path.lineToPoint(NSPoint::new(cx - 5.0, cy + 7.0));
    path.closePath();
    path.fill();
}

/** Two pause bars centered on a point. */
fn bars(cx: f64, cy: f64) {
    NSBezierPath::fillRect(rect(cx - 6.0, cy - 7.0, 4.0, 14.0));
    NSBezierPath::fillRect(rect(cx + 2.0, cy - 7.0, 4.0, 14.0));
}

/** A speaker, struck through when muted. */
fn speaker(cx: f64, cy: f64, muted: bool) {
    NSBezierPath::fillRect(rect(cx - 7.0, cy - 3.0, 4.0, 6.0));
    let cone = NSBezierPath::bezierPath();
    cone.moveToPoint(NSPoint::new(cx - 3.0, cy - 3.0));
    cone.lineToPoint(NSPoint::new(cx + 3.0, cy - 8.0));
    cone.lineToPoint(NSPoint::new(cx + 3.0, cy + 8.0));
    cone.lineToPoint(NSPoint::new(cx - 3.0, cy + 3.0));
    cone.closePath();
    cone.fill();
    if muted {
        let strike = NSBezierPath::bezierPath();
        strike.moveToPoint(NSPoint::new(cx - 9.0, cy + 9.0));
        strike.lineToPoint(NSPoint::new(cx + 9.0, cy - 9.0));
        strike.setLineWidth(2.0);
        strike.stroke();
    }
}

/** The scrubber track with the played part filled. */
fn scrubber([left, right]: [f64; 2], cy: f64, time: f64, duration: f64) {
    let width = right - left;
    NSColor::whiteColor()
        .colorWithAlphaComponent(SHADE)
        .setFill();
    NSBezierPath::fillRect(rect(left, cy - TRACK / 2.0, width, TRACK));
    let played = if duration.is_finite() && duration > 0.0 {
        (time / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    NSColor::whiteColor().setFill();
    NSBezierPath::fillRect(rect(left, cy - TRACK / 2.0, width * played, TRACK));
}

/** `m:ss` or `h:mm:ss`, as Chromium's controls print time; `0:00` while unknown. */
fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds as u64
    } else {
        0
    };
    let (hours, minutes, secs) = (total / HOUR, total % HOUR / MINUTE, total % MINUTE);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

/**
 * What a click at document point (`px`, `py`) does to a media element whose box is `item`'s:
 * the play button toggles, the scrubber seeks (returning the fraction), the mute button mutes,
 * and anywhere else on a video toggles playback. None when the click is not on controls.
 */
pub(crate) fn control_action(
    item_box: [f64; 4],
    video: bool,
    px: f64,
    py: f64,
) -> Option<(&'static str, f64)> {
    let [x, y, width, height] = item_box;
    let bar_top = if video {
        y + height - BAR.min(height)
    } else {
        y
    };
    if py < bar_top {
        return video.then_some(("toggle", 0.0));
    }
    if px < x + BUTTON {
        return Some(("toggle", 0.0));
    }
    if px > x + width - BUTTON {
        return Some(("mute", 0.0));
    }
    let (left, right) = (x + BUTTON + TIME_TEXT, x + width - BUTTON);
    if px >= left && right > left {
        return Some(("seek", ((px - left) / (right - left)).clamp(0.0, 1.0)));
    }
    None
}

#[cfg(test)]
mod tests {
    /** Time prints as m:ss, or h:mm:ss past an hour; unknown time prints as 0:00. */
    #[test]
    fn clock_formats_like_chromium_controls() {
        assert_eq!(super::clock(65.4), "1:05");
        assert_eq!(super::clock(3725.0), "1:02:05");
        assert_eq!(super::clock(f64::NAN), "0:00");
    }
    /** The bar's play button toggles, its scrubber seeks by fraction, its right end mutes. */
    #[test]
    fn control_clicks_map_to_actions() {
        let video = [0.0, 0.0, 400.0, 200.0];
        assert_eq!(
            super::control_action(video, true, 10.0, 190.0),
            Some(("toggle", 0.0))
        );
        assert_eq!(
            super::control_action(video, true, 390.0, 190.0),
            Some(("mute", 0.0))
        );
        let left = super::BUTTON + super::TIME_TEXT;
        let (action, fraction) = super::control_action(
            video,
            true,
            left + (400.0 - super::BUTTON - left) / 2.0,
            190.0,
        )
        .unwrap();
        assert_eq!(action, "seek");
        assert!((fraction - 0.5).abs() < 1e-9);
        assert_eq!(
            super::control_action(video, true, 200.0, 50.0),
            Some(("toggle", 0.0))
        );
        assert_eq!(
            super::control_action([0.0, 0.0, 300.0, 54.0], false, 10.0, 2.0),
            Some(("toggle", 0.0))
        );
    }
}
