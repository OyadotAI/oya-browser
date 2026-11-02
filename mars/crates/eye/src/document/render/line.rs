/** Line boxes: baseline and vertical alignment of each finished line, then text-align and inline bounds. */
use super::flow::*;
use crate::document::style::VerticalAlign;

/** What a line fragment moves when its line settles. */
#[derive(Clone, Copy, Debug)]
pub(super) enum Piece {
    /** One painted text run: the item and the index of the run in its lines. */
    Text { item: usize, line: usize },
    /** An atomic inline (inline-block, image, control, svg): every item in `start..end`. */
    Atomic { start: usize, end: usize },
}
/** One fragment on the current line, measured from the line's top when it was placed. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Frag {
    pub piece: Piece,
    /** Distance from the fragment's top to its baseline. */
    pub ascent: f64,
    /** Fragment height, margins included for atomics. */
    pub height: f64,
    pub align: VerticalAlign,
    /** How far `vertical-align: <length-percentage>` raises the baseline. */
    pub shift: f64,
    /** Left margin of an atomic, so text-align measures its margin box. */
    pub margin: (f64, f64),
}
/** The block's own font metrics every line includes, as CSS's strut does. */
#[derive(Clone, Copy, Debug)]
pub(super) struct Strut {
    pub ascent: f64,
    pub height: f64,
    pub font: f64,
    /** The font's own ascent and descent without leading, where `text-top` and `text-bottom`
     * align (Blink's TextTop and the primary font's descent). */
    pub text: (f64, f64),
    /** Where `text-box-trim` cuts the line around its baseline: the font's ascent and descent
     * at `text-box-edge` (Blink's InlineBoxState::AdjustEdges for the line's root style). */
    pub edges: (f64, f64),
}
impl Strut {
    /** The strut of a block with this style: its line-height around its primary font. */
    pub fn of(style: &crate::document::ComputedStyle) -> Self {
        let (ascent, content) = super::sizing::face_metrics(style);
        Self {
            ascent: super::sizing::text_ascent(style),
            height: super::sizing::advance(style),
            font: style.font_size,
            text: (ascent, content - ascent),
            edges: trim_edges(style, (ascent, content - ascent)),
        }
    }
}
/** The font's ascent at the `text-box-edge` over edge and descent at its under edge, given
 * its full `text` ascent and descent. */
fn trim_edges(style: &crate::document::ComputedStyle, (ascent, descent): (f64, f64)) -> (f64, f64) {
    use crate::document::style::text_box::{OVER_CAP, OVER_EX, UNDER_ALPHABETIC};
    let [over, under] = style.text_box_edge;
    let face = || crate::platform::edges(super::sizing::face(style));
    let over = match over {
        OVER_CAP => face().0,
        OVER_EX => face().1,
        _ => ascent,
    };
    (
        over,
        if under == UNDER_ALPHABETIC {
            0.0
        } else {
            descent
        },
    )
}
/** Horizontal alignment of finished lines inside their available width. */
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Align {
    #[default]
    Left,
    Center,
    Right,
    /** Free space spread over the line's expansion opportunities (`text-align: justify`). */
    Justify(super::justify::Spread),
}
/** A finished line waiting for `settle`: each fragment with its vertical offset, and the room it had. */
pub(super) struct Finished {
    frags: Vec<(Frag, f64)>,
    /** Out-of-flow boxes on the line: (identity, fragments before it, line top). */
    statics: Vec<(usize, usize, f64, f64)>,
    left: f64,
    width: f64,
    hang: Hang,
    /** The line's `text-indent`, measured from its start edge. */
    indent: f64,
}
/** Preserved white space (`pre-wrap` spaces and tabs) at the end of a line, which hangs past
 * its end edge, after Blink's LineInfo::ComputeTrailingSpaceWidth. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Hang {
    /** The trailing spaces' width. */
    pub width: f64,
    /** The line ended at a soft wrap, so all of it hangs. */
    pub soft: bool,
    /** The spaces are in a right-to-left paragraph. */
    pub rtl: bool,
}
impl Hang {
    /** How much of the line's `content` width hangs in `available`: everything before a soft
     * wrap, only what overflows before a forced break or at the paragraph's end (CSS Text's
     * conditional hang). Right to left it all hangs, as trailing white space reorders to the
     * line's visual end (UAX #9 rule L1) and so never moves the text off its start edge. */
    // ponytail: lines lay out left to right, so a right-to-left line's trailing spaces sit at
    // its right past the edge instead of at its left; same text, different hanging background.
    fn used(self, content: f64, available: f64) -> f64 {
        if self.soft || self.rtl {
            self.width
        } else {
            self.width.min((content - available).max(0.0))
        }
    }
}
/** Subscripts drop by a fifth of the parent font, as Chromium does. */
const SUB_SHIFT: f64 = 0.2;
/** Superscripts rise by a third of the parent font. */
const SUPER_SHIFT: f64 = 1.0 / 3.0;
/** Half the parent's x-height, which `middle` centers on, as a share of its font size. */
const HALF_X_HEIGHT: f64 = 0.25;

