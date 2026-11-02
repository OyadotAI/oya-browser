/** Moves a multicol container's laid-out items into the pieces the column cut makes: an item
inside one piece moves with it; a box crossing pieces is sliced (`box-decoration-break:
slice`), a copy per piece clipped to it and carrying only that piece's lines. */
use super::display::ClipRect;
use super::multicol_balance::EPSILON;
use super::multicol_fragment::Piece;
use super::sizing::LayoutContext;
use crate::document::DisplayItem;

/** How far a slice's clip reaches past its column in the inline axis and at the content's two
ends, since columns cut only in the block axis. */
const CLIP_REACH: f64 = 1.0e5;
/** Moves and slices items into their pieces. */
pub(super) struct Cutter<'a> {
    /// The node tree, for spanner membership.
    context: &'a LayoutContext<'a>,
    /// Every piece, in single-column order.
    pieces: &'a [Piece],
}
impl<'a> Cutter<'a> {
    /** A cutter over `pieces`, in single-column order. */
    pub fn new(context: &'a LayoutContext<'a>, pieces: &'a [Piece]) -> Self {
        Self { context, pieces }
    }
    /** The piece a single-column position falls in. */
    pub fn of(&self, y: f64) -> usize {
        let last = self.pieces.len().saturating_sub(1);
        (0..=last)
            .rev()
            .find(|k| y + EPSILON >= self.pieces[*k].start)
            .unwrap_or(0)
    }
    /** The piece a box ending at `bottom` ends in: the last one starting above it. */
    fn ending(&self, bottom: f64) -> usize {
        (0..self.pieces.len())
            .rev()
            .find(|k| self.pieces[*k].start < bottom - EPSILON)
            .unwrap_or(0)
    }
    /** How far piece k moves its content. */
    pub fn shift(&self, k: usize) -> (f64, f64) {
        let piece = self.pieces[k];
        (piece.dx, piece.top - piece.start)
    }
    /** Piece k's clip in final coordinates, open at the content's two ends and inline. */
    fn clip(&self, k: usize) -> ClipRect {
        let piece = self.pieces[k];
        let y = if k == 0 {
            piece.top - CLIP_REACH
        } else {
            piece.top
        };
        let bottom = if k + 1 == self.pieces.len() {
            piece.top + CLIP_REACH
        } else {
            piece.top + piece.clip
        };
        ClipRect {
            x: -CLIP_REACH,
            y,
            width: CLIP_REACH * 2.0,
            height: bottom - y,
            radius: [0.0; 4],
        }
    }
    /** Whether the item may show in piece k: a spanner piece shows only its spanner's subtree. */
    fn shows(&self, item: &DisplayItem, k: usize) -> bool {
        let Some(spanner) = self.pieces[k].spanner else {
            return true;
        };
        let mut current = self.context.node(item.local_id).ok();
        while let Some(node) = current {
            if node.local_id == spanner {
                return true;
            }
            current = node
                .parent
                .and_then(|parent| self.context.node(parent).ok());
        }
        false
    }
    /** The pieces an item's box and lines reach, first to last. */
    fn reach(&self, item: &DisplayItem) -> (usize, usize) {
        let first = self.of(item.y);
        let last = self.ending(item.y + item.height).max(first);
        item.lines
            .iter()
            .map(|line| self.of(line.y))
            .fold((first, last), |(a, b), k| (a.min(k), b.max(k)))
    }
    /** An item moved into its piece, or sliced into one copy per piece it crosses; one held
    inside a monolithic box (`anchor`, that box's top) moves whole with it. */
    pub fn cut(&self, item: DisplayItem, anchor: Option<f64>) -> Vec<DisplayItem> {
        if let Some(y) = anchor {
            return vec![self.moved(item, self.of(y), None)];
        }
        let (first, last) = self.reach(&item);
        if first == last {
            return vec![self.moved(item, first, None)];
        }
        (first..=last)
            .filter(|k| self.shows(&item, *k))
            .map(|k| {
                let mut copy = item.clone();
                copy.lines.retain(|line| self.of(line.y) == k);
                self.moved(copy, k, Some(self.clip(k)))
            })
            .collect()
    }
    /** Moves an item (and any clip it carries) into piece k, intersecting `clip`. */
    fn moved(&self, mut item: DisplayItem, k: usize, clip: Option<ClipRect>) -> DisplayItem {
        let (dx, dy) = self.shift(k);
        super::line::translate(&mut item, dx, dy);
        let carried = item.clip.map(|mut own| {
            own.x += dx;
            own.y += dy;
            own
        });
        item.clip = match (carried, clip) {
            (Some(own), Some(cut)) => Some(own.intersect(&cut)),
            (own, cut) => own.or(cut),
        };
        item
    }
    /** Moves the container's own lines, without its box, into their pieces. */
    pub fn move_lines(&self, owner: &mut DisplayItem) {
        for line in &mut owner.lines {
            let (dx, dy) = self.shift(self.of(line.y));
            line.x += dx;
            line.y += dy;
        }
    }
}
