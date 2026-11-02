/**
 * Text decoration lines and outline rings in agent screenshots, placed and shaped as the desktop
 * window strokes them (desktop/canvas/lines.rs and outline.rs), so an agent sees the underline
 * or focus ring a person sees. Every dash, dot and wave step is planned over the canvas columns
 * or rows the line actually covers, so the work is bounded by the screenshot's size, never by
 * the length or thinness page CSS asks for.
 */
use super::{Canvas, glyphs};
use crate::document::{BorderStyle, DecorationStyle, DisplayItem, PaintLine, painter_rgba};

/// Underlines sit this far below the baseline (desktop/canvas/lines.rs).
const UNDERLINE_OFFSET: f64 = 1.5;
/// Line-throughs sit this fraction of the font size above the baseline: the desktop's share of
/// the ascender, with the ascender taken as the font's usual 0.8 em.
const STRIKE_HEIGHT: f64 = 0.35 * 0.8;
/// The font size per point of default thickness: a sixteenth of the font.
const FONT_PER_THICKNESS: f64 = 16.0;
/// The default thickness never drops below a point or rises above two and a half.
const THICKNESS_RANGE: (f64, f64) = (1.0, 2.5);
/// The thinnest line drawn, in points, so a zero width from page CSS still paints and plans.
const MIN_STROKE: f64 = 0.5;
/// Double lines are two bands of one thickness, one thickness apart.
const DOUBLE_SPACING: f64 = 2.0;
/// Dashes and their gaps, in thicknesses.
const DASH: (f64, f64) = (3.0, 2.0);
/// Dots and their gaps, in thicknesses: a square dot one thickness wide.
const DOT: (f64, f64) = (1.0, 1.0);
/// A wave's half period and crest height, in thicknesses.
const WAVE_HALF_PERIOD: f64 = 2.0;
const WAVE_CREST: f64 = 1.0;
/// The most steps one band plans: one per point of the longest screenshot edge.
const MAX_STEPS: f64 = 2048.0;
/// A double outline is three equal bands: line, gap, line.
const DOUBLE_THIRDS: f64 = 3.0;

/** One straight decoration or outline side: where it runs, how thick, and in what shape. */
#[derive(Clone, Copy)]
struct Band {
    /// Start and end along the line, in document points.
    span: (f64, f64),
    /// The line's top edge across the span (left edge for a vertical side).
    at: f64,
    /// Thickness in points.
    thick: f64,
    /// Whether the band runs down the page instead of across it.
    vertical: bool,
}

impl Canvas {
    /** A run's underline and line-through, in the item's decoration shape and thickness. */
    pub(super) fn decorate(&mut self, item: &DisplayItem, line: &PaintLine, ink: [u8; 4]) {
        if !item.underline && !item.strikethrough {
            return;
        }
        let baseline = line.y + glyphs::baseline_offset(item);
        let thick = decoration_thickness(item);
        let shape = item.decoration.map(|d| d.style).unwrap_or_default();
        let span = (line.x, line.x + line.width);
        if item.underline {
            let shift = item.decoration.and_then(|d| d.offset).unwrap_or(0.0);
            let at = baseline + UNDERLINE_OFFSET + shift - thick / 2.0;
            self.line(
                shape,
                Band {
                    span,
                    at,
                    thick,
                    vertical: false,
                },
                item,
                ink,
            );
        }
        if item.strikethrough {
            let at = baseline - item.font_size * STRIKE_HEIGHT - thick / 2.0;
            self.line(
                shape,
                Band {
                    span,
                    at,
                    thick,
                    vertical: false,
                },
                item,
                ink,
            );
        }
    }

    /** The item's outline ring outside its border box, in its own color or the text color. */
    pub(super) fn outline(&mut self, item: &DisplayItem, ink: [u8; 4]) {
        let Some(outline) = &item.outline else {
            return;
        };
        let color = outline
            .color
            .as_deref()
            .and_then(painter_rgba)
            .unwrap_or(ink);
        let width = outline.width.max(MIN_STROKE);
        let grow = outline.offset + width;
        let ring = [
            item.x - grow,
            item.y - grow,
            item.width + 2.0 * grow,
            item.height + 2.0 * grow,
        ];
        if ring[2] <= 0.0 || ring[3] <= 0.0 {
            return;
        }
        match outline.style {
            BorderStyle::Double if width >= DOUBLE_THIRDS => {
                let third = width / DOUBLE_THIRDS;
                self.ring(ring, third, outline.style, item, color);
                let inner = inset(ring, 2.0 * third);
                self.ring(inner, third, outline.style, item, color);
            }
            style => self.ring(ring, width, style, item, color),
        }
    }