/** Baseline-aligns the current fragments; returns the line height and stores the offsets for `settle`. */
pub(super) fn finish(flow: &mut Flow) -> f64 {
    let mut frags = std::mem::take(&mut flow.frags);
    let statics = std::mem::take(&mut flow.statics);
    let phantoms = std::mem::take(&mut flow.phantoms);
    if frags.is_empty() {
        return 0.0;
    }
    frags.extend(phantoms);
    let strut = flow.strut;
    let font = strut.map_or(0.0, |strut| strut.font);
    // Blink shifts sub and super by a fifth and a third of the font plus one layout pixel,
    // which is a device pixel at 2x (half a CSS px).
    let pixel = 1.0 / super::sizing::DEVICE_SCALE;
    let raise = |frag: &Frag| match frag.align {
        VerticalAlign::Sub => -(font * SUB_SHIFT + pixel),
        VerticalAlign::Super => font * SUPER_SHIFT + pixel,
        _ => frag.shift,
    };
    let anchored = |frag: &Frag| matches!(frag.align, VerticalAlign::Top | VerticalAlign::Bottom);
    // The baseline sits as low as the tallest ascent above it needs.
    let mut baseline = strut.map_or(0.0, |strut| strut.ascent);
    for frag in frags.iter().filter(|frag| !anchored(frag)) {
        baseline = baseline.max(match frag.align {
            VerticalAlign::Middle => frag.height / 2.0 + font * HALF_X_HEIGHT,
            // These hang from the parent's text edges; `lowest` below extends the line for them.
            VerticalAlign::TextTop | VerticalAlign::TextBottom => 0.0,
            _ => frag.ascent + raise(frag),
        });
    }
    let top_of = |frag: &Frag| match frag.align {
        VerticalAlign::Middle => baseline - font * HALF_X_HEIGHT - frag.height / 2.0,
        // Blink's kTextTop and kTextBottom: the box's leading edges meet the parent's font
        // ascent and descent, not its line-height, so leading never shifts them.
        VerticalAlign::TextTop => baseline - strut.map_or(frag.ascent, |strut| strut.text.0),
        VerticalAlign::TextBottom => {
            baseline + strut.map_or(0.0, |strut| strut.text.1) - frag.height
        }
        _ => baseline - frag.ascent - raise(frag),
    };
    let lowest = frags
        .iter()
        .filter(|frag| !anchored(frag))
        .map(&top_of)
        .fold(0.0_f64, f64::min);
    let mut height = strut.map_or(0.0, |strut| baseline - strut.ascent + strut.height) - lowest;
    for frag in frags.iter().filter(|frag| !anchored(frag)) {
        height = height.max(top_of(frag) - lowest + frag.height);
    }
    // Blink's MetricsForTopAndBottomAlign: a top or bottom box taller than the aligned subtree
    // keeps the subtree on the edge it names and extends the other one, so a tall `bottom` box
    // pushes the subtree down rather than hanging below it.
    let (ascent, descent) = (baseline - lowest, height - (baseline - lowest));
    let (mut over, mut under) = (ascent, descent);
    for frag in frags.iter().filter(|frag| anchored(frag)) {
        if frag.height > over + under {
            (over, under) = match frag.align {
                VerticalAlign::Top => (ascent, frag.height - ascent),
                _ => (frag.height - descent, descent),
            };
        }
    }
    let mut height = over + under;
    let down = over - ascent;
    let cut = trim_line(flow, strut, height, baseline - lowest + down);
    height -= cut;
    let placed = frags
        .into_iter()
        .map(|frag| {
            let top = match frag.align {
                VerticalAlign::Top => 0.0,
                VerticalAlign::Bottom => height - frag.height,
                _ => top_of(&frag) - lowest + down,
            };
            // vertical-lr: the line's over edge is the far (physical right) side.
            let top = match flow.mirror_lines {
                true => height - (top - cut) - frag.height + cut,
                false => top,
            };
            (frag, top - cut)
        })
        .collect();
    let (left, width) = flow.room_at(flow.y);
    flow.finished.push(Finished {
        frags: placed,
        statics,
        left,
        width,
        hang: flow.hang,
        indent: std::mem::take(&mut flow.indent),
    });
    height
}
/**
 * Blink's ApplyTextBoxTrim for a line of a block that trims: the first formatted line's top
 * moves down to the root font's over edge (the returned cut, which the caller takes off the
 * line and its fragments), and every line records how far its bottom sits below the under edge,
 * so the block's end trims the last one with `trim_end`. Content past the edges overflows.
 */
