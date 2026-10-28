/**
 * `-webkit-line-clamp` after Blink's LineClampData: a clamping flow-root counts the line boxes of
 * its block formatting context (nested blocks' lines too, new formatting contexts' not), hides
 * everything after the Nth, ends its own height at that line and appends "…" to it.
 * Lines are found from laid-out geometry: text runs and atomic inline boxes, grouped into line
 * boxes by baseline and overlap.
 * ponytail: an empty line box with no text and no atomic is not counted; count them in the line
 * builder if a page needs it.
 */
use super::{flow::Flow, line::Align, sizing::LayoutContext};
use crate::document::{DisplayItem, Node, WhiteSpace};
use crate::protocol::EngineResult;

/** Runs whose tops are this close belong to one line box (baseline-aligned runs in different
fonts start a little apart). */
const SAME_LINE: f64 = 0.5;

/** The tag of a text node, which is never a block. */
const TEXT_TAG: &str = "#text";

/** One text run: its line box top and bottom, its item and its index in that item's lines. */
#[derive(Clone, Copy)]
struct Run {
    /// Line box top.
    top: f64,
    /// Line box bottom.
    bottom: f64,
    /// Baseline, which every run of one line box shares unless `vertical-align` moves it.
    base: f64,
    /// Display item index.
    item: usize,
    /// Index into the item's lines, or `ATOMIC` for an atomic inline's whole box.
    line: usize,
}

/** The `Run::line` of an atomic inline (inline-block, inline-flex, inline-table, an image):
the box itself is the line content. */
const ATOMIC: usize = usize::MAX;

/** Clamps the block at `items[index]` whose content `inner` holds: at `max-lines`, or with
`max-lines: none` (`line-clamp: auto`) at the last line its height or max-height holds. */
pub(super) fn clamp(
    context: &mut LayoutContext<'_>,
    node: &Node,
    items: &mut [DisplayItem],
    index: usize,
    inner: &mut Flow,
) -> EngineResult<()> {
    let style = &node.style;
    if !style.continue_collapse {
        return Ok(());
    }
    let lines = line_boxes(runs(context, node, items, index)?);
    let count = match style.max_lines {
        0 => fitting(context, node, (items, index), &lines),
        max => max as usize,
    };
    if count == 0 || lines.len() < count {
        return Ok(());
    }
    let kept = &lines[count - 1];
    let cut = kept.iter().map(|run| run.bottom).fold(f64::MIN, f64::max);
    if style.max_lines == 0
        && let Some(end) = box_clamp(context, node, items, index, cut)
    {
        hide_after(context, items, index, end);
        inner.bottom = enclosing_end(context, node, items, kept, end);
        return Ok(());
    }
    // A block after the clamp line clamps even when it holds no lines.
    let Some(next) = lines.get(count).map(|line| line[0].top).or_else(|| {
        let mut after = items[index + 1..].iter();
        after
            .any(|item| block_after(context, item, cut))
            .then_some(cut)
    }) else {
        return Ok(());
    };
    // `line-clamp: auto` puts the clamp point after whatever fits, which may be a block.
    let auto = style.max_lines == 0;
    let after_block = auto && block_between(context, node, &items[index + 1..], (cut, next));
    hide_after(context, items, index, cut.min(next));
    if let (Some(marker), Some(last), false) = (
        style
            .block_ellipsis
            .as_deref()
            .filter(|marker| !marker.is_empty()),
        kept.iter()
            .max_by(|a, b| right(items, a).total_cmp(&right(items, b))),
        after_block,
    ) {
        place_marker(context, node, items, (index, kept, *last), marker)?;
    }
    inner.bottom = enclosing_end(context, node, items, kept, cut);
    Ok(())
}