    /** A rectangle's four sides, each a band of `width` inside `ring`. */
    fn ring(
        &mut self,
        ring: [f64; 4],
        width: f64,
        style: BorderStyle,
        item: &DisplayItem,
        ink: [u8; 4],
    ) {
        let [x, y, w, h] = ring;
        let shape = match style {
            BorderStyle::Dashed => DecorationStyle::Dashed,
            BorderStyle::Dotted => DecorationStyle::Dotted,
            _ => DecorationStyle::Solid,
        };
        let sides = [
            Band {
                span: (x, x + w),
                at: y,
                thick: width,
                vertical: false,
            },
            Band {
                span: (x, x + w),
                at: y + h - width,
                thick: width,
                vertical: false,
            },
            Band {
                span: (y + width, y + h - width),
                at: x,
                thick: width,
                vertical: true,
            },
            Band {
                span: (y + width, y + h - width),
                at: x + w - width,
                thick: width,
                vertical: true,
            },
        ];
        for side in sides {
            self.line(shape, side, item, ink);
        }
    }

    /** One band in `shape`: solid, doubled, dashed, dotted or wavy. */
    fn line(&mut self, shape: DecorationStyle, band: Band, item: &DisplayItem, ink: [u8; 4]) {
        let Some(band) = self.visible(band) else {
            return;
        };
        let thick = band.thick;
        match shape {
            DecorationStyle::Solid => self.segment(band, band.span, item, ink),
            DecorationStyle::Double => {
                self.segment(band, band.span, item, ink);
                let second = Band {
                    at: band.at + thick * DOUBLE_SPACING,
                    ..band
                };
                self.segment(second, band.span, item, ink);
            }
            DecorationStyle::Dashed => {
                self.dashes(band, (DASH.0 * thick, DASH.1 * thick), item, ink)
            }
            DecorationStyle::Dotted => self.dashes(band, (DOT.0 * thick, DOT.1 * thick), item, ink),
            DecorationStyle::Wavy => self.wave(band, item, ink),
        }
    }

    /** The band cut to the canvas along its run, so every plan below walks visible points only. */
    fn visible(&self, band: Band) -> Option<Band> {
        let (low, high) = match band.vertical {
            true => (self.top, self.top + self.height as f64),
            false => (self.left, self.left + self.width as f64),
        };
        let span = (band.span.0.max(low), band.span.1.min(high));
        (span.0 < span.1 && band.thick.is_finite()).then_some(Band { span, ..band })
    }

    /** Dashes of `on` points with `off` gaps, phased from the band's start; at most one dash per
    point of visible length, since both parts are at least `MIN_STROKE`. */
    fn dashes(&mut self, band: Band, (on, off): (f64, f64), item: &DisplayItem, ink: [u8; 4]) {
        let period = (on + off).max(2.0 * MIN_STROKE);
        // Counted, not accumulated: far from the origin `from += period` could stop moving.
        for index in 0..steps(band, period) {
            let from = band.span.0 + index as f64 * period;
            let to = (from + on.max(MIN_STROKE)).min(band.span.1);
            if to <= from {
                break;
            }
            self.segment(band, (from, to), item, ink);
        }
    }

    /** A zigzag wave one crest above and below the band, stepped one point at a time. */
    fn wave(&mut self, band: Band, item: &DisplayItem, ink: [u8; 4]) {
        let half = band.thick * WAVE_HALF_PERIOD;
        let crest = band.thick * WAVE_CREST;
        for index in 0..steps(band, 1.0) {
            let from = band.span.0 + index as f64;
            let to = (from + 1.0).min(band.span.1);
            if to <= from {
                break;
            }
            let phase = (index as f64 / half).rem_euclid(2.0);
            let rise = if phase < 1.0 { phase } else { 2.0 - phase };
            let shifted = Band {
                at: band.at + crest * (2.0 * rise - 1.0),
                ..band
            };
            self.segment(shifted, (from, to), item, ink);
        }
    }

    /** Fills `(from, to)` of the band, inside the item's clip. */
    fn segment(&mut self, band: Band, (from, to): (f64, f64), item: &DisplayItem, ink: [u8; 4]) {
        let rect = match band.vertical {
            true => [band.at, from, band.thick, to - from],
            false => [from, band.at, to - from, band.thick],
        };
        self.fill(rect, item.clip.map(|c| [c.x, c.y, c.width, c.height]), ink);
    }
}

/** How many `period` steps cover the band's visible span. `visible` cut the span to the canvas,
so this is at most the screenshot's edge divided by the period. */
fn steps(band: Band, period: f64) -> usize {
    ((band.span.1 - band.span.0) / period)
        .ceil()
        .clamp(0.0, MAX_STEPS) as usize
}

/** The authored thickness, else one derived from the font size, never under `MIN_STROKE`. */
fn decoration_thickness(item: &DisplayItem) -> f64 {
    item.decoration
        .and_then(|d| d.thickness)
        .unwrap_or_else(|| {
            (item.font_size / FONT_PER_THICKNESS).clamp(THICKNESS_RANGE.0, THICKNESS_RANGE.1)
        })
        .max(MIN_STROKE)
}

/** `rect` shrunk by `by` on every side. */
fn inset([x, y, w, h]: [f64; 4], by: f64) -> [f64; 4] {
    [
        x + by,
        y + by,
        (w - 2.0 * by).max(0.0),
        (h - 2.0 * by).max(0.0),
    ]
}
