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
    indent_first_line(context, node, flow, false);
    let index = items.len();
    let (y, frags) = (flow.y, flow.frags.len());
    let sides = super::sizing::border_sides(&node.style);
    // Inline start and end edges take room on the line; vertical ones paint outside it.
    // A space pending from before the box (the previous run's, or source whitespace between
    // elements) stays outside it, so the box starts after it, as Blink attributes spaces.
    let preserves = matches!(
        node.style.white_space,
        crate::document::WhiteSpace::Pre | crate::document::WhiteSpace::PreWrap
    );
    let outside = !preserves && started(flow) && (flow.space == Some(true) || node.space_leading);
    let before = if outside {
        context.measure(" ", super::sizing::face_for(&node.style, Some(" ")))?
            + node.style.word_spacing
    } else {
        0.0
    };
    flow.cursor += node.style.margin[3] + node.style.padding[3] + sides[3];
    let x = flow.left() + flow.cursor + before;
    if node.space_leading && !preserves {
        flow.space = Some(true);
    }
    items.push(box_item(node, x, y, 0.0, 0.0, Vec::new()));
    super::generated::generated(context, node, flow, items, false)?;
    let lines = text(context, &own_text(node), node, flow, index)?;
    items[index].lines = lines;
    if !super::generated::is_generated(node) {
        for child in context.all_children(node.local_id) {
            node_layout(context, child, flow, items, None)?;
        }
    }
    super::generated::generated(context, node, flow, items, true)?;
    // Floats inside place nothing on the line, so a box holding only floats is empty too.
    let empty = flow.y == y && flow.frags.len() == frags && items[index].lines.is_empty();
    if empty && opens_line(node, &sides) {
        // An empty run at the box's content start, so the line settles it and the box gets its
        // content area, padding and borders around it like any other inline box.
        let run = PaintLine {
            text: String::new(),
            x: flow.left() + flow.cursor + before,
            y,
            width: 0.0,
        };
        items[index].lines.push(run);
        flow.push(text_frag(context, node, index, 0));
    }
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
    indent_first_line(context, node, flow, false);
    if started(flow) && !flow.fits(outer) {
        flow.wrap();
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
/** Blink's IsInlineBoxStartEmpty and IsInlineBoxEndEmpty: an inline box with an inline-start or
 * inline-end margin, border or padding is not an empty item, so its line is not empty and gets
 * the box's line-height and the strut even when the box holds nothing. An empty `<span>` with a
 * border used to leave its block 0px tall. */
fn opens_line(node: &Node, sides: &[f64; 4]) -> bool {
    let style = &node.style;
    [1, 3]
        .iter()
        .any(|&side| style.margin[side] != 0.0 || style.padding[side] != 0.0 || sides[side] != 0.0)
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
        let Some(node) = current.filter(|node| inline_box(node) && !node.style.inline_block) else {
            break;
        };
        shift += super::sizing::baseline_shift(&node.style);
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    shift
}
/** An inline box in flow. Blink's StyleAdjuster blockifies absolutely positioned and floated
 * boxes, so their own `vertical-align` never shifts the text inside them. */
fn inline_box(node: &Node) -> bool {
    node.style.inline
        && !super::flow::out_of_flow(node)
        && node.style.float == crate::document::style::Float::None
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
        if node.style.vertical_align != VerticalAlign::Baseline || !inline_box(node) {
            return if inline_box(node) {
                node.style.vertical_align
            } else {
                VerticalAlign::Baseline
            };
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    VerticalAlign::Baseline
}
/** A box's own text. The parser drops white-space-only text and marks the box instead;
 * preserved white space (`pre`, `pre-wrap`) keeps it as the start of the box's content, as
 * Blink's InlineItemsBuilder does. */
pub(super) fn own_text(node: &Node) -> std::borrow::Cow<'_, str> {
    use crate::document::WhiteSpace;
    let preserves = matches!(
        node.style.white_space,
        WhiteSpace::Pre | WhiteSpace::PreWrap
    );
    if preserves && !node.leading_space.is_empty() {
        format!("{}{}", node.leading_space, node.text).into()
    } else {
        node.text.as_str().into()
    }
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
    let mut source = transformed(&node.style, source);
    let space = node.style.white_space;
    // A carriage return lays out as a space, as it does where white space collapses
    // (control-chars-00D).
    if matches!(space, WhiteSpace::Pre | WhiteSpace::PreWrap) && source.contains('\r') {
        source = source.replace('\r', " ").into();
    }
    let mut lines: Vec<PaintLine> = Vec::new();
    let preserves = matches!(space, WhiteSpace::Pre | WhiteSpace::PreWrap);
    if item != usize::MAX && (preserves || css_words(&source).next().is_some()) {
        indent_first_line(context, node, flow, true);
    }
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
        if space == WhiteSpace::PreLine {
            words(context, segment, node, flow, (item, &mut lines))?;
        } else {
            preserved(context, segment, node, flow, (item, &mut lines))?;
        }
    }
    Ok(lines)
}
/** One piece of preserved text between break opportunities, after Blink's
 * InlineItemsBuilder: a word (with, under `break-spaces`, the first space after it, since a
 * line only breaks after a space), a run of spaces, or one tab. */
