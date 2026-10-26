/**
 * The clamp line's alignment around its `block-ellipsis`: Blink places the marker before it
 * aligns the line, so a right-aligned, centred or justified clamp line is first laid back to
 * its natural start-aligned widths, cut and given its marker, then aligned with the marker as
 * its last content (webkit-line-clamp-051, -052, -053).
 */
use super::line::Align;
use super::sizing::LayoutContext;
use crate::document::DisplayItem;
use crate::protocol::EngineResult;

/** One text run of the clamp line: its item index and its index in that item's lines. */
pub(super) type TextRun = (usize, usize);

/** Lays the line's runs back to back from `start` at their shaped widths, undoing alignment
and justification. */
pub(super) fn unalign(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    runs: &[TextRun],
    start: f64,
) -> EngineResult<()> {
    let mut x = start;
    for &(item, line) in runs {
        let style = context.node(items[item].local_id)?.style.clone();
        let text = items[item].lines[line].text.clone();
        let width = context.measure(&text, super::sizing::face_for(&style, Some(&text)))?;
        let run = &mut items[item].lines[line];
        (run.x, run.width) = (x, width);
        x += width;
    }
    Ok(())
}

/** Aligns the cut line and its marker (the last of `runs`) within the room ending at `right`. */
pub(super) fn align(items: &mut [DisplayItem], runs: &[TextRun], align: Align, right: f64) {
    let Some(&(item, line)) = runs.last() else {
        return;
    };
    let end = items[item].lines[line].x + items[item].lines[line].width;
    let free = (right - end).max(0.0);
    match align {
        Align::Left => {}
        Align::Center => shift(items, runs, free / 2.0),
        Align::Right => shift(items, runs, free),
        Align::Justify(spread) => justify(items, runs, free, spread.right),
    }
}

/** Moves every run `dx` along. */
fn shift(items: &mut [DisplayItem], runs: &[TextRun], dx: f64) {
    for &(item, line) in runs {
        items[item].lines[line].x += dx;
    }
}

/** Spreads `free` evenly over the runs that end at a word separator (each word is its own run
on a justified line), widening those and moving the rest; a line with none aligns at its start. */
fn justify(items: &mut [DisplayItem], runs: &[TextRun], free: f64, right: bool) {
    let gaps = runs[..runs.len() - 1]
        .iter()
        .filter(|&&(item, line)| items[item].lines[line].text.ends_with(' '))
        .count();
    if gaps == 0 {
        return shift(items, runs, if right { free } else { 0.0 });
    }
    let each = free / gaps as f64;
    let mut moved = 0.0;
    for &(item, line) in runs {
        let run = &mut items[item].lines[line];
        run.x += moved;
        if run.text.ends_with(' ') {
            run.width += each;
            moved += each;
        }
    }
}