fn trim_line(flow: &mut Flow, strut: Option<Strut>, height: f64, baseline: f64) -> f64 {
    let Some(strut) = strut else {
        return 0.0;
    };
    let cut = if std::mem::take(&mut flow.trim[0]) {
        baseline - strut.edges.0
    } else {
        0.0
    };
    // The text runs' own advance (`flow.line`) and the bottom they reached lose the cut with
    // the line box.
    flow.line -= cut;
    if flow.bottom > flow.y {
        flow.bottom = (flow.bottom - cut).max(flow.y);
    }
    if flow.trim[1] {
        flow.tail = Some((height - baseline - strut.edges.1, Vec::new(), f64::INFINITY));
    }
    cut
}
/**
 * Blink's TrimBlockEndBy at a block's end: when this block trims its end (`own`) and its last
 * formatted line came last, the line's bottom rises to the under edge, taking the block's
 * content height and every nested last block (`tail`'s items) with it.
 */
pub(super) fn trim_end(flow: &mut Flow, items: &mut [DisplayItem], own: bool) {
    if !own {
        return;
    }
    let Some((cut, nested, end)) = flow.tail.take() else {
        return;
    };
    // A `<br clear>` moved the end below the line to clear floats; the line still trims, but
    // the clearance keeps the block's end where it was.
    if super::flow::cleared_past(flow, end) {
        return;
    }
    if flow.bottom <= end.min(flow.y) + LINE_EPSILON {
        flow.bottom = flow.y - cut;
    }
    flow.y -= cut;
    for index in nested {
        if let Some(item) = items.get_mut(index) {
            item.height = (item.height - cut).max(0.0);
        }
    }
}
/** How far past a line's bottom the flow may end and still end at that line. */
pub(super) const LINE_EPSILON: f64 = 0.01;
/** Applies every finished line's offsets and alignment to the items, then refits inline boxes. */
pub(super) fn settle(
    context: &mut LayoutContext<'_>,
    flow: &mut Flow,
    items: &mut [DisplayItem],
    from: usize,
) -> EngineResult<()> {
    let aligned = flow.align;
    // A block of only blocks has no lines; its nested blocks already refit their own inlines.
    if flow.finished.is_empty() {
        return Ok(());
    }
    for line in std::mem::take(&mut flow.finished) {
        context.charge(line.frags.len() + 1)?;
        if line.hang.soft {
            super::hyphenate::mark(context, items, &line.frags)?;
        }
        let mut extent: Option<(f64, f64)> = None;
        for (frag, dy) in &line.frags {
            move_piece(items, frag.piece, 0.0, *dy);
            if let Some((left, right)) = bounds(items, frag) {
                let (l, r) = extent.unwrap_or((left, right));
                extent = Some((l.min(left), r.max(right)));
            }
        }
        let Some((left, right)) = extent else {
            continue;
        };
        // The line's content starts at its start edge: a text-indent or an inline box's start
        // margin, border and padding before the first fragment take part of the line, as in
        // Blink's line offset, so alignment distributes only what is left.
        let left = left.min(line.left);
        let right = (right - line.hang.used(right - left, line.width)).max(left);
        let free = (line.width - (right - left)).max(0.0);
        let align = match if line.hang.soft {
            aligned.lines
        } else {
            aligned.last
        } {
            Align::Justify(_) if super::justify::spread(context, items, &line.frags, free)? => {
                Align::Left
            }
            Align::Justify(how) => how.start(),
            other => other,
        };
        let mut dx = line.left - left
            + match align {
                Align::Center => free / 2.0,
                Align::Right => free,
                _ => 0.0,
            };
        if flow.rtl && line.indent != 0.0 && align == Align::Right {
            // A right-to-left indent comes off the right edge (CSS Text 3 text-indent).
            dx = line.left + line.width - line.indent - right;
        }
        if align != Align::Left && dx != 0.0 {
            for (frag, _) in &line.frags {
                move_piece(items, frag.piece, dx, 0.0);
            }
        }
        super::bidi::reorder(items, &line.frags, flow.rtl);
        place_statics(context, items, &line, flow.rtl);
        if let Some(owner) = flow.ellipsis {
            truncate(context, items, &line, owner)?;
        }
    }
    refit_inlines(context, items, from)
}
/** The ellipsis string Blink uses for `text-overflow: ellipsis` (U+2026). */
pub(super) const ELLIPSIS: &str = "\u{2026}";
/** Overflow below this is layout rounding, not content past the end edge. */
const OVERFLOW_EPSILON: f64 = 0.01;
/**
 * Blink's LineTruncator (LTR): a line whose content passes the block's end edge keeps what fits
 * before an ellipsis measured in the block's font, hides the rest and paints "…" right after the
 * kept content. The first character or atomic of a line is clipped rather than ellipsed
 * (css-overflow-3). Without it rustdoc's sidebar links were cut mid-letter with no "…".
 */
