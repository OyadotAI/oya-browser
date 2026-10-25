/** Inline text and native shaping share the same containing content box. */
use super::flow::*;
use super::line::{Frag, Piece};
/** Inline descendants preserve their own paint and exact hit geometry. */
pub(super) fn inline(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    if node.style.inline_block {
        return atomic(context, node, flow, items);
    }
    let index = items.len();
    let y = flow.y;
    let sides = super::sizing::border_sides(&node.style);
    // Inline start and end edges take room on the line; vertical ones paint outside it.
    // A space pending from before the box (the previous run's, or source whitespace between
    // elements) stays outside it, so the box starts after it, as Blink attributes spaces.
    let outside = flow.cursor > 0.0 && (flow.space == Some(true) || node.space_leading);
    let before = if outside {
        context.measure(" ", super::sizing::face_for(&node.style, Some(" ")))?
    } else {
        0.0
    };
    flow.cursor += node.style.margin[3] + node.style.padding[3] + sides[3];
    let x = flow.left() + flow.cursor + before;
    if node.space_leading {
        flow.space = Some(true);
    }
    items.push(box_item(node, x, y, 0.0, 0.0, Vec::new()));
    super::generated::generated(context, node, flow, items, false)?;
    let lines = text(context, &node.text, node, flow, index)?;
    items[index].lines = lines;
    if !super::generated::is_generated(node) {
        for child in context.all_children(node.local_id) {
            node_layout(context, child, flow, items, None)?;
        }
    }
    super::generated::generated(context, node, flow, items, true)?;
    flow.cursor += node.style.margin[1] + node.style.padding[1] + sides[1];
    let descendants = &items[index..];
    let left = descendants
        .iter()
        .flat_map(|i| i.lines.iter())
        .map(|line| line.x)
        .fold(x, f64::min);
    let right = descendants
        .iter()
        .flat_map(|i| i.lines.iter())
        .map(|line| line.x + line.width)
        .fold(flow.left() + flow.cursor, f64::max);
    record_edges(
        context,
        node,
        &items[index..],
        (x, y),
        flow.left() + flow.cursor,
    );
    items[index].x = left;
    items[index].width = (right - left).max(0.0);
    items[index].height = (flow.y + flow.line - y).max(super::sizing::advance(&node.style));
    Ok(())
}
/** How far the box's content start sits before its first run on its first line, and its end
 * after its last run on the last line, so `refit_inlines` keeps child margins and the box's
 * own leading and trailing spaces inside it, as Blink's inline box fragments do. */
fn record_edges(
    context: &mut LayoutContext<'_>,
    node: &Node,
    descendants: &[DisplayItem],
    (start, top): (f64, f64),
    end: f64,
) {
    let sides = super::sizing::border_sides(&node.style);
    let end = end - node.style.margin[1] - node.style.padding[1] - sides[1];
    let runs = || descendants.iter().flat_map(|item| item.lines.iter());
    let first = runs()
        .filter(|line| line.y == top)
        .map(|line| line.x)
        .reduce(f64::min);
    let bottom = runs().map(|line| line.y).reduce(f64::max);
    let last = runs()
        .filter(|line| Some(line.y) == bottom)
        .map(|line| line.x + line.width)
        .reduce(f64::max);
    let lead = first.map_or(0.0, |first| (first - start).max(0.0));
    let trail = last.map_or(0.0, |last| (end - last).max(0.0));
    context.record_inline_edges(node.local_id, (lead, trail));
}
/** An inline-block, inline-flex or inline-grid: laid out as a block, then placed on the line as one unit. */
fn atomic(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    let style = &node.style;
    let margins = (style.margin[3], style.margin[1]);
    let width = super::float::shrink_width(context, node, flow.width, margins.0 + margins.1)?;
    let outer = width + margins.0 + margins.1;
    if flow.cursor > 0.0 && !flow.fits(outer) {
        flow.newline();
    }
    flow.fit(outer);
    let bounds = BorderBox {
        x: flow.left() + flow.cursor + margins.0,
        y: flow.y + style.margin[0],
        width,
        height: None,
    };
    let start = items.len();
    let mut inner = Flow::new(bounds.x, bounds.y, width);
    inner.definite = flow.definite;
    node_layout(context, node, &mut inner, items, Some(bounds))?;
    let height = items.get(start).map_or(0.0, |item| item.height);
    let margin_box = style.margin[0] + height + style.margin[2];
    let ascent = if style.overflow == crate::document::Overflow::Visible {
        last_baseline(context, &items[start..])?
            .map(|baseline| baseline - flow.y)
            .unwrap_or(margin_box)
    } else {
        margin_box
    };
    place_atomic(
        flow,
        (start, items.len()),
        outer,
        (ascent, margin_box),
        style,
        margins,
    );
    Ok(())
}
/** Records an atomic fragment and advances the line past its margin box. */
pub(super) fn place_atomic(
    flow: &mut Flow,
    (start, end): (usize, usize),
    outer: f64,
    (ascent, height): (f64, f64),
    style: &crate::document::ComputedStyle,
    margins: (f64, f64),
) {
    flow.push(Frag {
        piece: Piece::Atomic { start, end },
        ascent,
        height,
        align: style.vertical_align,
        shift: super::sizing::baseline_shift(style),
        margin: margins,
    });
    flow.cursor += outer;
    flow.space = None;
    flow.line = flow.line.max(height);
    flow.bottom = flow.bottom.max(flow.y + flow.line);
}
/** The baseline of the last line box inside an atomic inline's items, in document coordinates. */
fn last_baseline(
    context: &mut LayoutContext<'_>,
    items: &[DisplayItem],
) -> EngineResult<Option<f64>> {
    for item in items.iter().rev() {
        context.charge(1)?;
        if let Some(line) = item.lines.iter().max_by(|a, b| a.y.total_cmp(&b.y)) {
            let node = context.node(item.local_id)?;
            return Ok(Some(line.y + super::sizing::text_ascent(&node.style)));
        }
    }
    Ok(None)
}
/** A text fragment for the run at `line` of the item at `item`, with its effective vertical alignment. */
fn text_frag(context: &LayoutContext<'_>, node: &Node, item: usize, line: usize) -> Frag {
    Frag {
        piece: Piece::Text { item, line },
        ascent: super::sizing::text_ascent(&node.style),
        height: super::sizing::advance(&node.style),
        align: effective_align(context, node),
        shift: inherited_shift(context, node),
        margin: (0.0, 0.0),
    }
}
/** A run sits on its inline boxes' baselines, so every `vertical-align` length between it and
 * its block adds up. */
