/** The document's horizontal scroll range. Eye lays a page out at scroll zero with the
viewport's left edge at x = 0; content past the right edge is reachable by scrolling right in a
left-to-right horizontal document, and content past the left edge (negative x) by scrolling left
in a vertical-rl or right-to-left one, whose initial position is its right edge. Offsets follow
CSSOM View: `scrollLeft` runs from 0 rightwards, or from 0 leftwards into negative values. */
use crate::document::{DisplayItem, DisplayList, Document};

/** Whether the document's scroll origin is its right edge: the root's writing mode is
vertical-rl, or its direction is right to left, the body's standing in for a default root's. */
fn origin_at_right(document: &Document) -> bool {
    let (root, body) = (
        &document.formatting.html.style,
        &document.formatting.body.style,
    );
    let principal = if root.vertical_writing || root.rtl {
        root
    } else {
        body
    };
    if principal.vertical_writing {
        !principal.writing_lr
    } else {
        principal.rtl
    }
}

/** Whether an item takes part in the document's scrollable overflow: painted, not pinned to the
viewport (fixed and sticky boxes move with it), and not clipped by a scroll container. */
fn counts(item: &DisplayItem) -> bool {
    item.width > 0.0 && item.opacity > 0.0 && item.pin.is_none() && item.clip.is_none()
}

/** The range for content spanning `spans` (left, right) in a viewport `width` wide, scrolled from
the right edge when `from_right`. */
fn range(from_right: bool, spans: impl Iterator<Item = (f64, f64)>, width: f64) -> [f64; 2] {
    let (left, right) = spans.fold((0.0_f64, width), |(left, right), (l, r)| {
        (left.min(l), right.max(r))
    });
    if from_right {
        [left.min(0.0), 0.0]
    } else {
        [0.0, (right - width).max(0.0)]
    }
}

/** The `[min, max]` horizontal offsets the document can scroll to, in a viewport `width` wide. */
pub(super) fn scroll_x_range(document: &Document, display: &DisplayList, width: f64) -> [f64; 2] {
    let spans = display
        .items
        .iter()
        .filter(|item| counts(item))
        .map(|item| (item.x, item.x + item.width));
    range(origin_at_right(document), spans, width)
}

#[cfg(test)]
mod tests {
    use super::range;

    /** A left-to-right page scrolls right as far as its widest box reaches past the viewport. */
    #[test]
    fn a_horizontal_page_scrolls_right_to_its_overflow() {
        let spans = [(0.0, 1000.0), (-50.0, 10.0)].into_iter();
        assert_eq!(range(false, spans, 800.0), [0.0, 200.0]);
    }

    /** A vertical-rl page scrolls left into negative offsets, from 0 at its right edge. */
    #[test]
    fn a_vertical_rl_page_scrolls_left_from_zero() {
        let spans = [(-300.0, 800.0), (0.0, 900.0)].into_iter();
        assert_eq!(range(true, spans, 800.0), [-300.0, 0.0]);
    }
}