fn truncate(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    line: &Finished,
    owner: usize,
) -> EngineResult<()> {
    let end = line.left + line.width;
    let mut frags = line
        .frags
        .iter()
        .filter_map(|(frag, _)| bounds(items, frag).map(|extent| (extent, frag.piece)))
        .collect::<Vec<_>>();
    if frags
        .iter()
        .all(|((_, right), _)| *right <= end + OVERFLOW_EPSILON)
    {
        return Ok(());
    }
    frags.sort_by(|a, b| a.0.0.total_cmp(&b.0.0));
    let style = &context.node(owner)?.style;
    let limit = end - context.measure(ELLIPSIS, super::sizing::face_for(style, Some(ELLIPSIS)))?;
    let mut last_text = None;
    let mut cut = false;
    for (index, ((_, right), piece)) in frags.into_iter().enumerate() {
        if !cut && right <= limit {
            if let Piece::Text { item, line } = piece {
                last_text = Some((item, line));
            }
            continue;
        }
        if cut {
            hide(items, piece);
            continue;
        }
        cut = true;
        match piece {
            Piece::Text { item, line } => {
                ellipsize(context, items, (item, line), limit, index == 0)?
            }
            Piece::Atomic { .. } if index == 0 => {}
            Piece::Atomic { .. } => {
                hide(items, piece);
                if let Some(run) = last_text {
                    ellipsize(context, items, run, f64::INFINITY, false)?;
                }
            }
        }
    }
    Ok(())
}
/** Cuts one text run to the longest prefix ending by `limit` (at least one character when it
 * starts the line) and appends the ellipsis, measured in the run's own font. */
