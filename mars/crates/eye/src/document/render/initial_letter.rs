/**
 * `initial-letter` boxes (CSS Inline 3 section 7): a first letter sized to span `size` lines,
 * sunk `sink` lines into the paragraph, and excluded from the lines beside it like a float, as
 * Blink's `initial_letter_utils` places it.
 */
use super::flow::*;
use super::sizing::{advance, border_sides, face_for, face_metrics, text_ascent};
use crate::document::{
    ComputedStyle, LineHeight,
    style::{InitialLetter, VerticalAlign},
};

/**
 * Places the initial letter box at the start of the first line and lowers that line by the
 * lines the letter rises above it. The box floats to the line's start side with the block offset
 * of Blink's `ComputeInitialLetterBoxBlockOffset` added to its top margin, so the lines it
 * overlaps are shortened by its margin box.
 */
// ponytail: horizontal writing only, and the box is the letter's em box rather than its ink
// bounds (equal for Ahem); vertical modes and ink-tight boxes come when a page needs them.
pub(super) fn place(
    context: &mut LayoutContext<'_>,
    letter: &Node,
    paragraph: &ComputedStyle,
    initial: InitialLetter,
    (flow, items): (&mut Flow, &mut Vec<DisplayItem>),
) -> EngineResult<()> {
    // An initial letter starts below earlier ones, and its line with it.
    flow.y = flow.y.max(clearance(flow).unwrap_or(flow.y));
    let mut letter = letter.clone();
    size_letter(&mut letter, paragraph, initial);
    let (offset, adjust) = block_offset(&letter.style, paragraph, initial);
    let letter = super::first_letter::fitted(context, &letter)?;
    let (px, percent) = paragraph.text_indent;
    let placement = Placement {
        offset,
        left: !paragraph.rtl,
        indent: px + flow.width * percent / PERCENT,
        line: advance(paragraph),
    };
    exclude(context, &letter, placement, (flow, items))?;
    flow.y += adjust;
    Ok(())
}

/// Where the letter box goes on its first line.
#[derive(Clone, Copy)]
struct Placement {
    /// Border-box offset below the line's top.
    offset: f64,
    /// It starts the line on the left (left-to-right text).
    left: bool,
    /// The paragraph's `text-indent`, which Blink applies to the box twice: the line's start
    /// is indented, and the box sits indented again from it. The first line's own indent then
    /// still follows the box.
    indent: f64,
    /// The paragraph's line advance: the first line's height.
    line: f64,
}

/// Percentages are hundredths.
const PERCENT: f64 = 100.0;

/**
 * Lays the letter out at the start of the current line, `offset` below its top, and excludes
 * its margin box from the lines beside it, as Blink's `CreateExclusionSpaceForInitialLetterBox`.
 * Unlike a float it ignores earlier floats' tops (it sits on the first line wherever that is),
 * and its exclusion starts at the line's top even when it hangs lower.
 */
fn exclude(
    context: &mut LayoutContext<'_>,
    letter: &Node,
    placement: Placement,
    (flow, items): (&mut Flow, &mut Vec<DisplayItem>),
) -> EngineResult<()> {
    let [top, right, bottom, start] = letter.style.margin;
    let width = super::float::shrink_width(context, letter, flow.width, start + right)?;
    let outer = (width + start + right).max(0.0);
    let (line_start, room) = flow.room_at(flow.y);
    let shift = 2.0 * placement.indent;
    let x = if placement.left {
        line_start + shift
    } else {
        line_start + room - outer - shift
    };
    let bounds = BorderBox {
        x: x + start,
        y: flow.y + placement.offset + top,
        width,
        height: None,
    };
    let first = items.len();
    let mut inner = Flow::new(bounds.x, bounds.y, width);
    node_layout(context, letter, &mut inner, items, Some(bounds))?;
    let height = items.get(first).map_or(0.0, |item| item.height);
    let bottom = flow.y + placement.offset + (top + height + bottom).max(0.0);
    let (x0, x1) = if placement.left {
        (line_start, x + outer)
    } else {
        (x, line_start + room)
    };
    let top = flow.y + placement.offset.min(0.0);
    for (top, bottom, x0, x1) in exclusions(placement, (top, bottom), (x0, x1), flow.y) {
        flow.add_float(super::float::FloatBox {
            left: placement.left,
            x0,
            x1,
            top,
            bottom,
            initial: true,
        });
    }
    Ok(())
}

/** The letter's exclusions: its margin box, with the first line's part narrowed by the indent
 * that line still applies after it, so its text lands right beside the box (on its right in
 * left-to-right text, on its left in right-to-left text). */
fn exclusions(
    placement: Placement,
    (top, bottom): (f64, f64),
    (x0, x1): (f64, f64),
    line_top: f64,
) -> Vec<(f64, f64, f64, f64)> {
    let indent = placement.indent;
    if indent == 0.0 {
        return vec![(top, bottom, x0, x1)];
    }
    let first_bottom = (line_top + placement.line).min(bottom);
    let first = if placement.left {
        (top, first_bottom, x0, x1 - indent)
    } else {
        (top, first_bottom, x0 + indent, x1)
    };
    vec![first, (first_bottom, bottom, x0, x1)]
}

/** The bottom of the lowest initial letter exclusion in `flow`, which a later float or initial
 * letter starts below, as Blink's exclusion space places them after initial letters. */
pub(super) fn clearance(flow: &Flow) -> Option<f64> {
    flow.floats
        .iter()
        .filter(|float| float.initial)
        .map(|float| float.bottom)
        .reduce(f64::max)
}

/** Blink's `ComputeInitialLetterFont` and `StyleForInitialLetterText`: the font size whose cap
 * height spans `size - 1` paragraph lines plus the paragraph's cap height, and a line height of
 * exactly the font's height; the authored font size and line height are ignored. */
fn size_letter(letter: &mut Node, paragraph: &ComputedStyle, initial: InitialLetter) {
    let cap =
        |style: &ComputedStyle, text: &str| crate::platform::edges(face_for(style, Some(text))).0;
    let desired = advance(paragraph) * (initial.size - 1.0) + cap(paragraph, &letter.text);
    let own = cap(&letter.style, &letter.text);
    if own > 0.0 {
        letter.style.font_size *= desired / own;
    }
    letter.style.line_height = LineHeight::Length(face_metrics(&letter.style).1);
    letter.style.vertical_align = VerticalAlign::Baseline;
}

/**
 * The box's border-box offset below the first line's top, and how far that line moves down
 * (Blink's `initial_letter_block_start_adjust`). A letter sinking deeper than its size hangs
 * from the sink line's bottom; otherwise its baseline sits on the baseline of line `size`, and
 * the text moves down by the `size - sink` lines it rises.
 */
fn block_offset(
    letter: &ComputedStyle,
    paragraph: &ComputedStyle,
    initial: InitialLetter,
) -> (f64, f64) {
    let line = advance(paragraph);
    let [top, _, bottom, _] = border_sides(letter);
    let (ascent, height) = face_metrics(letter);
    let block = height + top + bottom + letter.padding[0] + letter.padding[2];
    let size = initial.size.ceil();
    if size < initial.sink {
        return (line * initial.sink - block, 0.0);
    }
    // The paragraph's descent below its baseline, half leading included.
    let descent = line - text_ascent(paragraph);
    let baseline = top + letter.padding[0] + ascent;
    (
        line * initial.size - baseline - descent,
        line * (size - initial.sink),
    )
}