enum Unit<'a> {
    Word(&'a str),
    Spaces(&'a str),
    Tab,
}
/** Splits a preserved segment into its units. Under `break-spaces` every space is its own
 * break opportunity, so spaces come one per unit and the first one sticks to its word.
 * Otherwise a space run also takes in ideographic spaces, which hang at a line's end with
 * them (Blink's IsBreakableSpaceOrOtherSeparator). */
fn units(segment: &str, break_spaces: bool) -> Vec<Unit<'_>> {
    let spacing = |c: char| c == ' ' || (!break_spaces && c == IDEOGRAPHIC_SPACE);
    let mut units = Vec::new();
    let mut rest = segment;
    while let Some(c) = rest.chars().next() {
        let end = match c {
            '\t' => {
                units.push(Unit::Tab);
                rest = &rest[1..];
                continue;
            }
            ' ' if break_spaces => 1,
            c if spacing(c) => rest.find(|c| !spacing(c)).unwrap_or(rest.len()),
            _ => {
                let word = rest.find(|c| c == '\t' || spacing(c)).unwrap_or(rest.len());
                word + usize::from(break_spaces && rest[word..].starts_with(' '))
            }
        };
        let (piece, tail) = rest.split_at(end);
        units.push(if spacing(c) || c == ' ' {
            Unit::Spaces(piece)
        } else {
            Unit::Word(piece)
        });
        rest = tail;
    }
    units
}
/** U+3000, the one space separator besides U+0020 that hangs (Blink's IsOtherSpaceSeparator). */
const IDEOGRAPHIC_SPACE: char = '\u{3000}';
/** CSS Text's tab stops after Blink's Font::TabWidth: a tab advances to the next multiple of
 * `tab-size` (spaces, or a length) from the line's start, skipping a stop nearer than half a
 * space so the tab always shows. The spaces are the block container's, as Blink's
 * InlineNode::FontForTab measures them, so a smaller font inside the block keeps its stops. */
fn tab_width(context: &mut LayoutContext<'_>, node: &Node, position: f64) -> EngineResult<f64> {
    let block = &container(context, node).style;
    let (face, spacing) = (
        super::sizing::face_for(block, Some(" ")),
        block.word_spacing,
    );
    let space = context.measure(" ", face)?;
    let stop = match node.style.tab_size {
        Some((length, true)) => length,
        Some((count, false)) => count * (space + spacing),
        None => TAB_SPACES * (space + spacing),
    };
    if stop <= 0.0 {
        return Ok(0.0);
    }
    let mut distance = stop - position.rem_euclid(stop);
    if distance < space / 2.0 {
        distance += stop;
    }
    Ok(distance)
}
/** The block container whose inline formatting context lays `node` out: the nearest ancestor
 * (or `node` itself) that is not an inline box. */
fn container<'a>(context: &'a LayoutContext<'_>, node: &'a Node) -> &'a Node {
    let mut current = node;
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        if !current.style.inline || current.style.inline_block {
            break;
        }
        let Some(parent) = current.parent.and_then(|id| context.node(id).ok()) else {
            break;
        };
        current = parent;
    }
    current
}
/** The initial `tab-size`, in spaces. */
const TAB_SPACES: f64 = 8.0;
/**
 * A preserved segment (`pre`, `pre-wrap`, `break-spaces`), placed unit by unit. Wrapping modes
 * break only after spaces and tabs. Under `pre-wrap` a space run never moves to the next line:
 * it hangs past the end edge (`Flow::hang`, which the line's alignment leaves out as Blink's
 * LineInfo::HangWidth does); tabs hang the same way. Under `break-spaces` spaces and tabs take
 * room and wrap like letters.
 */
