/**
 * `::first-line`: the text on a block's first formatted line takes the pseudo-element's color and
 * fonts. Layout measures that line in those fonts (`first_line_layout.rs`); here its runs move
 * into text-only copies of their items painted in them, so a box spanning several lines keeps
 * one background and border. A clamped block's ellipsis keeps the block's own style.
 */
use super::{DisplayItem, PaintLine};
use crate::document::{Document, Node};

/** Restyles the first formatted line of every block with a `::first-line` style, in place. */
pub(super) fn apply(document: &Document, items: &mut Vec<DisplayItem>) {
    if !document
        .nodes
        .iter()
        .any(|node| node.style.first_line.is_some())
    {
        return;
    }
    let positions = super::sizing::positions(document);
    let mut members: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for (at, item) in items.iter().enumerate() {
        for block in ancestors(document, &positions, item.local_id) {
            if block.style.first_line.is_some() {
                members.entry(block.local_id).or_default().push(at);
            }
        }
    }
    let mut copies: Vec<(usize, DisplayItem)> = Vec::new();
    for (block, members) in members {
        let Some(block) = positions.get(block).copied().flatten() else {
            continue;
        };
        let block = &document.nodes[block];
        let Some(bottom) = first_line_bottom(items, &members) else {
            continue;
        };
        let looks = Looks::of(block);
        for at in members {
            let Some(node) = positions.get(items[at].local_id).copied().flatten() else {
                continue;
            };
            let node = &document.nodes[node];
            let inherits = super::first_line_layout::inherits(node);
            let ellipsis = node.style.block_ellipsis.as_deref();
            let restyled = restyle(&mut items[at], (&looks, inherits, ellipsis), bottom);
            copies.extend(restyled.map(|copy| (at, copy)));
        }
    }
    copies.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    for (at, copy) in copies {
        items.insert(at + 1, copy);
    }
}

/// How a block's text paints with and without its `::first-line` style, as item fields.
struct Looks {
    /// The block's own text's item fields.
    own: DisplayItem,
    /// The same text on the first line.
    line: DisplayItem,
    /// The block's `block-ellipsis` string, which paints in the block's own style.
    ellipsis: Option<String>,
}
impl Looks {
    /** The block's two looks, from stand-in items built as layout builds every item. */
    fn of(block: &Node) -> Self {
        let mut lined = block.clone();
        if let Some(line) = &block.style.first_line {
            lined.style = super::first_line_layout::merged(&block.style, line, &block.style, false);
            lined.style.color.clone_from(&line.color);
        }
        let item = |node: &Node| super::flow::box_item(node, 0.0, 0.0, 0.0, 0.0, Vec::new());
        Self {
            own: item(block),
            line: item(&lined),
            ellipsis: block.style.block_ellipsis.clone(),
        }
    }
    /** `item` in the first-line look: every field, or with `inherits` (an inline element, which
     * may style itself) only the fields where it has the block's own value. */
    fn apply(&self, item: &mut DisplayItem, inherits: bool) {
        let (own, line) = (&self.own, &self.line);
        // A field changes when the item inherits it from the block (`inherits`), or else when
        // the first-line style sets it to something other than the block's.
        let takes = |inherited: bool, set: bool| if inherits { inherited } else { set };
        if takes(item.color == own.color, line.color != own.color) {
            item.color.clone_from(&line.color);
        }
        if takes(
            item.font_size == own.font_size,
            line.font_size != own.font_size,
        ) {
            (item.font_size, item.line_height) = (line.font_size, line.line_height);
        }
        let weight = |i: &DisplayItem| (i.bold, i.font_weight);
        if takes(weight(item) == weight(own), weight(line) != weight(own)) {
            (item.bold, item.font_weight) = weight(line);
        }
        if takes(item.italic == own.italic, line.italic != own.italic) {
            item.italic = line.italic;
        }
        if takes(
            item.font_family == own.font_family,
            line.font_family != own.font_family,
        ) {
            item.font_family.clone_from(&line.font_family);
        }
        let spacing = (
            item.letter_spacing == own.letter_spacing,
            line.letter_spacing != own.letter_spacing,
        );
        if takes(spacing.0, spacing.1) {
            item.letter_spacing = line.letter_spacing;
        }
    }
}

/** The bottom of the first line among the members' runs: the top-most run's top plus its line
 * advance; runs starting above it share that line. */
fn first_line_bottom(items: &[DisplayItem], members: &[usize]) -> Option<f64> {
    members
        .iter()
        .flat_map(|at| {
            let item = &items[*at];
            let advance = item.line_height.unwrap_or(item.font_size);
            item.lines
                .iter()
                .map(move |line| (line.y, line.y + advance))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, bottom)| bottom)
}

/**
 * Moves the item's first-line runs (all but an ellipsis) into the first-line look: the whole item
 * when every run is there, else a text-only copy carrying those runs, returned to paint after it.
 */
fn restyle(
    item: &mut DisplayItem,
    (looks, inherits, own_ellipsis): (&Looks, bool, Option<&str>),
    bottom: f64,
) -> Option<DisplayItem> {
    if item.lines.is_empty() {
        return None;
    }
    // A clamp marker (the item's own block's, or the first-line block's) keeps its own look.
    let markers = [looks.ellipsis.as_deref(), own_ellipsis];
    let ellipsis = |line: &PaintLine| markers.contains(&Some(line.text.as_str()));
    let (first, rest): (Vec<PaintLine>, Vec<PaintLine>) = item
        .lines
        .drain(..)
        .partition(|line| line.y < bottom && !ellipsis(line));
    if rest.is_empty() {
        item.lines = first;
        looks.apply(item, inherits);
        return None;
    }
    item.lines = rest;
    (!first.is_empty()).then(|| {
        let mut copy = text_only(item, first);
        looks.apply(&mut copy, inherits);
        copy
    })
}

/** A copy of `item` that paints only `lines`, with no box of its own. */
fn text_only(item: &DisplayItem, lines: Vec<PaintLine>) -> DisplayItem {
    let mut copy = item.clone();
    copy.lines = lines;
    // The copy paints its own runs, even when its block defers the rest to a text echo.
    copy.text_deferred = false;
    copy.background = None;
    copy.backgrounds.clear();
    copy.border = 0.0;
    copy.border_widths = [0.0; 4];
    copy.shadows.clear();
    copy.mask = None;
    copy.border_image = None;
    copy.vector.clear();
    copy
}

/** Node `local` and its ancestors, nearest first, bounded by the document depth. */
fn ancestors<'a>(
    document: &'a Document,
    positions: &'a [Option<usize>],
    local: usize,
) -> impl Iterator<Item = &'a Node> + 'a {
    let node = |id: usize| {
        positions
            .get(id)
            .copied()
            .flatten()
            .map(|at| &document.nodes[at])
    };
    std::iter::successors(node(local), move |current| current.parent.and_then(node))
        .take(crate::constants::DOCUMENT_DEPTH)
}