/** Appends `block-ellipsis` after the clamp line's last run. The marker is styled by the line's
root inline box, so it is its own run on the item of the block holding the line, baseline-aligned
with the cut run, and every inline box around that run closes before it (its continuation is
clamped away). */
fn place_marker(
    context: &mut LayoutContext<'_>,
    node: &Node,
    items: &mut [DisplayItem],
    (index, kept, last): (usize, &[Run], Run),
    marker: &str,
) -> EngineResult<()> {
    let block = line_block(context, items[last.item].local_id)?;
    let face = super::sizing::face_for(&block.style, Some(marker));
    let width = context.measure(marker, face)?;
    let mut from = context.node(items[last.item].local_id)?;
    if last.line == ATOMIC {
        from = from
            .parent
            .map_or(Ok(from), |parent| context.node(parent))?;
    }
    let closing = closing_edges(context, &block, from.local_id, &items[last.item..]);
    let right = content_right(node, &items[index]);
    let aligned = super::justify::aligned(&block.style).lines;
    let mut runs = text_runs(items, kept);
    let realign = aligned != Align::Left && runs.len() == kept.len();
    if realign {
        let start = content_left(node, &items[index]);
        super::line_clamp_align::unalign(context, items, &runs, start)?;
    }
    let end = cut_line(context, items, kept, right - width - closing)?;
    let y = baseline(context, items, last)? - super::sizing::text_ascent(&block.style);
    let owner = items
        .iter()
        .position(|item| item.local_id == block.local_id);
    let x = end + closing;
    let line = crate::document::PaintLine {
        color: None,
        text: marker.to_string(),
        x,
        y,
        width,
    };
    let owner = owner.unwrap_or(index);
    items[owner].lines.push(line);
    if realign {
        runs.push((owner, items[owner].lines.len() - 1));
        super::line_clamp_align::align(items, &runs, aligned, right);
    }
    Ok(())
}

/** The clamp line's text runs in visual order (atomic boxes left out). */
fn text_runs(items: &[DisplayItem], kept: &[Run]) -> Vec<(usize, usize)> {
    let mut runs: Vec<Run> = kept
        .iter()
        .filter(|run| run.line != ATOMIC)
        .copied()
        .collect();
    runs.sort_by(|a, b| left(items, a).total_cmp(&left(items, b)));
    runs.iter().map(|run| (run.item, run.line)).collect()
}

/** A run's baseline: its text's, or an atomic box's bottom edge (where one with no text of its
own sits). */
fn baseline(context: &LayoutContext<'_>, items: &[DisplayItem], run: Run) -> EngineResult<f64> {
    let item = &items[run.item];
    if run.line == ATOMIC {
        return Ok(item.y + item.height);
    }
    let line_y = item.lines[run.line].y;
    // A run on a `::first-line` takes that line's font for its baseline.
    let style = match super::first_line_layout::run_style(context, items, run.item, line_y) {
        Some(style) => style,
        None => context.node(item.local_id)?.style.clone(),
    };
    Ok(line_y + super::sizing::text_ascent(&style))
}

/** Blink lays the marker out as content: the clamp line breaks again at its last soft wrap
opportunity that leaves room, dropping whole words, across runs (a word split over runs, as by
`::first-letter`, goes whole). With no such opportunity the line keeps nothing. Returns where the
kept content ends. */
fn cut_line(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    kept: &[Run],
    limit: f64,
) -> EngineResult<f64> {
    let mut runs = kept.to_vec();
    runs.sort_by(|a, b| left(items, a).total_cmp(&left(items, b)));
    let start = runs.first().map_or(0.0, |run| left(items, run));
    let mut whole = true;
    for run in runs.iter().rev() {
        if run.line == ATOMIC {
            // An atomic inline is wrapped around on both sides; it stays only if it fits.
            if right(items, run) <= limit {
                return Ok(right(items, run));
            }
            items[run.item].opacity = 0.0;
            whole = true;
            continue;
        }
        if let Some(end) = cut_run(context, items, *run, limit, whole)? {
            return Ok(end);
        }
        whole = line_of(items, run).text.starts_with(char::is_whitespace);
        clear(items, *run);
    }
    Ok(start)
}

/** Keeps the longest part of one run ending at a wrap opportunity by `limit` (the whole run
when `whole` allows it), returning its end; None when nothing of it can stay. */
fn cut_run(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    run: Run,
    limit: f64,
    whole: bool,
) -> EngineResult<Option<f64>> {
    let line = line_of(items, &run).clone();
    // A run on its block's first line was measured in the `::first-line` fonts.
    let style = super::first_line_layout::run_style(context, items, run.item, line.y)
        .or_else(|| {
            context
                .node(items[run.item].local_id)
                .ok()
                .map(|node| node.style.clone())
        })
        .unwrap_or_default();
    let face = super::sizing::face_for(&style, Some(&line.text));
    let wraps = !matches!(style.white_space, WhiteSpace::Nowrap | WhiteSpace::Pre);
    let mut cuts = Vec::new();
    if wraps {
        cuts.extend(line.text.match_indices(' ').map(|(at, _)| at));
    }
    if whole {
        cuts.push(line.text.len());
    }
    for &at in cuts.iter().rev() {
        let text = line.text[..at].trim_end();
        let width = context.measure(text, face)?;
        if !text.is_empty() && line.x + width <= limit {
            let target = &mut items[run.item].lines[run.line];
            (target.text, target.width) = (text.to_string(), width);
            return Ok(Some(line.x + width));
        }
    }
    Ok(None)
}