fn inherited_shift(context: &LayoutContext<'_>, node: &Node) -> f64 {
    let mut shift = 0.0;
    let mut current = Some(node);
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some(node) = current.filter(|node| node.style.inline && !node.style.inline_block)
        else {
            break;
        };
        shift += super::sizing::baseline_shift(&node.style);
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    shift
}
/** vertical-align applies to an inline box and everything inside it, so the nearest non-baseline inline ancestor wins. */
fn effective_align(
    context: &LayoutContext<'_>,
    node: &Node,
) -> crate::document::style::VerticalAlign {
    use crate::document::style::VerticalAlign;
    let mut current = Some(node);
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some(node) = current else {
            break;
        };
        if node.style.vertical_align != VerticalAlign::Baseline || !node.style.inline {
            return if node.style.inline {
                node.style.vertical_align
            } else {
                VerticalAlign::Baseline
            };
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    VerticalAlign::Baseline
}
/** Wrapping measures exact runs; whitespace collapses across adjacent inline elements unless preserved. */
pub(super) fn text(
    context: &mut LayoutContext<'_>,
    source: &str,
    node: &Node,
    flow: &mut Flow,
    item: usize,
) -> EngineResult<Vec<PaintLine>> {
    use crate::document::WhiteSpace;
    context.charge(source.chars().count())?;
    let source = transformed(&node.style, source);
    let space = node.style.white_space;
    let mut lines: Vec<PaintLine> = Vec::new();
    if !matches!(
        space,
        WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine
    ) {
        words(context, &source, node, flow, (item, &mut lines))?;
        return Ok(lines);
    }
    for (index, segment) in source.split('\n').enumerate() {
        if index > 0 {
            flow.line = flow.line.max(super::sizing::advance(&node.style));
            flow.newline();
        }
        if space == WhiteSpace::Pre {
            preserved(context, segment, node, flow, (item, &mut lines))?;
        } else {
            // ponytail: pre-wrap collapses runs of spaces inside a line like pre-line; keep them when it matters.
            words(context, segment, node, flow, (item, &mut lines))?;
        }
    }
    Ok(lines)
}
/** One unwrapped run keeps every space; tabs advance as eight spaces. */
fn preserved(
    context: &mut LayoutContext<'_>,
    segment: &str,
    node: &Node,
    flow: &mut Flow,
    (item, lines): (usize, &mut Vec<PaintLine>),
) -> EngineResult<()> {
    let segment = segment.replace('\t', "        ");
    // An empty run opens no line box of its own, so a trailing newline adds no blank line at the
    // end of a `<pre>` as in Chromium; a blank line inside still advances through `newline`.
    if segment.is_empty() {
        return Ok(());
    }
    let advance = super::sizing::advance(&node.style);
    flow.line = flow.line.max(advance);
    flow.bottom = flow.bottom.max(flow.y + flow.line);
    let width = context.measure(
        &segment,
        super::sizing::face_for(&node.style, Some(&segment)),
    )?;
    let frag = text_frag(context, node, item, lines.len());
    flow.push(frag);
    lines.push(PaintLine {
        text: segment,
        x: flow.left() + flow.cursor,
        y: flow.y,
        width,
    });
    flow.cursor += width;
    Ok(())
}
/** Collapsed words share line boxes; nowrap only breaks at forced newlines. */
fn words(
    context: &mut LayoutContext<'_>,
    source: &str,
    node: &Node,
    flow: &mut Flow,
    (item, lines): (usize, &mut Vec<PaintLine>),
) -> EngineResult<()> {
    let face = super::sizing::face_for(&node.style, Some(source));
    let nowrap = node.style.white_space == crate::document::WhiteSpace::Nowrap;
    let space = if source.split_whitespace().next().is_some() {
        context.measure(" ", face)?
    } else {
        0.0
    };
    let breakable = breaks_words(context, node);
    let limit = if nowrap {
        crate::constants::LAYOUT_EXTENT
    } else {
        flow.width.max(1.0)
    };
    // Only source whitespace separates runs: `one<b>two</b>` stays one word.
    let mut spaced =
        source.starts_with(char::is_whitespace) || flow.space.unwrap_or(node.tag != "#text");
    for word in source.split_whitespace() {
        let chunks = super::wrap::lines(word, limit, face, breakable, context)?;
        let count = chunks.len();
        for (index, chunk) in chunks.into_iter().enumerate() {
            let gap = if flow.cursor > 0.0 && spaced {
                space
            } else {
                0.0
            };
            if !nowrap && flow.cursor > 0.0 && !flow.fits(gap + chunk.width) {
                flow.newline();
            }
            if !nowrap {
                flow.fit(chunk.width);
            }
            let gap = if flow.cursor > 0.0 && spaced {
                space
            } else {
                0.0
            };
            let x = flow.left() + flow.cursor + gap;
            if let Some(line) = lines.last_mut().filter(|line| line.y == flow.y) {
                context.charge(chunk.text.chars().count())?;
                if gap > 0.0 {
                    line.text.push(' ');
                }
                line.text.push_str(&chunk.text);
                line.width += gap + chunk.width;
            } else {
                let frag = text_frag(context, node, item, lines.len());
                flow.push(frag);
                lines.push(PaintLine {
                    text: chunk.text,
                    x,
                    y: flow.y,
                    width: chunk.width,
                });
            }
            flow.cursor += gap + chunk.width;
            flow.line = flow.line.max(super::sizing::advance(&node.style));
            flow.bottom = flow.bottom.max(flow.y + flow.line);
            if index + 1 < count {
                flow.newline();
            }
        }
        spaced = true;
    }
    flow.space = if source.split_whitespace().next().is_some() {
        Some(source.ends_with(char::is_whitespace))
    } else if spaced || !source.is_empty() {
        Some(true)
    } else {
        flow.space
    };
    Ok(())
}
/** Whether `overflow-wrap` or `word-break`, inherited from the nearest ancestor that declares
it, lets a line break inside a word. */
fn breaks_words(context: &LayoutContext<'_>, node: &Node) -> bool {
    let declared = |pick: fn(&crate::document::ComputedStyle) -> Option<bool>| {
        let mut current = Some(node);
        while let Some(node) = current {
            if let Some(value) = pick(&node.style) {
                return value;
            }
            current = node.parent.and_then(|parent| context.node(parent).ok());
        }
        false
    };
    declared(|style| style.overflow_wrap_break) || declared(|style| style.word_break_all)
}
/** CSS text-transform changes what is measured and painted, never the agent-visible text. */
pub(super) fn transformed<'a>(
    style: &crate::document::ComputedStyle,
    source: &'a str,
) -> std::borrow::Cow<'a, str> {
    use crate::document::TextTransform;
    match style.text_transform {
        TextTransform::None => source.into(),
        TextTransform::Uppercase => source.to_uppercase().into(),
        TextTransform::Lowercase => source.to_lowercase().into(),
        TextTransform::Capitalize => {
            let mut result = String::with_capacity(source.len());
            let mut start = true;
            for c in source.chars() {
                if start && c.is_alphanumeric() {
                    result.extend(c.to_uppercase());
                } else {
                    result.push(c);
                }
                start = c.is_whitespace();
            }
            result.into()
        }
    }
}