fn preserved(
    context: &mut LayoutContext<'_>,
    segment: &str,
    node: &Node,
    flow: &mut Flow,
    (item, lines): (usize, &mut Vec<PaintLine>),
) -> EngineResult<()> {
    use crate::document::WhiteSpace;
    // An empty run opens no line box of its own, so a trailing newline adds no blank line at the
    // end of a `<pre>` as in Chromium; a blank line inside still advances through `newline`.
    if segment.is_empty() {
        return Ok(());
    }
    let wrap = node.style.white_space == WhiteSpace::PreWrap;
    let break_spaces = wrap && node.style.break_spaces;
    let face = super::sizing::face_for(&node.style, Some(segment));
    let breaks = if wrap {
        breaks(context, node)
    } else {
        Default::default()
    };
    for unit in units(segment, break_spaces) {
        let (text, width) = match unit {
            // Tab stops count from the block's content edge, not from a float's side.
            Unit::Tab => (
                "\t",
                tab_width(context, node, flow.left() - flow.x + flow.cursor)?,
            ),
            Unit::Word(text) | Unit::Spaces(text) => (
                text,
                context.measure(text, face)? + spaces(text) * node.style.word_spacing,
            ),
        };
        if matches!(unit, Unit::Spaces(_) | Unit::Tab) && !break_spaces {
            // Placing a run resets the hang, which spaces after spaces keep adding to.
            let before = flow.hang.width;
            place(context, node, flow, (item, lines), text, width)?;
            if wrap {
                flow.hang.width = before + width;
                flow.hang.rtl = node.style.rtl;
            }
            continue;
        }
        if wrap && started(flow) && !flow.fits(width) {
            if let Unit::Word(word) = unit
                && filled(context, node, flow, (item, lines), word, breaks)?
            {
                continue;
            }
            flow.wrap();
        }
        if wrap {
            flow.fit(width);
        }
        flow.hang = Default::default();
        let Unit::Word(word) = unit else {
            place(context, node, flow, (item, lines), text, width)?;
            continue;
        };
        if !wrap || width <= flow.room() - flow.cursor {
            place(context, node, flow, (item, lines), word, width)?;
            continue;
        }
        broken(context, node, flow, (item, lines), word, breaks)?;
    }
    Ok(())
}
/** A word too wide for its line, broken where Unicode line breaking allows (between
 * ideographs, after an ideographic space) or, with `breakable`, where `overflow-wrap` and
 * `word-break` allow, as Blink's LineBreaker breaks a text item. Under `break-spaces` the word
 * ends with the space after it, which stays on its last piece. */
