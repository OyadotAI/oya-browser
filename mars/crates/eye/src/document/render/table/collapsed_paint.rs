/** Collapsed borders as Blink's CollapsedBorderPainter draws them: each edge at its whole width,
 * centered on its grid line, as a plain band with square ends, and each joint where edges meet
 * painted once, in the color of the cell above and to the left of it (the first in grid order).
 * A cell paints its top and bottom edges across their right joints (and, in the first column,
 * the table's outer joints), and its side edges between joints.
 * Mitered rings mixed the colors of four cells at every joint (border-conflict-element-001d).
 *
 * ponytail: a joint is as wide as the owning cell's own edges, not the widest edge meeting
 * there; spanning and rtl cells take the same rule by their first row and column. */
use super::super::flow::*;

/** Where a cell sits: (first column, rtl table, whose first column is at the right and whose
 * joints go to the cell above and to the right). */
pub(super) type Leading = (bool, bool);

/** The cell's edge bands (whole widths) in place of its half borders, which its own box drops.
 * Empty when the cell has no borders. */
pub(super) fn edges(
    cell: &Node,
    leading: Leading,
    start: usize,
    items: &mut [DisplayItem],
) -> Vec<DisplayItem> {
    let Some(own) = items.get_mut(start) else {
        return Vec::new();
    };
    let half = super::super::sizing::border_sides(&cell.style);
    if own.local_id != cell.local_id || half.iter().all(|width| *width <= 0.0) {
        return Vec::new();
    }
    own.border_widths = [0.0; 4];
    let own = own.clone();
    let mut out: Vec<DisplayItem> = sided(&own, half, leading)
        .into_iter()
        .filter(|(side, _)| half[*side] > 0.0)
        .map(|(side, rect)| band(&own, side, rect))
        .collect();
    if !cell.style.shadows.is_empty() {
        out.push(shadow_ring(&own, half));
    }
    out
}
/** The bands, mirrored for an rtl table. */
fn sided(own: &DisplayItem, half: [f64; 4], (column, rtl): Leading) -> Vec<(usize, [f64; 4])> {
    if !rtl {
        return bands(own, half, column);
    }
    let mut mirrored = own.clone();
    mirrored.x = -(own.x + own.width);
    let flipped = [half[0], half[3], half[2], half[1]];
    let swap = |side: usize| match side {
        1 => 3,
        3 => 1,
        side => side,
    };
    bands(&mirrored, flipped, column)
        .into_iter()
        .map(|(side, [l, t, r, b])| (swap(side), [-r, t, -l, b]))
        .collect()
}
/** A cell's inset shadows need its whole-width ring as their padding edge: a ring item in the
 * cell's place with transparent borders, which effects decorates with the cell's shadows
 * (box-shadow-001). */
fn shadow_ring(own: &DisplayItem, half: [f64; 4]) -> DisplayItem {
    const CLEAR: &str = "transparent";
    let mut ring = own.clone();
    ring.background = None;
    ring.backgrounds = Vec::new();
    ring.lines = Vec::new();
    ring.vector = Vec::new();
    ring.validation = Vec::new();
    ring.x -= half[3];
    ring.y -= half[0];
    ring.width += half[1] + half[3];
    ring.height += half[0] + half[2];
    ring.border_widths = half.map(|width| super::super::sizing::snap_border(width * 2.0));
    ring.border_colors = std::array::from_fn(|_| Some(CLEAR.into()));
    ring
}
/** Each side's band as (side, [left, top, right, bottom]), whole width, joints owned as the
 * module says. */
fn bands(own: &DisplayItem, half: [f64; 4], first_column: bool) -> Vec<(usize, [f64; 4])> {
    let (left, top) = (own.x, own.y);
    let (right, bottom) = (own.x + own.width, own.y + own.height);
    let start_x = if first_column {
        left - half[3]
    } else {
        left + half[3]
    };
    let mut out = vec![
        (
            2,
            [start_x, bottom - half[2], right + half[1], bottom + half[2]],
        ),
        (
            3,
            [
                left - half[3],
                top + half[0],
                left + half[3],
                bottom - half[2],
            ],
        ),
        (
            1,
            [
                right - half[1],
                top + half[0],
                right + half[1],
                bottom - half[2],
            ],
        ),
    ];
    // The top edge: the same edge the row above's bottom edge paints, in the same winner's
    // color, so it may cover its right joint too; it matters below an empty row, where nothing
    // else paints it (border-collapse-empty-row).
    out.push((0, [start_x, top - half[0], right + half[1], top + half[0]]));
    out
}
/** One band as an item of the cell: a box exactly the band, whose one border is the band. */
fn band(own: &DisplayItem, side: usize, [left, top, right, bottom]: [f64; 4]) -> DisplayItem {
    let mut item = own.clone();
    item.bare = true;
    item.border = 0.0;
    item.background = None;
    item.backgrounds = Vec::new();
    item.shadows = Vec::new();
    item.lines = Vec::new();
    item.vector = Vec::new();
    item.validation = Vec::new();
    item.radius = [0.0; 4];
    item.x = left;
    item.y = top;
    item.width = (right - left).max(0.0);
    item.height = (bottom - top).max(0.0);
    let mut widths = [0.0; 4];
    widths[side] = super::super::sizing::snap_border(if side.is_multiple_of(2) {
        item.height
    } else {
        item.width
    });
    item.border_widths = widths;
    item
}
