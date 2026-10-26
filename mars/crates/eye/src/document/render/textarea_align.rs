/** `text-align` for a textarea's own text, which control layout lays out outside the block
 * flow and so outside `line::settle`: each line shifts by its free room, the last one by
 * `text-align-last`. Trailing spaces hang, so they never count as content to align. */
use super::flow::*;
use super::line::Align;

/** Shifts `lines` (runs, top to bottom) inside the content box starting at `left`, `width` wide. */
pub(super) fn align(
    lines: &mut [PaintLine],
    style: &crate::document::ComputedStyle,
    (left, width): (f64, f64),
) {
    let aligned = super::flow::alignment(style);
    let tops: Vec<f64> = {
        let mut tops: Vec<f64> = lines.iter().map(|l| l.y).collect();
        tops.dedup();
        tops
    };
    for (index, top) in tops.iter().enumerate() {
        let mode = if index + 1 == tops.len() {
            aligned.last
        } else {
            aligned.lines
        };
        let end = lines
            .iter()
            .filter(|l| l.y == *top)
            .map(|l| l.x + l.width - trailing(l))
            .fold(left, f64::max);
        let free = (width - (end - left)).max(0.0);
        let dx = match mode {
            Align::Center => free / 2.0,
            Align::Right => free,
            Align::Left | Align::Justify(_) => 0.0,
        };
        for line in lines.iter_mut().filter(|l| l.y == *top) {
            line.x += dx;
        }
    }
}

/** The width of a run's trailing spaces, which hang past the line's end: an even share of the
 * run per character, close enough for the monospace and Ahem text it matters for. */
fn trailing(line: &PaintLine) -> f64 {
    let count = line.text.chars().count();
    let spaces = count - line.text.trim_end_matches(' ').chars().count();
    if count == 0 {
        0.0
    } else {
        line.width * spaces as f64 / count as f64
    }
}