fn broken(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    (item, lines): (usize, &mut Vec<PaintLine>),
    word: &str,
    breaks: super::wrap::Breaks,
) -> EngineResult<()> {
    let face = super::sizing::face_for(&node.style, Some(word));
    let core = word.strip_suffix(' ').unwrap_or(word);
    let chunks = super::wrap::lines(core, flow.width.max(1.0), face, breaks, context)?;
    let count = chunks.len();
    for (index, chunk) in chunks.into_iter().enumerate() {
        let mut text = chunk.text;
        let mut width = chunk.width + spaces(&text) * node.style.word_spacing;
        if index + 1 == count && core.len() < word.len() {
            text.push(' ');
            width += context.measure(" ", face)? + node.style.word_spacing;
        }
        place(context, node, flow, (item, lines), &text, width)?;
        if index + 1 < count {
            flow.wrap();
        }
    }
    Ok(())
}
/** A preserved word that overflows the current line under a filling break (`word-break:
break-all`, `line-break: anywhere`): its longest head that fits stays on this line and the rest
wraps and breaks as usual, as Blink's LineBreaker breaks a text item at the remaining width.
False when no head fits, so the caller wraps the whole word first. */
fn filled(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    (item, lines): (usize, &mut Vec<PaintLine>),
    word: &str,
    breaks: super::wrap::Breaks,
) -> EngineResult<bool> {
    if !breaks.fill {
        return Ok(false);
    }
    let face = super::sizing::face_for(&node.style, Some(word));
    let room = flow.room() - flow.cursor;
    let Some(head) = super::wrap::head(word, room, face, breaks, context)? else {
        return Ok(false);
    };
    let width = head.width + spaces(&head.text) * node.style.word_spacing;
    place(context, node, flow, (item, lines), &head.text, width)?;
    flow.wrap();
    let rest = &word[head.text.len()..];
    let width = context.measure(rest, face)? + spaces(rest) * node.style.word_spacing;
    flow.fit(width);
    if width <= flow.room() - flow.cursor {
        place(context, node, flow, (item, lines), rest, width)?;
    } else {
        broken(context, node, flow, (item, lines), rest, breaks)?;
    }
    Ok(true)
}
/** Appends a measured piece to the line's run of this text, or starts a new run. */
fn place(
    context: &mut LayoutContext<'_>,
    node: &Node,
    flow: &mut Flow,
    (item, lines): (usize, &mut Vec<PaintLine>),
    text: &str,
    width: f64,
) -> EngineResult<()> {
    context.charge(text.chars().count())?;
    let advance = super::sizing::advance(&node.style);
    flow.line = flow.line.max(advance);
    flow.bottom = flow.bottom.max(flow.y + flow.line);
    // A tab is an empty run of its own as wide as its advance: the painter shapes a run's text
    // as one string, so neither the tab nor the text after it may join a run.
    let tab = text == "\t";
    let text = if tab { "" } else { text };
    match lines
        .last_mut()
        .filter(|line| line.y == flow.y && started(flow) && !tab && !line.text.is_empty())
    {
        Some(line) => {
            line.text.push_str(text);
            line.width += width;
        }
        None => {
            let frag = text_frag(context, node, item, lines.len());
            flow.push(frag);
            lines.push(PaintLine {
                text: text.to_string(),
                x: flow.left() + flow.cursor,
                y: flow.y,
                width,
            });
        }
    }
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
    let space = if css_words(source).next().is_some() {
        context.measure(" ", face)? + node.style.word_spacing
    } else {
        0.0
    };
    let breaks = breaks(context, node);
    let limit = if nowrap {
        crate::constants::LAYOUT_EXTENT
    } else {
        flow.width.max(1.0)
    };
    // Only source whitespace separates runs: `one<b>two</b>` stays one word.
    let mut spaced = source.starts_with(css_space) || flow.space.unwrap_or(node.tag != "#text");
    for word in css_words(source) {
        let room = flow.room() - flow.cursor - if spaced { space } else { 0.0 };
        let chunks = filling(context, word, (limit, room), face, breaks, flow)?;
        let count = chunks.len();
        for (index, mut chunk) in chunks.into_iter().enumerate() {
            // A no-break space does not collapse and takes word-spacing like a space does.
            chunk.width += spaces(&chunk.text) * node.style.word_spacing;
            let gap = if started(flow) && spaced { space } else { 0.0 };
            if !nowrap && started(flow) && !flow.fits(gap + chunk.width) {
                flow.wrap();
            }
            if !nowrap {
                flow.fit(chunk.width);
            }
            let gap = if started(flow) && spaced { space } else { 0.0 };
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
                flow.wrap();
            }
        }
        spaced = true;
    }
    flow.space = if css_words(source).next().is_some() {
        Some(source.ends_with(css_space))
    } else if spaced || !source.is_empty() {
        Some(true)
    } else {
        flow.space
    };
    Ok(())
}
/** A collapsed word's line pieces: under a filling break (`word-break: break-all`, `line-break:
anywhere`) a word that overflows a started line first leaves its longest fitting head there, as
Blink's LineBreaker does; otherwise the pieces of `limit` wide lines. */
fn filling(
    context: &mut LayoutContext<'_>,
    word: &str,
    (limit, room): (f64, f64),
    face: crate::platform::Face,
    breaks: super::wrap::Breaks,
    flow: &Flow,
) -> EngineResult<Vec<super::wrap::MeasuredText>> {
    if breaks.fill
        && started(flow)
        && context.measure(word, face)? > room + FIT_SLACK
        && let Some(head) = super::wrap::head(word, room, face, breaks, context)?
    {
        let rest = &word[head.text.len()..];
        let mut chunks = vec![head];
        chunks.extend(super::wrap::lines(rest, limit, face, breaks, context)?);
        return Ok(chunks);
    }
    super::wrap::lines(word, limit, face, breaks, context)
}
/** Overflow a line forgives, one Chromium 1/64 px layout unit, as `Flow::fits` does. */
const FIT_SLACK: f64 = 1.0 / 64.0;
/** How a text node's words may break, from `overflow-wrap`, `word-break` and `line-break`,
each inherited from the nearest ancestor that declares it. */
fn breaks(context: &LayoutContext<'_>, node: &Node) -> super::wrap::Breaks {
    let anywhere = inherited(context, node, |style| style.line_break_anywhere);
    let fill = anywhere || inherited(context, node, |style| style.word_break_fill);
    super::wrap::Breaks {
        breakable: fill || breaks_words(context, node),
        fill,
        keep_all: inherited(context, node, |style| style.word_break_keep_all),
    }
}
/** A boolean text property's value on `node`: its own declaration or its nearest declaring
ancestor's, false when none declares it. */
fn inherited(
    context: &LayoutContext<'_>,
    node: &Node,
    pick: fn(&crate::document::ComputedStyle) -> Option<bool>,
) -> bool {
    let mut current = Some(node);
    while let Some(node) = current {
        if let Some(value) = pick(&node.style) {
            return value;
        }
        current = node.parent.and_then(|parent| context.node(parent).ok());
    }
    false
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
/** CSS text-transform changes what is measured and painted, never the agent-visible text.
 * Control characters other than the white space ones paint visibly too (CSS Text 3 section 4.1.1
 * says they must not be hidden). */
pub(super) fn transformed<'a>(
    style: &crate::document::ComputedStyle,
    source: &'a str,
) -> std::borrow::Cow<'a, str> {
    let cased = cased(style, source);
    let cased = widened(style, cased);
    if !cased.chars().any(hidden_control) {
        return cased;
    }
    // ponytail: the replacement glyph stands in for Blink's per-font hex box; draw the code
    // point in a box if a reftest ever compares the glyph itself.
    cased
        .chars()
        .map(|c| if hidden_control(c) { '\u{fffd}' } else { c })
        .collect::<String>()
        .into()
}
/** `text-transform: full-width` and `full-size-kana`, after the case mapping as in Blink's
ApplyTextTransform: each scalar with a fullwidth form or a full-size kana paints as that. */
fn widened<'a>(
    style: &crate::document::ComputedStyle,
    text: std::borrow::Cow<'a, str>,
) -> std::borrow::Cow<'a, str> {
    let (wide, kana) = (
        style.text_transform_full_width,
        style.text_transform_full_size_kana,
    );
    if !wide && !kana {
        return text;
    }
    let map = |c: char| {
        let c = if wide { full_width(c) } else { c };
        if kana { lookup(&FULL_SIZE_KANA, c) } else { c }
    };
    text.chars().map(map).collect::<String>().into()
}
/** A scalar's fullwidth form: printable ASCII shifts into U+FF01..U+FF5E, the rest by table. */
fn full_width(c: char) -> char {
    match c {
        '\u{21}'..='\u{7e}' => char::from_u32(c as u32 + FULL_WIDTH_SHIFT).unwrap_or(c),
        _ => lookup(&FULL_WIDTH, c),
    }
}
/** The distance from printable ASCII to its fullwidth forms. */
const FULL_WIDTH_SHIFT: u32 = 0xfee0;
/** `c` mapped through a sorted pair table, or `c` itself. */
fn lookup(table: &[(char, char)], c: char) -> char {
    table
        .binary_search_by_key(&c, |pair| pair.0)
        .map_or(c, |index| table[index].1)
}
/** Blink's Character::FullwidthVariant beyond printable ASCII (U+0021..U+007E, which shifts by U+FEE0): the space, halfwidth katakana and hangul, and the symbols with fullwidth forms. Sorted for binary search. */
const FULL_WIDTH: [(char, char); 132] = [
    ('\u{20}', '\u{3000}'),
    ('\u{a2}', '\u{ffe0}'),
    ('\u{a3}', '\u{ffe1}'),
    ('\u{a5}', '\u{ffe5}'),
    ('\u{a6}', '\u{ffe4}'),
    ('\u{ac}', '\u{ffe2}'),
    ('\u{af}', '\u{ffe3}'),
    ('\u{20a9}', '\u{ffe6}'),
    ('\u{2985}', '\u{ff5f}'),
    ('\u{2986}', '\u{ff60}'),
    ('\u{ff61}', '\u{3002}'),
    ('\u{ff62}', '\u{300c}'),
    ('\u{ff63}', '\u{300d}'),
    ('\u{ff64}', '\u{3001}'),
    ('\u{ff65}', '\u{30fb}'),
    ('\u{ff66}', '\u{30f2}'),
    ('\u{ff67}', '\u{30a1}'),
    ('\u{ff68}', '\u{30a3}'),
    ('\u{ff69}', '\u{30a5}'),
    ('\u{ff6a}', '\u{30a7}'),
    ('\u{ff6b}', '\u{30a9}'),
    ('\u{ff6c}', '\u{30e3}'),
    ('\u{ff6d}', '\u{30e5}'),
    ('\u{ff6e}', '\u{30e7}'),
    ('\u{ff6f}', '\u{30c3}'),
    ('\u{ff70}', '\u{30fc}'),
    ('\u{ff71}', '\u{30a2}'),
    ('\u{ff72}', '\u{30a4}'),
    ('\u{ff73}', '\u{30a6}'),
    ('\u{ff74}', '\u{30a8}'),
    ('\u{ff75}', '\u{30aa}'),
    ('\u{ff76}', '\u{30ab}'),
    ('\u{ff77}', '\u{30ad}'),
    ('\u{ff78}', '\u{30af}'),
    ('\u{ff79}', '\u{30b1}'),
    ('\u{ff7a}', '\u{30b3}'),
    ('\u{ff7b}', '\u{30b5}'),
    ('\u{ff7c}', '\u{30b7}'),
    ('\u{ff7d}', '\u{30b9}'),
    ('\u{ff7e}', '\u{30bb}'),
    ('\u{ff7f}', '\u{30bd}'),
    ('\u{ff80}', '\u{30bf}'),
    ('\u{ff81}', '\u{30c1}'),
    ('\u{ff82}', '\u{30c4}'),
    ('\u{ff83}', '\u{30c6}'),
    ('\u{ff84}', '\u{30c8}'),
    ('\u{ff85}', '\u{30ca}'),
    ('\u{ff86}', '\u{30cb}'),
    ('\u{ff87}', '\u{30cc}'),
    ('\u{ff88}', '\u{30cd}'),
    ('\u{ff89}', '\u{30ce}'),
    ('\u{ff8a}', '\u{30cf}'),
    ('\u{ff8b}', '\u{30d2}'),
    ('\u{ff8c}', '\u{30d5}'),
    ('\u{ff8d}', '\u{30d8}'),
    ('\u{ff8e}', '\u{30db}'),
    ('\u{ff8f}', '\u{30de}'),
    ('\u{ff90}', '\u{30df}'),
    ('\u{ff91}', '\u{30e0}'),
    ('\u{ff92}', '\u{30e1}'),
    ('\u{ff93}', '\u{30e2}'),
    ('\u{ff94}', '\u{30e4}'),
    ('\u{ff95}', '\u{30e6}'),
    ('\u{ff96}', '\u{30e8}'),
    ('\u{ff97}', '\u{30e9}'),
    ('\u{ff98}', '\u{30ea}'),
    ('\u{ff99}', '\u{30eb}'),
    ('\u{ff9a}', '\u{30ec}'),
    ('\u{ff9b}', '\u{30ed}'),
    ('\u{ff9c}', '\u{30ef}'),
    ('\u{ff9d}', '\u{30f3}'),
    ('\u{ff9e}', '\u{3099}'),
    ('\u{ff9f}', '\u{309a}'),
    ('\u{ffa0}', '\u{3164}'),
    ('\u{ffa1}', '\u{3131}'),
    ('\u{ffa2}', '\u{3132}'),
    ('\u{ffa3}', '\u{3133}'),
    ('\u{ffa4}', '\u{3134}'),
    ('\u{ffa5}', '\u{3135}'),
    ('\u{ffa6}', '\u{3136}'),
    ('\u{ffa7}', '\u{3137}'),
    ('\u{ffa8}', '\u{3138}'),
    ('\u{ffa9}', '\u{3139}'),
    ('\u{ffaa}', '\u{313a}'),
    ('\u{ffab}', '\u{313b}'),
    ('\u{ffac}', '\u{313c}'),
    ('\u{ffad}', '\u{313d}'),
    ('\u{ffae}', '\u{313e}'),
    ('\u{ffaf}', '\u{313f}'),
    ('\u{ffb0}', '\u{3140}'),
    ('\u{ffb1}', '\u{3141}'),
    ('\u{ffb2}', '\u{3142}'),
    ('\u{ffb3}', '\u{3143}'),
    ('\u{ffb4}', '\u{3144}'),
    ('\u{ffb5}', '\u{3145}'),
    ('\u{ffb6}', '\u{3146}'),
    ('\u{ffb7}', '\u{3147}'),
    ('\u{ffb8}', '\u{3148}'),
    ('\u{ffb9}', '\u{3149}'),
    ('\u{ffba}', '\u{314a}'),
    ('\u{ffbb}', '\u{314b}'),
    ('\u{ffbc}', '\u{314c}'),
    ('\u{ffbd}', '\u{314d}'),
    ('\u{ffbe}', '\u{314e}'),
    ('\u{ffc2}', '\u{314f}'),
    ('\u{ffc3}', '\u{3150}'),
    ('\u{ffc4}', '\u{3151}'),
    ('\u{ffc5}', '\u{3152}'),
    ('\u{ffc6}', '\u{3153}'),
    ('\u{ffc7}', '\u{3154}'),
    ('\u{ffca}', '\u{3155}'),
    ('\u{ffcb}', '\u{3156}'),
    ('\u{ffcc}', '\u{3157}'),
    ('\u{ffcd}', '\u{3158}'),
    ('\u{ffce}', '\u{3159}'),
    ('\u{ffcf}', '\u{315a}'),
    ('\u{ffd2}', '\u{315b}'),
    ('\u{ffd3}', '\u{315c}'),
    ('\u{ffd4}', '\u{315d}'),
    ('\u{ffd5}', '\u{315e}'),
    ('\u{ffd6}', '\u{315f}'),
    ('\u{ffd7}', '\u{3160}'),
    ('\u{ffda}', '\u{3161}'),
    ('\u{ffdb}', '\u{3162}'),
    ('\u{ffdc}', '\u{3163}'),
    ('\u{ffe8}', '\u{2502}'),
    ('\u{ffe9}', '\u{2190}'),
    ('\u{ffea}', '\u{2191}'),
    ('\u{ffeb}', '\u{2192}'),
    ('\u{ffec}', '\u{2193}'),
    ('\u{ffed}', '\u{25a0}'),
    ('\u{ffee}', '\u{25cb}'),
];
/** Blink's Character::FullSizeKanaVariant: each small kana and its full-size letter. Sorted for binary search. */
const FULL_SIZE_KANA: [(char, char); 58] = [
    ('\u{3041}', '\u{3042}'),
    ('\u{3043}', '\u{3044}'),
    ('\u{3045}', '\u{3046}'),
    ('\u{3047}', '\u{3048}'),
    ('\u{3049}', '\u{304a}'),
    ('\u{3063}', '\u{3064}'),
    ('\u{3083}', '\u{3084}'),
    ('\u{3085}', '\u{3086}'),
    ('\u{3087}', '\u{3088}'),
    ('\u{308e}', '\u{308f}'),
    ('\u{3095}', '\u{304b}'),
    ('\u{3096}', '\u{3051}'),
    ('\u{30a1}', '\u{30a2}'),
    ('\u{30a3}', '\u{30a4}'),
    ('\u{30a5}', '\u{30a6}'),
    ('\u{30a7}', '\u{30a8}'),
    ('\u{30a9}', '\u{30aa}'),
    ('\u{30c3}', '\u{30c4}'),
    ('\u{30e3}', '\u{30e4}'),
    ('\u{30e5}', '\u{30e6}'),
    ('\u{30e7}', '\u{30e8}'),
    ('\u{30ee}', '\u{30ef}'),
    ('\u{30f5}', '\u{30ab}'),
    ('\u{30f6}', '\u{30b1}'),
    ('\u{31f0}', '\u{30af}'),
    ('\u{31f1}', '\u{30b7}'),
    ('\u{31f2}', '\u{30b9}'),
    ('\u{31f3}', '\u{30c8}'),
    ('\u{31f4}', '\u{30cc}'),
    ('\u{31f5}', '\u{30cf}'),
    ('\u{31f6}', '\u{30d2}'),
    ('\u{31f7}', '\u{30d5}'),
    ('\u{31f8}', '\u{30d8}'),
    ('\u{31f9}', '\u{30db}'),
    ('\u{31fa}', '\u{30e0}'),
    ('\u{31fb}', '\u{30e9}'),
    ('\u{31fc}', '\u{30ea}'),
    ('\u{31fd}', '\u{30eb}'),
    ('\u{31fe}', '\u{30ec}'),
    ('\u{31ff}', '\u{30ed}'),
    ('\u{ff67}', '\u{ff71}'),
    ('\u{ff68}', '\u{ff72}'),
    ('\u{ff69}', '\u{ff73}'),
    ('\u{ff6a}', '\u{ff74}'),
    ('\u{ff6b}', '\u{ff75}'),
    ('\u{ff6c}', '\u{ff94}'),
    ('\u{ff6d}', '\u{ff95}'),
    ('\u{ff6e}', '\u{ff96}'),
    ('\u{ff6f}', '\u{ff82}'),
    ('\u{1b132}', '\u{3053}'),
    ('\u{1b150}', '\u{3090}'),
    ('\u{1b151}', '\u{3091}'),
    ('\u{1b152}', '\u{3092}'),
    ('\u{1b155}', '\u{30b3}'),
    ('\u{1b164}', '\u{30f0}'),
    ('\u{1b165}', '\u{30f1}'),
    ('\u{1b166}', '\u{30f2}'),
    ('\u{1b167}', '\u{30f3}'),
];
/** A Unicode Cc character that is not CSS document white space, which a font has no ink for. */
fn hidden_control(c: char) -> bool {
    c.is_control() && !css_space(c)
}
/** The text-transform case mapping. */
fn cased<'a>(style: &crate::document::ComputedStyle, source: &'a str) -> std::borrow::Cow<'a, str> {
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
/** Whether the current line already holds a fragment. Blink removes spaces at the start of a
 * line and never breaks before its first item, whatever indent or inline-start edge precedes it. */
