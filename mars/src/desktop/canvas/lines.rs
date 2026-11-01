/** Text decoration lines: an underline or line-through in the shape, thickness and offset its text item carries. */
use crate::document::{DecorationStyle, DisplayItem, PaintLine};
use objc2_app_kit::{NSBezierPath, NSColor, NSLineCapStyle};
use objc2_foundation::NSPoint;

/// Underlines sit this far below the baseline, as Chromium draws them at small sizes.
const UNDERLINE_OFFSET: f64 = 1.5;
/// Line-throughs sit this fraction of the ascender above the baseline.
const STRIKE_HEIGHT: f64 = 0.35;
/// The font size per point of default thickness: a sixteenth of the font.
const FONT_PER_THICKNESS: f64 = 16.0;
/// The default thickness never drops below a point or rises above two and a half.
const THICKNESS_RANGE: (f64, f64) = (1.0, 2.5);
/// Double lines are two bands of one thickness, one thickness apart.
const DOUBLE_SPACING: f64 = 2.0;
/// Dashes and their gaps, in thicknesses.
const DASH_PATTERN: [f64; 2] = [3.0, 2.0];
/// A wave's half period and crest height, in thicknesses.
const WAVE_HALF_PERIOD: f64 = 2.0;
const WAVE_CREST: f64 = 1.0;
/// Where a cubic's handle sits along the way to its quadratic control point: two thirds makes the two curves equal.
const HANDLE: f64 = 2.0 / 3.0;

/**
 * Paints the underline and line-through of one run. Each uses the item's decoration shape,
 * thickness and offset, falling back to the font-derived solid line when none is set.
 */
pub(super) fn decorate(
    item: &DisplayItem,
    line: &PaintLine,
    color: &NSColor,
    (baseline, ascender): (f64, f64),
) {
    let thick = thickness(item);
    let shape = item
        .decoration
        .map(|decoration| decoration.style)
        .unwrap_or_default();
    let (start, end) = (line.x, line.x + line.width);
    if item.underline {
        let shift = item
            .decoration
            .and_then(|decoration| decoration.offset)
            .unwrap_or(0.0);
        let y = baseline + UNDERLINE_OFFSET + shift;
        stroke(shape, (start, end), y, thick, color);
    }
    if item.strikethrough {
        stroke(
            shape,
            (start, end),
            baseline - ascender * STRIKE_HEIGHT,
            thick,
            color,
        );
    }
}

/** The line's width in points: the authored thickness, else one derived from the font size. */
fn thickness(item: &DisplayItem) -> f64 {
    item.decoration
        .and_then(|decoration| decoration.thickness)
        .unwrap_or_else(|| {
            (item.font_size / FONT_PER_THICKNESS).clamp(THICKNESS_RANGE.0, THICKNESS_RANGE.1)
        })
}

/** Strokes one decoration line of `shape` from `(start, end)` at height `y`. */
fn stroke(shape: DecorationStyle, (start, end): (f64, f64), y: f64, thick: f64, color: &NSColor) {
    match shape {
        DecorationStyle::Solid => band(start, end, y, thick, color, &[], NSLineCapStyle::Butt),
        DecorationStyle::Double => {
            band(start, end, y, thick, color, &[], NSLineCapStyle::Butt);
            let second = y + thick * DOUBLE_SPACING;
            band(start, end, second, thick, color, &[], NSLineCapStyle::Butt);
        }
        DecorationStyle::Dotted => band(
            start,
            end,
            y,
            thick,
            color,
            &[0.0, thick * 2.0],
            NSLineCapStyle::Round,
        ),
        DecorationStyle::Dashed => {
            let dashes = DASH_PATTERN.map(|part| part * thick);
            band(start, end, y, thick, color, &dashes, NSLineCapStyle::Butt);
        }
        DecorationStyle::Wavy => wave(start, end, y, thick, color),
    }
}

/**
 * One straight band. A non-empty `dashes` pattern sets AppKit's dash, and a round cap turns a
 * zero-length dash into a dot (Blink's dotted decoration).
 */
fn band(
    start: f64,
    end: f64,
    y: f64,
    thick: f64,
    color: &NSColor,
    dashes: &[f64],
    cap: NSLineCapStyle,
) {
    color.setStroke();
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(start, y));
    path.lineToPoint(NSPoint::new(end, y));
    path.setLineWidth(thick);
    path.setLineCapStyle(cap);
    if !dashes.is_empty() {
        // SAFETY: the pattern outlives the call, and AppKit copies it into the path.
        unsafe { path.setLineDash_count_phase(dashes.as_ptr(), dashes.len() as isize, 0.0) };
    }
    path.stroke();
}

/**
 * A wave of half periods from `start` to `end`. Each half is a cubic whose control polygon peaks
 * at twice the crest, so the curve's apex lands one crest above or below the line. The last half
 * is shortened, with its crest scaled to match, so the wave ends exactly at `end`.
 */
fn wave(start: f64, end: f64, y: f64, thick: f64, color: &NSColor) {
    color.setStroke();
    let half = thick * WAVE_HALF_PERIOD;
    let crest = thick * WAVE_CREST;
    let path = NSBezierPath::bezierPath();
    path.moveToPoint(NSPoint::new(start, y));
    let (mut x, mut sign) = (start, -1.0);
    while x < end {
        let next = (x + half).min(end);
        let scale = (next - x) / half;
        let (mid, control_y) = ((x + next) / 2.0, y + sign * 2.0 * crest * scale);
        // Cubic handles at two thirds of the way to the quadratic's control point.
        let c1 = (x + (mid - x) * HANDLE, y + (control_y - y) * HANDLE);
        let c2 = (next + (mid - next) * HANDLE, y + (control_y - y) * HANDLE);
        path.curveToPoint_controlPoint1_controlPoint2(
            NSPoint::new(next, y),
            NSPoint::new(c1.0, c1.1),
            NSPoint::new(c2.0, c2.1),
        );
        (x, sign) = (next, -sign);
    }
    path.setLineWidth(thick);
    path.stroke();
}
