/** Floats: placement against earlier floats, the exclusions that shorten lines, and clearance. */
use super::flow::*;
use crate::document::style::{Clear, Float};

/** Overflow a line forgives: one Chromium LayoutUnit. */
pub(super) const LINE_FIT_SLACK: f64 = 1.0 / 64.0;
/** Floats one formatting context remembers; older ones above the cursor are pruned first. */
pub(super) const FLOATS: usize = 64;

/** A placed float's margin box in document coordinates. */
#[derive(Clone, Copy, Debug)]
pub(super) struct FloatBox {
    pub left: bool,
    pub x0: f64,
    pub x1: f64,
    pub top: f64,
    pub bottom: f64,
    /// An initial letter's exclusion (render/initial_letter.rs), which later floats and blocks
    /// clear.
    pub initial: bool,
}
impl Flow {
    /** Left edge and width of line room at `y`, past every float that overlaps that line. */
    pub fn room_at(&self, y: f64) -> (f64, f64) {
        let probe = self
            .line
            .max(self.strut.map_or(1.0, |strut| strut.height))
            .max(1.0);
        let (mut left, mut right) = (self.x, self.x + self.width);
        for float in self
            .floats
            .iter()
            .filter(|f| f.top < y + probe && f.bottom > y)
        {
            if float.left {
                left = left.max(float.x1);
            } else {
                right = right.min(float.x0);
            }
        }
        (left, (right - left).max(0.0))
    }
    /** The current line's start: content edge plus left float intrusion. */
    pub fn left(&self) -> f64 {
        self.room_at(self.y).0
    }
    /** The current line's width between floats. */
    pub fn room(&self) -> f64 {
        self.room_at(self.y).1
    }
    /** Whether `extent` more fits on the current line. A run overflowing by less than one of
     * Chromium's 1/64 px layout units still fits: sums of the same widths in another order
     * differ in the last bits, and a flex item sized to its text's max-content wrapped it
     * (BBC's "Watch Live"). */
    pub fn fits(&self, extent: f64) -> bool {
        self.cursor + extent <= self.room() + LINE_FIT_SLACK
    }
    /** Starts an empty line past every left float a box `height` tall reaches, not only those at
     * the line's top, by moving the line's start (floats-wrap-top-below-inline-001l: a lower,
     * wider float the inline-block would overlap pushes it right). */
    pub fn start_past_tall_floats(&mut self, height: f64) {
        let (left, _) = self.room_at_height(self.y, height);
        let shift = left - self.left();
        if self.cursor == 0.0 && shift > 0.0 {
            self.cursor = shift;
        }
    }
    /** Moves an empty line below floats until a box `width` wide and `height` tall fits beside
     * every float its height reaches, not only those at the line's top (an inline-block on a new
     * line that a lower float would overlap goes below it, floats-wrap-top-below-inline-002l). */
    pub fn fit_tall(&mut self, width: f64, height: f64) {
        for _ in 0..=self.floats.len() {
            if self.cursor > 0.0 || self.room_at_height(self.y, height).1 >= width {
                return;
            }
            let (y, extent) = (self.y, height);
            let below = self
                .floats
                .iter()
                .filter(|f| f.top < y + extent && f.bottom > y)
                .map(|f| f.bottom)
                .fold(f64::INFINITY, f64::min);
            if !below.is_finite() {
                return;
            }
            self.y = below;
            self.collapsible = 0.0;
        }
    }
    /** Moves an empty line below floats until `width` fits or no float narrows it. */
    pub fn fit(&mut self, width: f64) {
        for _ in 0..=self.floats.len() {
            if self.cursor > 0.0 || self.room() >= width {
                return;
            }
            let below = self
                .floats
                .iter()
                .filter(|f| f.top <= self.y && f.bottom > self.y)
                .map(|f| f.bottom)
                .fold(f64::INFINITY, f64::min);
            if !below.is_finite() {
                return;
            }
            self.y = below;
            self.collapsible = 0.0;
        }
    }
    /** The lowest float bottom on the cleared sides, if any. */
    pub fn clearance(&self, clear: Clear) -> Option<f64> {
        let side = |float: &&FloatBox| match clear {
            Clear::None => false,
            Clear::Left => float.left,
            Clear::Right => !float.left,
            Clear::Both => true,
        };
        self.floats
            .iter()
            .filter(side)
            .map(|f| f.bottom)
            .reduce(f64::max)
            .or(match clear {
                Clear::None => None,
                _ => self.cleared,
            })
    }
    /** Lowest bottom of every float, which a block formatting context root grows to contain. */
    pub fn floats_bottom(&self) -> f64 {
        self.floats
            .iter()
            .map(|f| f.bottom)
            .fold(self.cleared.unwrap_or(f64::NEG_INFINITY), f64::max)
    }
    /** Remembers a float, pruning ones entirely above the cursor when the list is full. */
    pub(super) fn add_float(&mut self, float: FloatBox) {
        if self.floats.len() >= FLOATS {
            let y = self.y;
            let mut lowest = self.cleared.unwrap_or(f64::NEG_INFINITY);
            self.floats.retain(|f| {
                let keep = f.bottom > y;
                if !keep {
                    lowest = lowest.max(f.bottom);
                }
                keep
            });
            self.cleared = lowest.is_finite().then_some(lowest);
            if self.floats.len() >= FLOATS {
                self.floats.remove(0);
            }
        }
        self.floats.push(float);
    }
}
/** A floated box: shrink-to-fit width, placed beside earlier floats at the current line or below it. */
pub(super) fn float(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let style = &node.style;
    let left = style.float == Float::Left;
    let margins = (style.margin[3], style.margin[1]);
    let width = shrink_width(context, node, flow.width, margins.0 + margins.1)?;
    let outer = width + margins.0 + margins.1;
    // Blink's PositionFloat: a float met mid-line goes beside that line when it fits next to
    // the content already there (a left one pushes that content over), else below the line.
    let beside = flow.cursor == 0.0 || outer <= flow.room() - flow.cursor + LINE_FIT_SLACK;
    let mut y = if beside { flow.y } else { flow.y + flow.line };
    if let Some(clear) = flow.clearance(style.clear) {
        y = y.max(clear);
    }
    // CSS 2.1 9.5.1 rule 5 (Blink's exclusion space floats_top): a float's top is never above
    // an earlier float's, so a right float after a wrapped left one starts on its row.
    y = flow.floats.iter().map(|f| f.top).fold(y, f64::max);
    y = y.max(super::initial_letter::clearance(flow).unwrap_or(y));
    let x = loop {
        let (start, room) = flow.room_at_height(y, 1.0);
        let row: Vec<_> = flow
            .floats
            .iter()
            .filter(|f| f.top <= y && f.bottom > y)
            .collect();
        // The opportunity is the BFC's shelf at this row: on the side the float does not hug, it
        // ends at the nearest opposite float even past this box's edge, so a float that overhangs
        // its containing block still fits beside a float outside that block (CSS2
        // floats-rule3-outside); with no opposite float it ends at the box edge (rule 7).
        let reach = if left {
            row.iter()
                .filter(|f| !f.left)
                .map(|f| f.x0)
                .fold(f64::INFINITY, f64::min)
                - start
        } else {
            start + room
                - row
                    .iter()
                    .filter(|f| f.left)
                    .map(|f| f.x1)
                    .fold(f64::NEG_INFINITY, f64::max)
        };
        let fits = room.max(if reach.is_finite() { reach } else { room }) >= outer;
        if fits || row.is_empty() {
            break if left { start } else { start + room - outer };
        }
        y = row.iter().map(|f| f.bottom).fold(f64::INFINITY, f64::min);
    };
    let bounds = BorderBox {
        x: x + margins.0,
        y: y + style.margin[0],
        width,
        height: None,
    };
    let start = items.len();
    let mut inner = Flow::new(bounds.x, bounds.y, width);
    inner.definite = flow.definite;
    node_layout(context, node, &mut inner, items, Some(bounds))?;
    let height = items.get(start).map_or(0.0, |item| item.height);
    let bottom = y + style.margin[0] + height + style.margin[2];
    flow.add_float(FloatBox {
        left,
        x0: x,
        x1: x + outer,
        top: y,
        bottom,
        initial: false,
    });
    if flow.strut_open {
        let index = flow.floats.len() - 1;
        flow.unpositioned.push((index, start..items.len(), y));
        flow.adjoining[usize::from(!left)] = true;
    }
    if left && flow.cursor > 0.0 && y == flow.y {
        super::line::shift_current(flow, items, outer);
    }
    Ok(())
}
impl Flow {
    /** Room at `y` for a box `height` tall. */
    pub(super) fn room_at_height(&self, y: f64, height: f64) -> (f64, f64) {
        let (mut left, mut right) = (self.x, self.x + self.width);
        for float in self
            .floats
            .iter()
            .filter(|f| f.top < y + height && f.bottom > y)
        {
            if float.left {
                left = left.max(float.x1);
            } else {
                right = right.min(float.x0);
            }
        }
        (left, (right - left).max(0.0))
    }
}
/** Authored width, or shrink-to-fit: max-content capped by the room, never below min-content. */
pub(super) fn shrink_width(
    context: &mut LayoutContext<'_>,
    node: &Node,
    available: f64,
    margins: f64,
) -> EngineResult<f64> {
    let room = (available - margins).max(0.0);
    let edges = horizontal_edges(node);
    if node.style.width.is_some() {
        let authored = border_width(node, available, room, None);
        // A table is never narrower than its grid's minimum, whatever its authored width
        // (floated-table-wider-than-specified).
        if super::table::is_table(context, node) {
            let minimum = context.intrinsic_widths(node.local_id)?.min_content + edges;
            return Ok(authored.max(minimum));
        }
        return Ok(authored);
    }
    let intrinsic = context.intrinsic_widths(node.local_id)?;
    let fit = intrinsic
        .max_content
        .min((room - edges).max(intrinsic.min_content));
    Ok(border_width(
        node,
        available,
        room.max(fit + edges),
        Some(fit),
    ))
}