fn started(flow: &Flow) -> bool {
    !flow.frags.is_empty()
}
/** Spaces in a preserved run, each widened by `word-spacing`. */
fn spaces(segment: &str) -> f64 {
    segment
        .chars()
        .filter(|c| *c == ' ' || *c == '\u{a0}')
        .count() as f64
}
/**
 * CSS `text-indent` after Blink's InlineLayoutAlgorithm: the first formatted line of a block
 * container starts `text-indent` (a length, or a percentage of the container's content width)
 * along the inline axis. Only the first line qualifies: no line finished yet in this flow, and
 * no in-flow block-level box before `node` inside its container (a line after a block child
 * is in an anonymous block that is not the first). `own` says `node` itself may be the
 * container (its own text); an atomic inline looks from its parent.
 */
// ponytail: LTR only and not counted in intrinsic widths or replaced first items; add each-line/hanging when needed.
fn indent_first_line(context: &LayoutContext<'_>, node: &Node, flow: &mut Flow, own: bool) {
    if flow.cursor != 0.0 || !flow.frags.is_empty() || !flow.finished.is_empty() {
        return;
    }
    let container_of =
        |n: &Node| (!n.style.inline || n.style.inline_block) && !n.style.display_contents;
    // The path from `node` up to its container, found before any sibling scan so a page
    // without an indent pays only this short walk.
    let mut path = vec![node];
    if !(own && container_of(node)) {
        for _ in 0..crate::constants::DOCUMENT_DEPTH {
            let Some(parent) = path[path.len() - 1]
                .parent
                .and_then(|id| context.node(id).ok())
            else {
                return;
            };
            path.push(parent);
            if container_of(parent) {
                break;
            }
        }
    }
    let container = path[path.len() - 1];
    let (px, percent) = container.style.text_indent;
    if (px, percent) == (0.0, 0.0)
        || path
            .windows(2)
            .any(|pair| block_before(context, pair[1], pair[0]))
    {
        return;
    }
    flow.cursor = px + percent / 100.0 * flow.width;
}
/** Whether an in-flow block-level sibling precedes `child` among `parent`'s children. */
fn block_before(context: &LayoutContext<'_>, parent: &Node, child: &Node) -> bool {
    use crate::document::style::{Float, Position};
    context
        .all_children(parent.local_id)
        .into_iter()
        .take_while(|sibling| sibling.local_id != child.local_id)
        .any(|sibling| {
            !sibling.style.inline
                && !sibling.style.hidden
                && sibling.style.float == Float::None
                && !matches!(sibling.style.position, Position::Absolute | Position::Fixed)
        })
}
/** CSS document white space (space, tab, line feed, carriage return, form feed): the only
 * characters that collapse. A no-break space and other Unicode spaces stay in their word, as in
 * Blink's whitespace collapsing. */
fn css_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c')
}
/** The runs between collapsible white space. */
fn css_words(source: &str) -> impl Iterator<Item = &str> {
    source.split(css_space).filter(|word| !word.is_empty())
}