pub(super) fn ellipsize(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    run: (usize, usize),
    limit: f64,
    first: bool,
) -> EngineResult<()> {
    ellipsize_with(context, items, run, limit, first, ELLIPSIS)
}
/** `ellipsize` with any marker string, as `block-ellipsis` gives one. */
pub(super) fn ellipsize_with(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    (item, line): (usize, usize),
    limit: f64,
    first: bool,
    marker: &str,
) -> EngineResult<()> {
    let Some(run) = items
        .get(item)
        .and_then(|found| found.lines.get(line))
        .cloned()
    else {
        return Ok(());
    };
    let style = context.node(items[item].local_id)?.style.clone();
    let face = super::sizing::face_for(&style, Some(&run.text));
    let room = limit - run.x;
    let ends = run
        .text
        .char_indices()
        .map(|(at, _)| at)
        .skip(1)
        .chain([run.text.len()]);
    let ends = ends.collect::<Vec<_>>();
    let (mut low, mut high) = (usize::from(first), ends.len());
    // Binary search for the most characters whose width fits the room.
    while low < high {
        let middle = (low + high).div_ceil(2);
        if context.measure(&run.text[..ends[middle - 1]], face)? <= room {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let kept = if low == 0 {
        ""
    } else {
        &run.text[..ends[low - 1]]
    };
    let text = format!("{kept}{marker}");
    write_run(context, items, (item, line), face, text)
}
/** Replaces one run's text and shaped width. */
fn write_run(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    (item, line): (usize, usize),
    face: crate::platform::Face,
    text: String,
) -> EngineResult<()> {
    let width = context.measure(&text, face)?;
    let target = &mut items[item].lines[line];
    target.text = text;
    target.width = width;
    Ok(())
}
/** Hides a fragment that falls after the ellipsis. */
fn hide(items: &mut [DisplayItem], piece: Piece) {
    match piece {
        Piece::Text { item, line } => {
            if let Some(run) = items
                .get_mut(item)
                .and_then(|found| found.lines.get_mut(line))
            {
                run.text.clear();
                run.width = 0.0;
            }
        }
        Piece::Atomic { start, end } => {
            for item in items.iter_mut().take(end).skip(start) {
                item.opacity = 0.0;
            }
        }
    }
}
/** Moves the fragments already on the current line `dx` along, as a left float placed beside
 * them does. */
pub(super) fn shift_current(flow: &Flow, items: &mut [DisplayItem], dx: f64) {
    for frag in &flow.frags {
        move_piece(items, frag.piece, dx, 0.0);
    }
}
/** Horizontal extent of a fragment, margins included for atomics. */
fn bounds(items: &[DisplayItem], frag: &Frag) -> Option<(f64, f64)> {
    match frag.piece {
        Piece::Text { item, line } => {
            let line = items.get(item)?.lines.get(line)?;
            Some((line.x, line.x + line.width))
        }
        Piece::Atomic { start, .. } => {
            let item = items.get(start)?;
            Some((item.x - frag.margin.0, item.x + item.width + frag.margin.1))
        }
    }
}
/** Re-derives the static position of each out-of-flow box on a settled line from the fragment
before it in logical order, now aligned and in visual order: just after it (to its left in a
right-to-left line), or at the line's start edge when it came first. */
fn place_statics(
    context: &mut LayoutContext<'_>,
    items: &[DisplayItem],
    line: &Finished,
    rtl: bool,
) {
    for &(local, before, y, space) in &line.statics {
        let edge = match before.checked_sub(1).and_then(|at| line.frags.get(at)) {
            Some((frag, _)) => {
                bounds(items, frag).map(|(l, r)| if rtl { l - space } else { r + space })
            }
            None => line
                .frags
                .iter()
                .filter_map(|(frag, _)| bounds(items, frag))
                .map(|(l, r)| if rtl { r } else { l })
                .reduce(if rtl { f64::max } else { f64::min }),
        };
        if let Some(x) = edge {
            context.defer(local, (x, y));
        }
    }
}
/** A fragment's horizontal extent, margins included for an atomic. */
pub(super) fn extent(items: &[DisplayItem], frag: &Frag) -> Option<(f64, f64)> {
    bounds(items, frag)
}
/** Moves one fragment along the line. */
pub(super) fn shift(items: &mut [DisplayItem], piece: Piece, dx: f64) {
    move_piece(items, piece, dx, 0.0);
}
/** Moves one fragment; atomics move with every item they painted. */
fn move_piece(items: &mut [DisplayItem], piece: Piece, dx: f64, dy: f64) {
    match piece {
        Piece::Text { item, line } => {
            if let Some(line) = items
                .get_mut(item)
                .and_then(|item| item.lines.get_mut(line))
            {
                line.x += dx;
                line.y += dy;
            }
        }
        Piece::Atomic { start, end } => {
            for item in items.iter_mut().take(end).skip(start) {
                translate(item, dx, dy);
            }
        }
    }
}
/** Shifts every absolute coordinate an item carries. */
pub(super) fn translate(item: &mut DisplayItem, dx: f64, dy: f64) {
    if dx == 0.0 && dy == 0.0 {
        return;
    }
    item.x += dx;
    item.y += dy;
    if let Some(origin) = &mut item.layout_origin {
        origin[0] += dx;
        origin[1] += dy;
    }
    for line in item.lines.iter_mut().chain(item.validation.iter_mut()) {
        line.x += dx;
        line.y += dy;
    }
    for path in &mut item.vector {
        path.translate(dx, dy);
    }
}
/** A text-bearing inline element (not atomic, not replaced) whose box spans its runs. A
 * floated or absolutely positioned one is blockified and keeps its own box. */
fn inline_box(context: &LayoutContext<'_>, item: &DisplayItem) -> bool {
    context.node(item.local_id).is_ok_and(|node| {
        node.style.inline
            && !node.style.inline_block
            && node.style.float == crate::document::style::Float::None
            && !super::flow::out_of_flow(node)
            && !super::flow::frame(node)
            && ![
                "img", "canvas", "svg", "input", "button", "select", "textarea", "br",
            ]
            .contains(&node.tag.as_str())
    })
}
/** Inline boxes cover their own runs and their inline descendants, plus padding and borders. */
fn refit_inlines(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    from: usize,
) -> EngineResult<()> {
    type Rect = (f64, f64, f64, f64);
    let mut extents = std::collections::BTreeMap::<usize, Rect>::new();
    let union = |a: Option<Rect>, b: Rect| match a {
        Some(a) => (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
        None => b,
    };
    for index in (from..items.len()).rev() {
        context.charge(items[index].lines.len() + 1)?;
        let inline = inline_box(context, &items[index]);
        let item = &items[index];
        let rect = if inline {
            // Blink's inline box is the font's content area (ascent plus descent) around the
            // baseline, not the line-height: half the leading sits above and below it.
            let style = &context.node(item.local_id)?.style;
            let (_, content) = super::sizing::face_metrics(style);
            let leading = super::sizing::half_leading(style);
            let mut rect = extents.remove(&item.local_id);
            for line in &item.lines {
                let top = line.y + leading;
                rect = Some(union(
                    rect,
                    (line.x, top, line.x + line.width, top + content),
                ));
            }
            let Some(rect) = rect else {
                continue;
            };
            let node = context.node(item.local_id)?;
            let sides = super::sizing::border_sides(&node.style);
            let pad = node.style.padding;
            // On one line the box also spans its children's margins and its own leading space.
            let rect = if rect.3 - rect.1 <= content + 0.01 {
                let (lead, trail) = context.inline_edges(item.local_id);
                (rect.0 - lead, rect.1, rect.2 + trail, rect.3)
            } else {
                rect
            };
            let rect = (
                rect.0 - pad[3] - sides[3],
                rect.1 - pad[0] - sides[0],
                rect.2 + pad[1] + sides[1],
                rect.3 + pad[2] + sides[2],
            );
            let item = &mut items[index];
            item.x = rect.0;
            item.y = rect.1;
            item.width = (rect.2 - rect.0).max(0.0);
            item.height = (rect.3 - rect.1).max(0.0);
            rect
        } else {
            (item.x, item.y, item.x + item.width, item.y + item.height)
        };
        let node = context.node(items[index].local_id)?;
        if !inline && !node.style.inline && !node.style.inline_block {
            continue;
        }
        let Some(parent) = node.parent else {
            continue;
        };
        if context.node(parent).is_ok_and(|parent| {
            parent.style.inline
                && !parent.style.inline_block
                && !super::flow::frame(parent)
                && ![
                    "img", "canvas", "svg", "input", "button", "select", "textarea", "br",
                ]
                .contains(&parent.tag.as_str())
        }) {
            let current = extents.remove(&parent);
            extents.insert(parent, union(current, rect));
        }
    }
    Ok(())
}