/** A run's line. */
fn line_of<'a>(items: &'a [DisplayItem], run: &Run) -> &'a crate::document::PaintLine {
    &items[run.item].lines[run.line]
}

/** Empties a run the marker displaced. */
fn clear(items: &mut [DisplayItem], run: Run) {
    let line = &mut items[run.item].lines[run.line];
    line.text.clear();
    line.width = 0.0;
}

/** The block box whose root inline box holds a node's line: its nearest non-inline ancestor. */
fn line_block(context: &LayoutContext<'_>, local: usize) -> EngineResult<Node> {
    let mut node = context.node(local)?;
    while node.style.inline || node.tag == TEXT_TAG {
        match node.parent {
            Some(parent) => node = context.node(parent)?,
            None => break,
        }
    }
    Ok(node.clone())
}

/** The inline-end padding, border and margin of every inline box from the run's node up to the
line's block: those boxes end on the clamp line, before the marker. */
fn closing_edges(
    context: &LayoutContext<'_>,
    block: &Node,
    local: usize,
    after: &[DisplayItem],
) -> f64 {
    let mut edges = 0.0;
    let mut current = context.node(local).ok();
    while let Some(node) = current.filter(|node| node.local_id != block.local_id) {
        if node.style.inline && !continues(context, node.local_id, after) {
            let end = super::sizing::border_sides(&node.style)[1];
            edges += node.style.padding[1] + end + node.style.margin[1];
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    edges
}

/** Whether an item is a block box below the clamp line: content past the clamp point even with
no lines of its own. A phantom line box (an empty inline) is no content. */
fn block_after(context: &LayoutContext<'_>, item: &DisplayItem, cut: f64) -> bool {
    let block = context
        .node(item.local_id)
        .is_ok_and(|node| !node.style.inline);
    block && item.y >= cut - SAME_LINE && item.height > 0.0
}

/** Whether an inline box goes on past the clamp line: the clamp hid some of its content (layout
has set no other opacity yet), so it does not end, or close, before the marker. */
fn continues(context: &LayoutContext<'_>, inline: usize, after: &[DisplayItem]) -> bool {
    after
        .iter()
        .filter(|item| item.opacity == 0.0)
        .any(|item| within(context, item.local_id, inline))
}

/** Whether `local` is `ancestor` or inside it. */
fn within(context: &LayoutContext<'_>, local: usize, ancestor: usize) -> bool {
    let mut current = context.node(local).ok();
    while let Some(node) = current {
        if node.local_id == ancestor {
            return true;
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    false
}

/** Whether a block box of the container's formatting context lies wholly between the clamp
line and the next line: the clamp point then follows that block, not a line, so no ellipsis. */
fn block_between(
    context: &LayoutContext<'_>,
    container: &Node,
    items: &[DisplayItem],
    (cut, next): (f64, f64),
) -> bool {
    items.iter().any(|item| {
        let block = context.node(item.local_id).is_ok_and(|node| {
            !node.style.inline && node.tag != TEXT_TAG && counted(context, container, node.local_id)
        });
        block && item.y >= cut - SAME_LINE && item.y + item.height <= next + SAME_LINE
    })
}

/** Where the clamp container's content ends: after the clamp line and every box between it and
the container, a box with an authored height keeping its whole border box and an auto-height
one ending at the line plus its end padding and border, as Blink ends a clamped fragment. */
fn enclosing_end(
    context: &LayoutContext<'_>,
    container: &Node,
    items: &mut [DisplayItem],
    kept: &[Run],
    line_bottom: f64,
) -> f64 {
    let Some(first) = kept.first() else {
        return line_bottom;
    };
    let mut end = line_bottom;
    let mut current = context.node(items[first.item].local_id).ok();
    while let Some(node) = current {
        if node.local_id == container.local_id {
            break;
        }
        let at = items.iter().position(|item| item.local_id == node.local_id);
        if let (Some(at), false) = (at, node.style.inline) {
            let item = &mut items[at];
            end = if node.style.height.is_some() {
                end.max(item.y + item.height)
            } else {
                let sides = super::sizing::border_sides(&node.style);
                let bottom = end + node.style.padding[2] + sides[2];
                item.height = (bottom - item.y).max(0.0);
                bottom
            };
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
        if let (Some(parent), false) = (current, node.style.inline) {
            end += kept_margin(parent, node);
        }
    }
    end
}

/** A block's bottom margin, kept inside its parent when the parent's bottom padding or border
stops it collapsing through. */
fn kept_margin(parent: &Node, child: &Node) -> f64 {
    let sides = super::sizing::border_sides(&parent.style);
    match parent.style.padding[2] + sides[2] > 0.0 {
        true => child.style.margin[2],
        false => 0.0,
    }
}

/** How many line boxes fit in the box's authored height or max-height, the clamp point of
`line-clamp: auto`; all of them when neither bounds it. */
fn fitting(
    context: &LayoutContext<'_>,
    node: &Node,
    (items, index): (&[DisplayItem], usize),
    lines: &[Vec<Run>],
) -> usize {
    let Some(limit) = auto_limit(node, &items[index]) else {
        return lines.len();
    };
    // A line fits with the end edges of the boxes it would close: their padding, border and
    // margin must fit the room too.
    let fits = |line: &&Vec<Run>| {
        let closing = trailing_edges(context, node, items[line[0].item].local_id);
        line.iter().all(|run| run.bottom + closing <= limit)
    };
    lines.iter().take_while(fits).count()
}

/** The block-end padding, border and margin of every block between a node and the clamp
container, which a clamp at that node's line keeps below it. */
fn trailing_edges(context: &LayoutContext<'_>, container: &Node, local: usize) -> f64 {
    let mut edges = 0.0;
    let mut current = context.node(local).ok();
    while let Some(node) = current.filter(|node| node.local_id != container.local_id) {
        if !node.style.inline && node.tag != TEXT_TAG {
            let border = super::sizing::border_sides(&node.style)[2];
            edges += node.style.padding[2] + border + node.style.margin[2];
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    edges
}

/** Where `line-clamp: auto`'s room ends: the content edge its authored height or max-height
puts, in document coordinates; None when neither bounds it. */
fn auto_limit(node: &Node, item: &DisplayItem) -> Option<f64> {
    let style = &node.style;
    let bound = style
        .height
        .or(style.max_height)
        .and_then(|extent| extent.resolve_size(None))?;
    let edges = super::sizing::border_sides(style)[0] + style.padding[0];
    let content = if style.box_sizing == crate::document::BoxSizing::BorderBox {
        (bound - super::flow::vertical_edges(node)).max(0.0)
    } else {
        bound
    };
    Some(item.y + edges + content + super::line::LINE_EPSILON)
}

/** `line-clamp: auto` can clamp between two independent formatting contexts: the bottom of the
lowest new-formatting-context box below the last fitting line (`cut`) that still fits the
room, when content follows it. The clamp point is then no line, so it gets no ellipsis. */
fn box_clamp(
    context: &LayoutContext<'_>,
    node: &Node,
    items: &[DisplayItem],
    index: usize,
    cut: f64,
) -> Option<f64> {
    let limit = auto_limit(node, &items[index])?;
    let fits = |item: &&DisplayItem| {
        let child = context.node(item.local_id).ok();
        let fresh = child.is_some_and(|child| {
            // Floats make formatting contexts too, but are no clamp points.
            let float = child.style.float != crate::document::style::Float::None;
            super::flow::new_context(child)
                && !child.style.inline
                && !float
                && !super::flow::out_of_flow(child)
        });
        fresh && item.y >= cut - SAME_LINE && item.y + item.height <= limit
    };
    let end = items[index + 1..]
        .iter()
        .filter(fits)
        .map(|item| item.y + item.height)
        .fold(f64::MIN, f64::max);
    let after = items[index + 1..].iter().any(|item| {
        item.y >= end - SAME_LINE && (block_after(context, item, end) || !item.lines.is_empty())
    });
    (end > cut + SAME_LINE && after).then_some(end)
}

/** The text runs of the clamp container's own formatting context, in item order. */
fn runs(
    context: &LayoutContext<'_>,
    node: &Node,
    items: &[DisplayItem],
    index: usize,
) -> EngineResult<Vec<Run>> {
    let mut runs = Vec::new();
    for (at, item) in items.iter().enumerate().skip(index) {
        if let Some(run) = atomic_run(context, node, item, at) {
            runs.push(run);
            continue;
        }
        if !counted(context, node, item.local_id) {
            continue;
        }
        let advance = item.line_height.unwrap_or(0.0);
        let ascent = context
            .node(item.local_id)
            .map_or(0.0, |node| super::sizing::text_ascent(&node.style));
        for (line, run) in item.lines.iter().enumerate() {
            if !run.text.trim().is_empty() {
                runs.push(Run {
                    top: run.y,
                    bottom: run.y + advance,
                    base: run.y + ascent,
                    item: at,
                    line,
                });
            }
        }
    }
    Ok(runs)
}

/** An atomic inline sitting on one of the container's own lines, as a run covering its box. */
fn atomic_run(
    context: &LayoutContext<'_>,
    container: &Node,
    item: &DisplayItem,
    at: usize,
) -> Option<Run> {
    let node = context.node(item.local_id).ok()?;
    let replaced = node.style.inline && REPLACED.contains(&node.tag.as_str());
    let inside = node
        .parent
        .is_some_and(|parent| counted(context, container, parent));
    (node.local_id != container.local_id && (node.style.inline_block || replaced) && inside)
        .then_some(Run {
            top: item.y,
            bottom: item.y + item.height,
            base: item.y + item.height,
            item: at,
            line: ATOMIC,
        })
}

/** Replaced elements that sit on a line as atomic inlines. */
const REPLACED: [&str; 7] = [
    "img", "svg", "input", "button", "select", "textarea", "video",
];

/** Whether an item's lines are in the clamp container's formatting context: no box between
it and the container starts a new one. */
fn counted(context: &LayoutContext<'_>, container: &Node, local: usize) -> bool {
    let mut current = context.node(local).ok();
    while let Some(node) = current {
        if node.local_id == container.local_id {
            return true;
        }
        if super::flow::new_context(node) {
            return false;
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    // Generated boxes have no document node; they belong to the box that made them.
    true
}

/** Runs grouped into line boxes, top to bottom. */
fn line_boxes(mut runs: Vec<Run>) -> Vec<Vec<Run>> {
    runs.sort_by(|a, b| a.top.total_cmp(&b.top));
    let mut lines: Vec<Vec<Run>> = Vec::new();
    for run in runs {
        match lines.last_mut() {
            Some(line) if same_line(&line[0], &run) => line.push(run),
            _ => lines.push(vec![run]),
        }
    }
    lines
}

/** Whether a run (sorted after `first`) is on `first`'s line box: text runs start together or
share a baseline (a bigger font starts higher);
with an atomic box, whose top sits off the line's, the two must overlap (a zero-height line
holds nothing that starts below it). */
fn same_line(first: &Run, run: &Run) -> bool {
    match first.line == ATOMIC || run.line == ATOMIC {
        true => run.top < first.bottom - SAME_LINE,
        false => run.top < first.top + SAME_LINE || (run.base - first.base).abs() < SAME_LINE,
    }
}

/** Hides every line and box of the container from the first clamped line box down. */
fn hide_after(context: &LayoutContext<'_>, items: &mut [DisplayItem], index: usize, from: f64) {
    for item in items.iter_mut().skip(index + 1) {
        if content_top(context, item) >= from {
            item.opacity = 0.0;
        }
        for run in item.lines.iter_mut().filter(|run| run.y >= from) {
            run.text.clear();
            run.width = 0.0;
        }
    }
    for run in items[index].lines.iter_mut().filter(|run| run.y >= from) {
        run.text.clear();
        run.width = 0.0;
    }
}

/** Where an item's content starts: an inline box's fragment opens with its border and padding
above the line it sits on. */
fn content_top(context: &LayoutContext<'_>, item: &DisplayItem) -> f64 {
    match context.node(item.local_id) {
        Ok(node) if node.style.inline => {
            item.y + node.style.padding[0] + super::sizing::border_sides(&node.style)[0]
        }
        _ => item.y,
    }
}

/** A run's right edge. */
fn right(items: &[DisplayItem], run: &Run) -> f64 {
    left(items, run) + extent(items, run)
}

/** A run's left edge. */
fn left(items: &[DisplayItem], run: &Run) -> f64 {
    match run.line {
        ATOMIC => items[run.item].x,
        _ => line_of(items, run).x,
    }
}

/** A run's width. */
fn extent(items: &[DisplayItem], run: &Run) -> f64 {
    match run.line {
        ATOMIC => items[run.item].width,
        _ => line_of(items, run).width,
    }
}

/** The container's content-box left edge, where an unaligned line starts. */
fn content_left(node: &Node, item: &DisplayItem) -> f64 {
    let sides = super::sizing::border_sides(&node.style);
    item.x + node.style.padding[3] + sides[3]
}

/** The container's content-box right edge, where the ellipsis must end. */
fn content_right(node: &Node, item: &DisplayItem) -> f64 {
    let sides = super::sizing::border_sides(&node.style);
    item.x + item.width - node.style.padding[1] - sides[1]
}
