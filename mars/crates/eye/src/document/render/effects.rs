/** After layout: relative offsets, transforms, overflow clips, decorations and stacking paint order. */
use super::{
    ClipRect, DisplayBackground, DisplayItem, DisplayShadow, GradientStop, OpacityGroup, Pin,
    ScrollArea, sizing,
};
use crate::{
    document::{BackgroundLayer, ColorStop, Document, Node, Overflow, Position},
    protocol::{EngineError, EngineResult},
};

/** A 2D affine (a, b, c, d, e, f) mapping (x, y) to (ax + cy + e, bx + dy + f). */
type Matrix = [f64; 6];
/** The identity leaves geometry where layout put it. */
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
/** One step of a paint-order key: phase within a painting layer, z-index, document order, then
 * 0 for an element's own box, 1 for its `::before` box and 2 for its `::after` box. */
pub(super) type Step = (u8, i32, usize, u8);
/** A painting layer's own background, border and text paint first. */
const OWN: u8 = 0;
/** The negative z-order list of a stacking context. */
const NEGATIVE: u8 = 1;
/** Normal-flow block-level descendants: their backgrounds paint in tree order. */
pub(super) const BLOCK: u8 = 2;
/** Floats, each painted atomically after every block background. */
const FLOAT: u8 = 3;
/** Inline-level content: inline boxes, text runs and atomic inlines, in tree order, after
 * every block background and float of the layer (CSS 2.1 Appendix E), so text overhanging a
 * block paints over a later sibling block's background.
 * ponytail: a block's own leading text rides on the block's item and so still paints in the
 * block phase; splitting it into its own item would move it to this phase too. */
pub(super) const INLINE: u8 = 4;
/** The positive z-order list: z-index 0 and auto stacked boxes in tree order, then higher z. */
const POSITIVE: u8 = 5;

/** Read-only facts about the laid-out tree, shared by every per-item computation. */
struct Tree<'a> {
    document: &'a Document,
    positions: Vec<Option<usize>>,
    /// Paint order per identity: document order, with flex and grid items reordered among
    /// their siblings by `order` (css-display-3 order-modified document order).
    painting: Vec<Option<usize>>,
    /// The first item each identity painted, used for transform origins and clip boxes.
    boxes: Vec<Option<usize>>,
    /// Memoized absolute matrices per identity.
    matrices: Vec<Option<Matrix>>,
    /// Clamped scroll offsets per scroll container identity; empty paints everything unscrolled.
    offsets: ScrollOffsets,
    /// How far each scroll container's contents reach right and down (its scrollable overflow).
    reach: std::collections::BTreeMap<usize, [f64; 2]>,
    /// Set while scroll ranges are measured: sticky boxes count where layout put them, as
    /// scrollable overflow ignores sticky offsets in Blink. Otherwise a `bottom: 0` sticky item
    /// pulled up into view hid the very overflow it sticks within (position-sticky-flex-item).
    measuring: bool,
    /// The viewport's width and height: the view a sticky box under a fixed ancestor sticks to.
    viewport: [f64; 2],
}
impl<'a> Tree<'a> {
    /** Identity lookup never assumes identities equal vector indexes. */
    fn node(&self, local: usize) -> EngineResult<&'a Node> {
        self.positions
            .get(local)
            .copied()
            .flatten()
            .map(|position| &self.document.nodes[position])
            .ok_or_else(|| EngineError::invalid("Invalid layout node identity."))
    }
    /** Order-modified document order of an identity. */
    fn order(&self, local: usize) -> usize {
        self.painting
            .get(local)
            .copied()
            .flatten()
            .unwrap_or(usize::MAX)
    }
    /** Ancestors from the parent upward. */
    fn ancestors(&self, node: &'a Node) -> EngineResult<Vec<&'a Node>> {
        let mut chain = Vec::new();
        let mut current = node.parent;
        while let Some(local) = current {
            let ancestor = self.node(local)?;
            chain.push(ancestor);
            current = ancestor.parent;
        }
        Ok(chain)
    }
}

/// Scroll offsets `[x, y]` the host holds per scroll container identity. Like Blink's
/// `PaintLayerScrollableArea`, they never change layout: paint translates the container's
/// contents by them, and Eye clamps each to the container's scroll range.
pub type ScrollOffsets = std::collections::BTreeMap<usize, [f64; 2]>;

/// Resolves effects on the laid-out items and returns them in final paint order, with every
/// scroll container whose content overflows it.
pub(super) fn stack(
    document: &Document,
    items: Vec<DisplayItem>,
    offsets: &ScrollOffsets,
    viewport: [f64; 2],
) -> EngineResult<(Vec<DisplayItem>, Vec<ScrollArea>)> {
    let positions = sizing::positions(document);
    let mut boxes = vec![None; positions.len()];
    for (index, item) in items.iter().enumerate() {
        if let Some(slot) = boxes.get_mut(item.local_id)
            && slot.is_none()
        {
            *slot = Some(index);
        }
    }
    let painting = super::order::modified(document, &positions);
    let mut tree = Tree {
        document,
        painting,
        matrices: vec![None; positions.len()],
        positions,
        boxes,
        offsets: ScrollOffsets::new(),
        reach: std::collections::BTreeMap::new(),
        measuring: true,
        viewport,
    };
    let ranges = scroll_ranges(&mut tree, &items)?;
    tree.measuring = false;
    tree.offsets = clamp_offsets(&ranges, offsets);
    // Sticky boxes inside scroll containers move with the offsets and the reach just learned, so
    // matrices memoized while measuring the ranges are recomputed.
    tree.reach = ranges
        .iter()
        .map(|(local, range)| (*local, range.reach))
        .collect();
    tree.matrices.iter_mut().for_each(|slot| *slot = None);
    let areas = scroll_areas(&mut tree, &items, &ranges)?;
    let mut resolved = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let node = tree.node(item.local_id)?;
        let matrix = painted(&mut tree, &items, node)?;
        let clip = clip(&mut tree, &items, node)?;
        let clip = generated_clip(&mut tree, &items, node, item, clip)?;
        // A column slice carries its own clip, which the overflow clips intersect.
        let clip = match (
            clip,
            item.clip
                .map(|own| super::display::transform_rect(&own, Some(matrix))),
        ) {
            (Some(outer), Some(own)) => Some(outer.intersect(&own)),
            (outer, own) => outer.or(own),
        };
        let key = key(&tree, node, item)?;
        let pin = pin(&tree, &items, node)?;
        if let Some(echo) = super::text_echo::inline_key(node, item, &key) {
            resolved.push((echo, index, matrix, clip, pin));
        }
        resolved.push((key, index, matrix, clip, pin));
    }
    resolved.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut slots = items.into_iter().map(Some).collect::<Vec<_>>();
    let mut ordered = Vec::with_capacity(slots.len());
    let mut echoes = std::collections::BTreeMap::new();
    for (key, index, matrix, clip, pin) in resolved {
        let Some(mut item) = slots[index].take() else {
            // The second entry of a block whose text paints later: its echo goes here.
            ordered.extend(echoes.remove(&index));
            continue;
        };
        let node = tree.node(item.local_id)?;
        match generated_style(node, &item) {
            _ if item.bare => {}
            Some(style) => {
                let mut generated = node.clone();
                generated.style = style.clone();
                decorate(&mut item, &generated);
            }
            None if item.tag == super::multicol::RULE_TAG => {}
            None => decorate(&mut item, node),
        }
        let generated = generated_style(node, &item);
        if !item.bare {
            super::border_image::decorate(&mut item, node, generated);
        }
        item.stacking_opacity = groups(&tree, node)?;
        item.z = key
            .iter()
            .rev()
            .find(|step| step.0 == NEGATIVE || step.0 == POSITIVE)
            .map_or(0, |s| s.1);
        // Layout's own clip (a table part's background under one cell) narrows the ancestors';
        // it moves with the box's own offset, as the cell it outlines moves with it.
        let own = item.clip.take().map(|mut own| {
            if matrix[..4] == IDENTITY[..4] {
                own.x += matrix[4];
                own.y += matrix[5];
            }
            own
        });
        let clip = match (own, clip) {
            (Some(own), Some(outer)) => Some(own.intersect(&outer)),
            (own, outer) => own.or(outer),
        };
        item.clip = content_clip(node, &item, matrix, clip);
        let matrix = generated_matrix(node, &item, matrix);
        place(&mut item, matrix);
        item.pin = pin;
        if super::text_echo::deferred(node, &item, &key) {
            echoes.insert(index, super::text_echo::defer(&mut item));
        }
        ordered.push(item);
    }
    Ok((ordered, areas))
}

/** Vector content of an img or inline svg stays inside the content box, as Blink's replaced
 * painters clip it (`object-fit: cover`, `preserveAspectRatio` slice, paths past the viewBox). */
fn content_clip(
    node: &Node,
    item: &DisplayItem,
    matrix: Matrix,
    clip: Option<ClipRect>,
) -> Option<ClipRect> {
    if item.vector.is_empty() || !super::svg::clips_content(node) {
        return clip;
    }
    let border = sizing::border_sides(&node.style);
    let inset: [f64; 4] = std::array::from_fn(|side| {
        border[side] + node.style.padding[side] - replaced_clip_outset(&node.style, side)
    });
    let [top, right, bottom, left] = inset;
    let mut content = ClipRect {
        x: item.x + left,
        y: item.y + top,
        width: (item.width - left - right).max(0.0),
        height: (item.height - top - bottom).max(0.0),
        radius: [0.0; 4],
    };
    open_visible_axis(&node.style, &mut content);
    let content = super::display::transform_rect(&content, Some(matrix));
    Some(match clip {
        Some(outer) => outer.intersect(&content),
        None => content,
    })
}
/** A replaced element with `overflow: clip` on one axis and `visible` on the other clips only the
first, as boxes do (overflow-clip-y-visible-x-svg). */
fn open_visible_axis(style: &crate::document::ComputedStyle, rect: &mut ClipRect) {
    let open = crate::constants::LAYOUT_EXTENT / 2.0;
    if style.overflow_x == Overflow::Visible && style.overflow_y == Overflow::Clip {
        (rect.x, rect.width) = (-open, open * 2.0);
    }
    if style.overflow_y == Overflow::Visible && style.overflow_x == Overflow::Clip {
        (rect.y, rect.height) = (-open, open * 2.0);
    }
}
/** How far past its content box a replaced element's vector content may paint on `side`: its
`overflow-clip-margin` measured from the reference box (padding box by default), as Blink
applies the margin to replaced elements, which always clip (overflow-clip-margin-svg). */
fn replaced_clip_outset(style: &crate::document::ComputedStyle, side: usize) -> f64 {
    if style.overflow_clip_margin == 0.0 && style.overflow_clip_box == 0 {
        return 0.0;
    }
    let padding = style.padding[side];
    let reference = match style.overflow_clip_box {
        1 => padding + sizing::border_sides(style)[side],
        2 => 0.0,
        _ => padding,
    };
    reference + style.overflow_clip_margin
}
/** The style of a `::before` or `::after` item, which shares its element's identity. */
pub(super) fn generated_style<'a>(
    node: &'a Node,
    item: &DisplayItem,
) -> Option<&'a crate::document::ComputedStyle> {
    let after = match item.tag.as_str() {
        "::before" => false,
        "::after" => true,
        super::first_letter::FIRST_LETTER => return node.style.first_letter.as_deref(),
        _ => return None,
    };
    node.style
        .generated
        .iter()
        .find(|generated| generated.after == after)
        .map(|generated| &*generated.style)
}
/**
 * Whether a box is a stacking context, as Blink's `IsStackingContext`: the root, a z-index on a
 * box that allows one (positioned, or a flex or grid item), fixed and sticky boxes, transforms,
 * opacity, and the grouping properties (filter, blend mode, isolation, clip-path, mask,
 * will-change, paint or layout containment).
 */
fn stacking_context(
    style: &crate::document::ComputedStyle,
    parent: Option<&crate::document::ComputedStyle>,
) -> bool {
    let allows_z = style.position != Position::Static
        || parent.is_some_and(|parent| parent.display_flex || parent.display_grid);
    (allows_z && style.z_index.is_some())
        || matches!(style.position, Position::Fixed | Position::Sticky)
        || transformed(style)
        || style.opacity < 1.0
        || style.stacking != 0
        || style.clip_path.is_some()
        || style.masked
}
/** Whether a box is stacked, as Blink's `IsStacked`: positioned or a stacking context. It then
 * paints from its stacking context's z-order lists instead of with the normal flow. */
fn stacked(
    style: &crate::document::ComputedStyle,
    parent: Option<&crate::document::ComputedStyle>,
) -> bool {
    style.position != Position::Static || stacking_context(style, parent)
}
/** The z-index Blink stacks a box by (`EffectiveZIndex`): 0 unless the box allows one. */
fn effective_z(
    style: &crate::document::ComputedStyle,
    parent: Option<&crate::document::ComputedStyle>,
) -> i32 {
    let allows_z = style.position != Position::Static
        || parent.is_some_and(|parent| parent.display_flex || parent.display_grid);
    if allows_z {
        style.z_index.unwrap_or(0)
    } else {
        0
    }
}
/** A flex or grid item, which CSS paints exactly like an inline-block (css-flexbox 4.3). */
fn layout_item(parent: Option<&crate::document::ComputedStyle>) -> bool {
    parent.is_some_and(|parent| parent.display_flex || parent.display_grid)
}
/** The normal-flow phase a non-stacked box paints in: floats after block backgrounds, then
 * inline-level content, per CSS 2.1 Appendix E and Blink's paint phases. */
fn phase(link: &Link<'_>) -> u8 {
    if link.style.float != crate::document::style::Float::None {
        FLOAT
    } else if link.text || link.style.inline || link.style.inline_block || layout_item(link.parent)
    {
        INLINE
    } else {
        BLOCK
    }
}
/** A block item (other than a table part, whose borders a collapsed table paints apart) that
 * paints nothing but its own text runs (no background, border, shadow or other decoration): it paints with the inline content, after later sibling blocks'
 * backgrounds, as its text would in Blink. A decorated block keeps its text in the block phase
 * with its background.
 * ponytail: splitting a decorated block's text into its own item would move that text too. */
pub(super) fn only_text(item: &DisplayItem) -> bool {
    let bare_background = item
        .background
        .as_deref()
        .is_none_or(|color| color == "transparent");
    !item.lines.is_empty()
        && bare_background
        && item.backgrounds.is_empty()
        && item.border == 0.0
        && item.border_widths == [0.0; 4]
        && item.shadows.is_empty()
        && item.border_image.is_none()
        && item.vector.is_empty()
        && item.mask.is_none()
}
/** A table cell or other table part, by tag or by `display`. */
pub(super) fn table_part(node: &Node) -> bool {
    node.table_cell.is_some()
        || ["td", "th", "caption", "tr", "table"].contains(&node.tag.as_str())
        || node.style.table_box != crate::document::style::TableBox::None
}
/** Floats, atomic inlines and flex and grid items paint all their phases at once, as if they
 * were stacking contexts (Blink's `PaintAllPhasesAtomically`). */
fn atomic(link: &Link<'_>) -> bool {
    link.style.float != crate::document::style::Float::None
        || link.style.inline_block
        || link.replaced
        || layout_item(link.parent)
}
/** Form controls and replaced elements, which lay out as atomic inlines by their tag. */
const ATOMIC_TAGS: [&str; 7] = [
    "button", "input", "select", "textarea", "img", "canvas", "svg",
];
/** One box on the path from the root to a painted item: its style, its parent's, its order. */
struct Link<'a> {
    style: &'a crate::document::ComputedStyle,
    parent: Option<&'a crate::document::ComputedStyle>,
    order: usize,
    sub: u8,
    text: bool,
    /// A control or replaced element, an atomic inline whatever its display.
    replaced: bool,
    /// The document element, which Blink always makes a stacking context.
    root: bool,
}
/** The sub-order of an inline fragment that follows a block: after the block's last descendant
 * and its `::before` (1) and `::after` (2). */
const AFTER_BLOCK: u8 = 3;
/**
 * The paint-order key of an item, a port of Blink's `PaintLayerPainter` and
 * `PaintLayerStackingNode`. A stacked box enters the z-order list of its nearest stacking
 * context, negative z below the normal flow and the rest above it, sorted by z then tree order;
 * a positioned `z-index: auto` box is stacked but not a context, so its own stacked descendants
 * join the same lists rather than painting inside it. Normal-flow boxes of a layer paint block
 * backgrounds, then floats, then inline content; floats and atomic inlines paint atomically.
 * A `::before` or `::after` item is a child box with its own style.
 */
fn key(tree: &Tree<'_>, node: &Node, item: &DisplayItem) -> EngineResult<Vec<Step>> {
    let mut chain = tree.ancestors(node)?;
    chain.reverse();
    chain.push(node);
    let mut path: Vec<Link<'_>> = Vec::with_capacity(chain.len() + 1);
    let last = chain.len() - 1;
    for (index, element) in chain.iter().enumerate() {
        path.push(Link {
            style: &element.style,
            parent: index.checked_sub(1).map(|parent| &chain[parent].style),
            order: tree.order(element.local_id),
            sub: 0,
            text: element.tag == "#text"
                || (index == last && !table_part(element) && only_text(item)),
            replaced: element.style.inline && ATOMIC_TAGS.contains(&element.tag.as_str()),
            root: index == 0 && element.tag == "html",
        });
    }
    if let Some(style) = generated_style(node, item) {
        path.push(Link {
            style,
            parent: Some(&node.style),
            order: tree.order(node.local_id),
            sub: if item.tag == "::after" { 2 } else { 1 },
            text: false,
            replaced: false,
            root: false,
        });
    }
    // A fragment after a block inside its inline box paints after that block's content.
    if let (Some(after), Some(last)) = (item.paint_after, path.last_mut()) {
        last.order = tree.order(after);
        last.sub = AFTER_BLOCK;
    }
    Ok(path_key(&path))
}
/** The key of the last link of `path`, built from the root down. */
fn path_key(path: &[Link<'_>]) -> Vec<Step> {
    // The key prefix of each enclosing stacking context's members, innermost last.
    let mut contexts: Vec<Vec<Step>> = vec![Vec::new()];
    // The prefix under which the current painting layer's normal flow paints.
    let mut layer: Vec<Step> = Vec::new();
    let last = path.len().saturating_sub(1);
    for (index, link) in path.iter().enumerate() {
        if link.root || stacked(link.style, link.parent) {
            let z = effective_z(link.style, link.parent);
            let bucket = if z < 0 { NEGATIVE } else { POSITIVE };
            layer.clone_from(contexts.last().unwrap_or(&Vec::new()));
            layer.push((bucket, z, link.order, link.sub));
            if link.root || stacking_context(link.style, link.parent) {
                contexts.push(layer.clone());
            }
        } else if index == last {
            layer.push((phase(link), 0, link.order, link.sub));
            return layer;
        } else if atomic(link) {
            layer.push((phase(link), 0, link.order, link.sub));
        }
    }
    layer.push((OWN, 0, 0, 0));
    layer
}
/** Translucent stacking contexts around and including this element, outermost first. */
fn groups(tree: &Tree<'_>, node: &Node) -> EngineResult<Vec<OpacityGroup>> {
    let mut chain = tree.ancestors(node)?;
    chain.insert(0, node);
    Ok(chain
        .into_iter()
        .rev()
        .filter(|node| node.style.opacity < 1.0)
        .map(|node| OpacityGroup {
            id: node.local_id,
            opacity: node.style.opacity,
        })
        .collect())
}

/** Composition: `outer` applied after `inner`. */
fn multiply(outer: Matrix, inner: Matrix) -> Matrix {
    let [a, b, c, d, e, f] = outer;
    let [p, q, r, s, t, u] = inner;
    [
        a * p + c * q,
        b * p + d * q,
        a * r + c * s,
        b * r + d * s,
        a * t + c * u + e,
        b * t + d * u + f,
    ]
}
/** Relative offsets and the transform about the box center, in the parent's coordinates. */
fn local(tree: &Tree<'_>, items: &[DisplayItem], node: &Node) -> Matrix {
    let style = &node.style;
    let mut matrix = IDENTITY;
    let own = tree
        .boxes
        .get(node.local_id)
        .copied()
        .flatten()
        .map(|index| &items[index]);
    if style.position == Position::Sticky && !tree.measuring {
        [matrix[4], matrix[5]] = match scroll_container(tree, node) {
            Some(scroller) => inner_sticky_offset(tree, items, node, own, scroller),
            None => fixed_sticky_offset(tree, items, node, own)
                .unwrap_or([0.0, sticky_offset(tree, items, node, own)]),
        };
    } else if style.position == Position::Relative {
        let [x, y] = relative_offset(tree, items, node);
        matrix[4] = x;
        matrix[5] = y;
    }
    if transformed(style) {
        matrix = multiply(matrix, own_transform(style, own));
    }
    matrix
}
/** The containing block a relatively positioned box resolves percentages against: its nearest
ancestor that is no inline box, since a block inside an inline (position-relative-002) or an
inline itself sits in the block around it. None for a top-level box (the body). */
fn block_container<'a>(tree: &Tree<'a>, node: &Node) -> Option<&'a Node> {
    let mut parent = node.parent.and_then(|local| tree.node(local).ok());
    while let Some(inline) = parent.filter(|p| p.style.inline && !p.style.inline_block) {
        parent = inline.parent.and_then(|local| tree.node(local).ok());
    }
    parent
}
/**
 * A relatively positioned box's offset, after Blink's ComputeRelativeOffset: insets resolve
 * against the parent's content box, and a percentage against an indefinite height (the parent's
 * `height` is auto) counts as `auto`, so `top: 100%` in an auto-height table row stays put
 * instead of moving by the row's own height. With both `left` and `right` set, the parent's
 * direction picks the winner.
 */
fn relative_offset(tree: &Tree<'_>, items: &[DisplayItem], node: &Node) -> [f64; 2] {
    let style = &node.style;
    let parent = block_container(tree, node);
    let (width, height) = parent.map_or_else(
        || (None, body_height(tree.document)),
        |parent| containing_size(tree, items, parent),
    );
    let inset = |side: usize, basis: Option<f64>| {
        if style.inset_percent[side] != 0.0 {
            basis.and_then(|basis| super::positioned::inset(style, side, basis))
        } else {
            super::positioned::inset(style, side, 0.0)
        }
    };
    let (left, right) = (inset(3, width), inset(1, width));
    let (top, bottom) = (inset(0, height), inset(2, height));
    let rtl = parent.is_some_and(|parent| parent.style.rtl);
    let x = match (left, right) {
        (Some(_), Some(right)) if rtl => -right,
        (Some(left), _) => left,
        (None, right) => right.map_or(0.0, |right| -right),
    };
    [x, top.or(bottom.map(|bottom| -bottom)).unwrap_or(0.0)]
}
/** The body's definite content height, the basis of a top-level box's percentage `top` and
`bottom` (CSS2 top-113: `top: 100%` under `body { height: 1in }`).
ponytail: a percentage body height (against the viewport) and the width basis are not resolved
here, since stacking runs without the viewport size. */
fn body_height(document: &crate::document::Document) -> Option<f64> {
    let body = &document.formatting.body.style;
    let height = body.height?.resolve_size(None)?;
    Some(sizing::content_height(body, height))
}
/** The content-box size a parent offers its relatively positioned children: the width of its
 * box (or the nearest boxed ancestor's), and a height only when an authored one makes it
 * definite. ponytail: a percentage height counts as definite without checking its own basis. */
fn containing_size(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    parent: &Node,
) -> (Option<f64>, Option<f64>) {
    let item = |node: &Node| {
        tree.boxes
            .get(node.local_id)
            .copied()
            .flatten()
            .map(|index| &items[index])
    };
    let width = std::iter::once(parent)
        .chain(tree.ancestors(parent).unwrap_or_default())
        .find_map(|node| {
            item(node).map(|own| (own.width - sizing::horizontal_edges(node)).max(0.0))
        });
    let height = parent
        .style
        .height
        .and_then(|height| match height.resolve_size(None) {
            Some(px) => Some(sizing::content_height(&parent.style, px)),
            None => item(parent).map(|own| (own.height - sizing::vertical_edges(parent)).max(0.0)),
        });
    (width, height)
}
/**
 * How far a sticky box moves down in the unscrolled page Eye renders: only far enough that its
 * top sits `top` below the viewport top, and never past its parent's bottom. Its insets are
 * thresholds, not offsets; BBC's `top: 80px` navigation dropped 80px below its header.
 * ponytail: the page is laid out at scroll zero, so `bottom` and horizontal stickiness stay put.
 */
fn sticky_offset(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    own: Option<&DisplayItem>,
) -> f64 {
    sticky_pin(tree, items, node, own).map_or(0.0, |pin| pin.offset(0.0))
}
/** The nearest scroll container a sticky box sticks to, None for the viewport. A fixed box
 * between them pins to the viewport, so the walk stops there. The walk follows containing blocks:
 * above an absolutely positioned ancestor it skips to that box's containing block, so a scroller
 * the absolute box escapes is not the sticky box's (position-sticky-escape-scroller-003). */
fn scroll_container<'a>(tree: &Tree<'a>, node: &'a Node) -> Option<&'a Node> {
    let mut escaping = false;
    for ancestor in tree.ancestors(node).ok()? {
        let style = &ancestor.style;
        escaping &= style.position == Position::Static && style.transform.is_none();
        if !escaping && scrolls(style) {
            return Some(ancestor);
        }
        match style.position {
            Position::Fixed => return None,
            Position::Absolute => escaping = true,
            _ => {}
        }
    }
    None
}
/** The containing block a sticky box travels within, never above `stop`. */
fn container(tree: &Tree<'_>, node: &Node, stop: Option<usize>) -> Option<usize> {
    super::sticky_container::sticky_container(tree.document, &tree.positions, node, stop)
}
/** A rectangle as [left, top, right, bottom] in layout coordinates. */
type Edges = [f64; 4];
/** The border box of an identity's first item as edges. */
fn edges_of(tree: &Tree<'_>, items: &[DisplayItem], local: usize) -> Option<Edges> {
    let item = &items[tree.boxes.get(local).copied().flatten()?];
    Some([item.x, item.y, item.x + item.width, item.y + item.height])
}
/** Edges moved inward by per-side amounts in CSS order (top, right, bottom, left). */
fn contract([l, t, r, b]: Edges, [top, right, bottom, left]: [f64; 4]) -> Edges {
    [l + left, t + top, r - right, b - bottom]
}
/**
 * A sticky box's offset inside a scroll container, a port of Blink's
 * UpdateStickyPositionConstraints and StickyPositionScrollingConstraints: the constraining rect
 * is the container's content box at its scroll offset; the box may travel within its parent's
 * content box (the scrollable overflow when the parent is the scroller) less its own margins; a
 * start inset pushes it forward, then an end inset pulls it back, each limited by that room.
 * Sticky ancestors below the scroller move the box first (`moved`).
 * ponytail: transforms between the box and the scroller are ignored, and nested offsets are
 * recomputed per level (quadratic in sticky depth, which real pages keep tiny).
 */
fn inner_sticky_offset(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    own: Option<&DisplayItem>,
    scroller: &Node,
) -> [f64; 2] {
    let (Some(own), Some(outer)) = (own, edges_of(tree, items, scroller.local_id)) else {
        return [0.0; 2];
    };
    let [sx, sy] = tree
        .offsets
        .get(&scroller.local_id)
        .copied()
        .unwrap_or_default();
    let client = contract(outer, sizing::border_sides(&scroller.style));
    let [l, t, r, b] = contract(client, scroller.style.padding);
    let view = [l + sx, t + sy, r + sx, b + sy];
    let parent = container(tree, node, Some(scroller.local_id)).unwrap_or(scroller.local_id);
    let room = if parent == scroller.local_id {
        let [right, bottom] = tree
            .reach
            .get(&parent)
            .copied()
            .unwrap_or([client[2], client[3]]);
        contract(
            [client[0], client[1], right, bottom],
            scroller.style.padding,
        )
    } else {
        let Some(box_edges) = edges_of(tree, items, parent) else {
            return [0.0; 2];
        };
        let parent = tree
            .node(parent)
            .map(|n| &n.style)
            .unwrap_or(&scroller.style);
        contract(
            contract(box_edges, sizing::border_sides(parent)),
            parent.padding,
        )
    };
    let style = &node.style;
    let margins = std::array::from_fn(|side| {
        if style.margin_auto[side] {
            0.0
        } else {
            style.margin[side]
        }
    });
    // Sticky ancestors below the scroller already moved the box and maybe its containing block.
    let [rx, ry] = moved(tree, items, parent, scroller);
    let room = contract(room, margins);
    let room = [room[0] + rx, room[1] + ry, room[2] + rx, room[3] + ry];
    let [dx, dy] = node
        .parent
        .map_or([0.0; 2], |up| moved(tree, items, up, scroller));
    let rect = [
        own.x + dx,
        own.y + dy,
        own.x + own.width + dx,
        own.y + own.height + dy,
    ];
    sticky_axes(style, view, room, rect)
}
/** A sticky box's offset on both axes for its border box `rect` in the `view` it sticks to and
the `room` its containing block allows (edges as left, top, right, bottom): a start inset pushes
it forward, then an end inset pulls it back, each limited by the room. */
fn sticky_axes(
    style: &crate::document::ComputedStyle,
    view: Edges,
    room: Edges,
    rect: Edges,
) -> [f64; 2] {
    let axis = |start: usize, end: usize, low: usize, high: usize| {
        let size = view[high] - view[low];
        let mut min = super::positioned::inset(style, start, size);
        let mut max = super::positioned::inset(style, end, size);
        if let (Some(lo), Some(hi)) = (min, max) {
            let free = size - (rect[high] - rect[low]) - lo - hi;
            if free < 0.0 {
                max = Some(hi + free);
            }
        }
        let mut offset = 0.0;
        if let Some(lo) = min.take() {
            let push = (view[low] + lo - rect[low]).min(room[high] - rect[high]);
            offset += push.max(0.0);
        }
        if let Some(hi) = max {
            let pull =
                (rect[high] + offset - (view[high] - hi)).min(rect[low] + offset - room[low]);
            offset -= pull.max(0.0);
        }
        offset
    };
    [axis(3, 1, 0, 2), axis(0, 2, 1, 3)]
}
/** A sticky box under a fixed ancestor with no scroller between: nothing scrolls it, so it
sticks to the viewport as laid out, on every side (position-sticky-fixed-ancestor-002: a
`bottom: 0` box pulled up into a full-viewport fixed container). None without such an ancestor. */
fn fixed_sticky_offset(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    own: Option<&DisplayItem>,
) -> Option<[f64; 2]> {
    let ancestors = tree.ancestors(node).ok()?;
    ancestors
        .iter()
        .find(|a| a.style.position == Position::Fixed)?;
    let own = own?;
    let parent = container(tree, node, None)?;
    let parent_node = tree.node(parent).ok()?;
    let style = &parent_node.style;
    let edges = edges_of(tree, items, parent)?;
    let room = contract(contract(edges, sizing::border_sides(style)), style.padding);
    let view = [0.0, 0.0, tree.viewport[0], tree.viewport[1]];
    let rect = [own.x, own.y, own.x + own.width, own.y + own.height];
    Some(sticky_axes(&node.style, view, room, rect))
}
/** How far the sticky boxes at and above `local`, below `scroller`, move it: a nested sticky box
starts from where its sticky ancestors put it (position-sticky-nested-thead-th). */
fn moved(tree: &Tree<'_>, items: &[DisplayItem], local: usize, scroller: &Node) -> [f64; 2] {
    let mut total = [0.0; 2];
    let mut current = tree.node(local).ok();
    while let Some(node) = current.filter(|node| node.local_id != scroller.local_id) {
        if node.style.position == Position::Sticky {
            let own = tree
                .boxes
                .get(node.local_id)
                .copied()
                .flatten()
                .map(|index| &items[index]);
            let [x, y] = inner_sticky_offset(tree, items, node, own, scroller);
            total = [total[0] + x, total[1] + y];
        }
        current = node.parent.and_then(|up| tree.node(up).ok());
    }
    total
}
/** A sticky box's constraint against the viewport (Blink's StickyPositionScrollingConstraints):
 * its `top` inset, where it sits unstuck, and how far its containing block lets it travel. */
fn sticky_pin(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    own: Option<&DisplayItem>,
) -> Option<Pin> {
    let own = own?;
    let parent = container(tree, node, None)
        .and_then(|parent| tree.boxes.get(parent).copied().flatten())
        .map(|index| &items[index]);
    let top = super::positioned::inset(&node.style, 0, parent.map_or(0.0, |p| p.height))?;
    // A top-level box's containing block is the body, which ends where the content does.
    let bottom = parent.map_or_else(
        || {
            items
                .iter()
                .map(|item| item.y + item.height)
                .fold(0.0, f64::max)
        },
        |p| p.y + p.height,
    );
    let room = (bottom - own.y - own.height).max(0.0);
    Some(Pin {
        top,
        start: own.y,
        room,
    })
}
/** How a box follows the viewport's scroll: the nearest sticky or fixed box at or above it that
 * the viewport, not an inner scroller or a transform, moves. Painters shift it by
 * `Pin::shift(scroll)`; layout and the rest of the display stay at scroll 0. */
fn pin(tree: &Tree<'_>, items: &[DisplayItem], node: &Node) -> EngineResult<Option<Pin>> {
    let mut chain = vec![node];
    chain.extend(tree.ancestors(node)?);
    for (depth, anchor) in chain.iter().enumerate() {
        let style = &anchor.style;
        if style.position != Position::Sticky && style.position != Position::Fixed {
            continue;
        }
        let above = &chain[depth + 1..];
        if above.iter().any(|a| a.style.transform.is_some())
            || (style.position == Position::Sticky
                && above.iter().any(|a| a.style.overflow != Overflow::Visible))
        {
            return Ok(None);
        }
        if style.position == Position::Fixed {
            return Ok(Some(Pin {
                top: 0.0,
                start: 0.0,
                room: crate::constants::LAYOUT_EXTENT,
            }));
        }
        let own = tree
            .boxes
            .get(anchor.local_id)
            .copied()
            .flatten()
            .map(|index| &items[index]);
        return Ok(sticky_pin(tree, items, anchor, own));
    }
    Ok(None)
}
/** Whether a style moves its box with `transform` or a percentage `translate`. */
fn transformed(style: &crate::document::ComputedStyle) -> bool {
    style.transform.is_some() || style.translate_percent != [0.0; 2]
}
/** A box's `transform` and percentage `translate` about its own center, in its layout coordinates. */
fn own_transform(style: &crate::document::ComputedStyle, own: Option<&DisplayItem>) -> Matrix {
    let mut transform = style.transform.unwrap_or(IDENTITY);
    let (x, y, w, h) = own
        .map(|item| {
            (
                item.x + item.width / 2.0,
                item.y + item.height / 2.0,
                item.width,
                item.height,
            )
        })
        .unwrap_or_default();
    if style.translate_percent != [0.0; 2] {
        transform = multiply(
            transform,
            [
                1.0,
                0.0,
                0.0,
                1.0,
                w * style.translate_percent[0] / 100.0,
                h * style.translate_percent[1] / 100.0,
            ],
        );
    }
    multiply(
        [1.0, 0.0, 0.0, 1.0, x, y],
        multiply(transform, [1.0, 0.0, 0.0, 1.0, -x, -y]),
    )
}
/**
 * A `::before` or `::after` box shares its element's matrix and adds its own transform, so a
 * placeholder `transform: translate(-50%, -50%)` centers the generated box, not its element.
 */
fn generated_matrix(node: &Node, item: &DisplayItem, matrix: Matrix) -> Matrix {
    match generated_style(node, item) {
        Some(style) if transformed(style) => multiply(matrix, own_transform(style, Some(item))),
        _ => matrix,
    }
}
/** Every ancestor's offsets and transforms, composed from the root down; memoized per identity. */
fn absolute(tree: &mut Tree<'_>, items: &[DisplayItem], node: &Node) -> EngineResult<Matrix> {
    let mut pending = vec![node];
    let mut base = IDENTITY;
    let mut current = node.parent;
    while let Some(local) = current {
        if let Some(matrix) = tree.matrices.get(local).copied().flatten() {
            base = matrix;
            break;
        }
        let ancestor = tree.node(local)?;
        pending.push(ancestor);
        current = ancestor.parent;
    }
    for node in pending.into_iter().rev() {
        base = multiply(base, local(tree, items, node));
        if let Some(slot) = tree.matrices.get_mut(node.local_id) {
            *slot = Some(base);
        }
    }
    Ok(base)
}
/** Where a box paints: its offsets and transforms, then the scroll translation of every scroll
 * container it scrolls with (Blink's ScrollTranslation paint property node).
 * ponytail: the scroll translation applies after ancestor transforms, exact unless a transformed
 * box sits between a scroller and its content; per-node property trees are the upgrade. */
fn painted(tree: &mut Tree<'_>, items: &[DisplayItem], node: &Node) -> EngineResult<Matrix> {
    let mut matrix = absolute(tree, items, node)?;
    if !tree.offsets.is_empty() {
        for ancestor in clipping_ancestors(tree, node)? {
            if let Some([x, y]) = tree.offsets.get(&ancestor.local_id) {
                matrix[4] -= x;
                matrix[5] -= y;
            }
        }
    }
    Ok(matrix)
}
/** Whether overflow makes a box a scroll container (`hidden` scrolls programmatically; `clip`
 * never does), as Blink's `IsScrollContainer`. */
fn scrolls(style: &crate::document::ComputedStyle) -> bool {
    matches!(
        style.overflow,
        Overflow::Auto | Overflow::Scroll | Overflow::Hidden
    )
}
/** A scroll container's unscrolled padding box and how far its contents reach right and down. */
struct Range {
    client: ClipRect,
    reach: [f64; 2],
}
/** Each scroll container's range from its scrollable overflow: the padding box united with every
 * box that scrolls with it, plus its end padding (css-overflow-3 2.2). A box counts toward its
 * innermost scroll container only, since an inner one clips its own contents. */
fn scroll_ranges(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
) -> EngineResult<std::collections::BTreeMap<usize, Range>> {
    let mut ranges = std::collections::BTreeMap::new();
    // Out-of-flow boxes reach as far as they paint; only in-flow content gets the end padding.
    let mut unpadded: std::collections::BTreeMap<usize, [f64; 2]> = Default::default();
    // Layout hides boxes with opacity 0 (lines past a line clamp, boxes in a hidden container);
    // they are ink overflow at most and give the scroller no range (line-clamp-021).
    for item in items
        .iter()
        .filter(|item| item.width > 0.0 && item.height > 0.0 && item.opacity > 0.0)
    {
        let node = tree.node(item.local_id)?;
        let Some(scroller) = clipping_ancestors(tree, node)?
            .into_iter()
            .find(|ancestor| scrolls(&ancestor.style))
        else {
            continue;
        };
        let range = match ranges.entry(scroller.local_id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let Some(client) = unscrolled_padding_box(tree, items, scroller)? else {
                    continue;
                };
                let [_, right, bottom, _] = scroller.style.padding;
                entry.insert(Range {
                    reach: [
                        client.x + client.width - right,
                        client.y + client.height - bottom,
                    ],
                    client,
                })
            }
        };
        let matrix = absolute(tree, items, node)?;
        let own = ClipRect {
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            radius: [0.0; 4],
        };
        let own = super::display::transform_rect(&own, Some(matrix));
        let positioned = positioned_within(tree, node, scroller.local_id)?;
        let mut bottom = own.y + own.height;
        if !positioned && let Some(end) = clamp_bottom(tree, items, node, scroller.local_id) {
            bottom = bottom.min(end);
        }
        let reach = if positioned {
            unpadded.entry(scroller.local_id).or_default()
        } else {
            &mut range.reach
        };
        reach[0] = reach[0].max(own.x + own.width);
        reach[1] = reach[1].max(bottom);
    }
    for (local, range) in ranges.iter_mut() {
        let [_, right, bottom, _] = tree.node(*local)?.style.padding;
        let plain = unpadded.get(local).copied().unwrap_or_default();
        range.reach[0] = (range.reach[0] + right).max(plain[0]);
        range.reach[1] = (range.reach[1] + bottom).max(plain[1]);
    }
    Ok(ranges)
}
/** The border-box bottom of the nearest line-clamp container around `node` below `scroller`:
in-flow content past the clamp (a float hanging off the last kept line) is clipped there and
is no scrollable overflow (line-clamp-with-floats-010); absolute boxes still count (-019). */
fn clamp_bottom(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    scroller: usize,
) -> Option<f64> {
    let mut current = node.parent;
    while let Some(local) = current.filter(|local| *local != scroller) {
        let ancestor = tree.node(local).ok()?;
        if ancestor.style.continue_collapse {
            return edges_of(tree, items, local).map(|edges| edges[3]);
        }
        current = ancestor.parent;
    }
    None
}
/** Whether `node` is out of flow inside the scroll container `scroller`: it or an ancestor below
the scroller is absolutely positioned. Its box counts toward the scrollable overflow as painted,
without the container's end padding that in-flow content gets (css-overflow-3 2.2;
position-sticky-padding-001 scrolls an abspos marker exactly to the bottom edge). */
fn positioned_within(tree: &Tree<'_>, node: &Node, scroller: usize) -> EngineResult<bool> {
    let mut current = node;
    while current.local_id != scroller {
        if current.style.position == Position::Absolute {
            return Ok(true);
        }
        let Some(parent) = current.parent else {
            return Ok(false);
        };
        current = tree.node(parent)?;
    }
    Ok(false)
}
/** A scroll container's padding box before any scrolling moved it. */
fn unscrolled_padding_box(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
) -> EngineResult<Option<ClipRect>> {
    let Some(index) = tree.boxes.get(node.local_id).copied().flatten() else {
        return Ok(None);
    };
    let item = &items[index];
    let [top, right, bottom, left] = sizing::border_sides(&node.style);
    let padding = ClipRect {
        x: item.x + left,
        y: item.y + top,
        width: (item.width - left - right).max(0.0),
        height: (item.height - top - bottom).max(0.0),
        radius: [0.0; 4],
    };
    let matrix = absolute(tree, items, node)?;
    Ok(Some(super::display::transform_rect(&padding, Some(matrix))))
}
/** The furthest a container scrolls along each axis: its scroll size less its client size. */
fn max_offset(range: &Range) -> [f64; 2] {
    [
        (range.reach[0] - range.client.x - range.client.width).max(0.0),
        (range.reach[1] - range.client.y - range.client.height).max(0.0),
    ]
}
/** Requested offsets held to each container's range; offsets for boxes that do not scroll drop. */
fn clamp_offsets(
    ranges: &std::collections::BTreeMap<usize, Range>,
    requested: &ScrollOffsets,
) -> ScrollOffsets {
    requested
        .iter()
        .filter_map(|(local, [x, y])| {
            let [max_x, max_y] = max_offset(ranges.get(local)?);
            let clamped = [x.clamp(0.0, max_x), y.clamp(0.0, max_y)];
            (clamped != [0.0; 2]).then_some((*local, clamped))
        })
        .collect()
}
/** Every scroll container with somewhere to scroll, where it paints and how far it can go, so
 * hosts route a wheel to the innermost one under the pointer and clamp what they ask for. */
fn scroll_areas(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    ranges: &std::collections::BTreeMap<usize, Range>,
) -> EngineResult<Vec<ScrollArea>> {
    let mut areas = Vec::new();
    for (local, range) in ranges {
        let max = max_offset(range);
        if max == [0.0; 2] {
            continue;
        }
        let node = tree.node(*local)?;
        let style = &node.style;
        // `visible` beside a scrolling axis computes to `auto` (css-overflow-3 3.1).
        let user =
            |axis: Overflow| matches!(axis, Overflow::Auto | Overflow::Scroll | Overflow::Visible);
        let shift = painted(tree, items, node)?;
        let unshifted = absolute(tree, items, node)?;
        let offset = tree.offsets.get(local).copied().unwrap_or_default();
        areas.push(ScrollArea {
            local_id: *local,
            x: range.client.x + shift[4] - unshifted[4],
            y: range.client.y + shift[5] - unshifted[5],
            width: range.client.width,
            height: range.client.height,
            max_x: max[0],
            max_y: max[1],
            scroll_x: offset[0],
            scroll_y: offset[1],
            user_x: user(style.overflow_x),
            user_y: user(style.overflow_y),
        });
    }
    Ok(areas)
}
/** A `::before` or `::after` box is its element's content, so the element's own overflow clip
applies to it as to a child, though the box shares the element's identity
(overflow-clip-margin-border-radius-002). */
fn generated_clip(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    item: &DisplayItem,
    clip: Option<ClipRect>,
) -> EngineResult<Option<ClipRect>> {
    let generated = matches!(item.tag.as_str(), "::before" | "::after");
    let clips = node.style.overflow != Overflow::Visible || node.style.contain_paint;
    if !generated || !clips {
        return Ok(clip);
    }
    let Some(own) = clip_box(tree, items, node)? else {
        return Ok(clip);
    };
    Ok(Some(match clip {
        Some(outer) => outer.intersect(&own),
        None => own,
    }))
}
/** Overflow clips, then the `clip-path` and `clip` shapes of this element and every ancestor. */
fn clip(tree: &mut Tree<'_>, items: &[DisplayItem], node: &Node) -> EngineResult<Option<ClipRect>> {
    let mut result = overflow_clip(tree, items, node)?;
    let chain = std::iter::once(node).chain(tree.ancestors(node)?);
    for clipping in chain.collect::<Vec<_>>() {
        if let Some(rect) = shape(tree, items, clipping)? {
            result = Some(match result {
                Some(inner) => inner.intersect(&rect),
                None => rect,
            });
        }
    }
    Ok(result)
}
/**
 * The region `clip-path: inset()` and, on absolute and fixed boxes, `clip: rect()` leave of a
 * box, in document space. They clip the whole subtree, positioned descendants included, so a
 * visually hidden label (`clip: rect(1px,1px,1px,1px)`) paints nothing.
 */
fn shape(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
) -> EngineResult<Option<ClipRect>> {
    let style = &node.style;
    let absolute_box = matches!(style.position, Position::Absolute | Position::Fixed);
    let rect_clip = style.clip_rect.filter(|_| absolute_box);
    let insets = [style.clip_path, rect_clip].into_iter().flatten();
    let Some(index) = tree.boxes.get(node.local_id).copied().flatten() else {
        return Ok(None);
    };
    let item = &items[index];
    let mut result: Option<ClipRect> = None;
    for [top, right, bottom, left] in insets.collect::<Vec<_>>() {
        let along = |extent: crate::document::style::Extent, basis: f64| {
            extent.resolve(Some(basis)).unwrap_or(0.0)
        };
        let (top, bottom) = (along(top, item.height), along(bottom, item.height));
        let (left, right) = (along(left, item.width), along(right, item.width));
        let own = ClipRect {
            x: item.x + left,
            y: item.y + top,
            width: (item.width - left - right).max(0.0),
            height: (item.height - top - bottom).max(0.0),
            radius: [0.0; 4],
        };
        result = Some(match result {
            Some(inner) => inner.intersect(&own),
            None => own,
        });
    }
    let Some(own) = result else {
        return Ok(None);
    };
    let matrix = painted(tree, items, node)?;
    Ok(Some(super::display::transform_rect(&own, Some(matrix))))
}
/** Ancestors whose overflow clips this element, innermost first: absolute boxes escape those
 * below their containing block and fixed boxes escape all of them (Blink's containing-block
 * chain). The same chain decides which scroll containers move the element. */
fn clipping_ancestors<'a>(tree: &Tree<'a>, node: &'a Node) -> EngineResult<Vec<&'a Node>> {
    let mut escaping = match node.style.position {
        Position::Fixed => return Ok(Vec::new()),
        Position::Absolute => true,
        _ => false,
    };
    let mut chain = Vec::new();
    for ancestor in tree.ancestors(node)? {
        let style = &ancestor.style;
        if escaping && (style.position != Position::Static || style.transform.is_some()) {
            escaping = false;
        }
        if !escaping && (style.overflow != Overflow::Visible || style.contain_paint) {
            chain.push(ancestor);
        }
        match style.position {
            Position::Absolute => escaping = true,
            Position::Fixed => break,
            _ => {}
        }
    }
    Ok(chain)
}
/** Ancestors that clip this element. Scroll containers clip their padding box where it paints. */
fn overflow_clip(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
) -> EngineResult<Option<ClipRect>> {
    let mut result: Option<ClipRect> = None;
    for ancestor in clipping_ancestors(tree, node)? {
        if let Some(rect) = clip_box(tree, items, ancestor)? {
            result = Some(match result {
                Some(inner) => inner.intersect(&rect),
                None => rect,
            });
        }
    }
    // A clipping box's own text stays inside its border box too, so screen-reader-only text stays unseen.
    if node.style.overflow != Overflow::Visible
        && let Some(index) = tree.boxes.get(node.local_id).copied().flatten()
        && !items[index].lines.is_empty()
    {
        let item = &items[index];
        let own = ClipRect {
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            radius: [0.0; 4],
        };
        let matrix = painted(tree, items, node)?;
        let own = super::display::transform_rect(&own, Some(matrix));
        result = Some(match result {
            Some(inner) => inner.intersect(&own),
            None => own,
        });
    }
    Ok(result)
}
/**
 * A clipping ancestor's overflow clip rect in document space, after Blink's
 * LayoutBox::OverflowClipRect: the padding box; on an axis left `visible` beside `clip` it does
 * not clip at all; and an `overflow: clip` or paint-contained box that is not a scroll container
 * moves the edge to its `overflow-clip-margin` reference box, outset by the margin. Rotated
 * boxes clip to their bounding box.
 */
fn clip_box(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
) -> EngineResult<Option<ClipRect>> {
    let Some(index) = tree.boxes.get(node.local_id).copied().flatten() else {
        return Ok(None);
    };
    let item = &items[index];
    let style = &node.style;
    let border = sizing::border_sides(style);
    let mut outsets = border.map(|side| -side);
    // ponytail: overflow clips round with the horizontal radii; elliptical clip corners come
    // when a page clips content to them.
    let mut radius = inner_radius(radii(style, item.width, item.height)[0], border);
    if let Some(margin) = clip_margin(style) {
        match style.overflow_clip_box {
            1 => outsets = [0.0; 4],
            2 => outsets = std::array::from_fn(|side| outsets[side] - style.padding[side]),
            _ => {}
        }
        outsets = outsets.map(|side| side + margin);
        // Blink outsets the padding-box rounded rect with corner correction
        // (AdjustRoundedClipForOverflowClipMargin), by each corner's horizontal outset.
        let spread: [f64; 4] = std::array::from_fn(|side| outsets[side] + border[side]);
        let size = [
            item.width - border[1] - border[3],
            item.height - border[0] - border[2],
        ];
        let [_, right, _, left] = spread;
        let corner = [left, right, right, left];
        radius = std::array::from_fn(|i| outset_radius(radius[i], corner[i], size));
    }
    let [top, right, bottom, left] = outsets;
    let mut rect = ClipRect {
        x: item.x - left,
        y: item.y - top,
        width: (item.width + left + right).max(0.0),
        height: (item.height + top + bottom).max(0.0),
        radius: [0.0; 4],
    };
    // `clip` on one axis with `visible` on the other clips only the first (Blink's
    // ApplyVisibleOverflowToClipRect); `visible` beside any other value computes to `auto`.
    // Half the layout extent each way keeps the span inside display validation's bound.
    let open = crate::constants::LAYOUT_EXTENT / 2.0;
    let visible = |axis: Overflow, other: Overflow| {
        axis == Overflow::Visible && other == Overflow::Clip && !style.contain_paint
    };
    let open_x = visible(style.overflow_x, style.overflow_y);
    let open_y = visible(style.overflow_y, style.overflow_x);
    if open_x {
        (rect.x, rect.width) = (-open, open * 2.0);
    }
    if open_y {
        (rect.y, rect.height) = (-open, 2.0 * open);
    }
    let matrix = painted(tree, items, node)?;
    let mut rect = super::display::transform_rect(&rect, Some(matrix));
    // Blink builds the inner border-radius clip only for boxes clipping both axes, so a box that
    // clips one axis clips square (rounded-overflow-clip-visible).
    if matrix[..4] == IDENTITY[..4] && !open_x && !open_y {
        rect.radius = radius;
    }
    Ok(Some(rect))
}
/** The `overflow-clip-margin` that applies to a box (Blink's ShouldApplyOverflowClipMargin):
 * only `overflow: clip` on both axes or paint containment, never a scroll container. */
fn clip_margin(style: &crate::document::ComputedStyle) -> Option<f64> {
    let clip_both = style.overflow_x == Overflow::Clip && style.overflow_y == Overflow::Clip;
    let scroller = matches!(
        style.overflow,
        Overflow::Auto | Overflow::Scroll | Overflow::Hidden
    );
    let effect = style.overflow_clip_margin != 0.0 || style.overflow_clip_box != 0;
    ((clip_both || style.contain_paint) && !scroller && effect)
        .then_some(style.overflow_clip_margin)
}
/** A corner radius outset by `outset` on a box of `size` (css-backgrounds-3 corner shaping, as
 * Blink's ComputeOutsetAdjustedBorderRadius): a small radius grows less than the outset, by the
 * cubic ratio, unless the corner already covers over half its edge. Never negative. */
fn outset_radius(radius: f64, outset: f64, [width, height]: [f64; 2]) -> f64 {
    if radius <= 0.0 || outset == 0.0 || width <= 0.0 || height <= 0.0 {
        return radius.max(0.0);
    }
    let coverage = 2.0 * (radius / width).min(radius / height);
    if radius > outset || coverage > 1.0 {
        return (radius + outset).max(0.0);
    }
    let ratio = radius / outset;
    radius + outset * (1.0 - (1.0 - ratio).powi(3) * (1.0 - coverage.powi(3)))
}
/** Inner padding-edge radii shrink by the adjacent border widths. */
fn inner_radius(outer: [f64; 4], [top, right, bottom, left]: [f64; 4]) -> [f64; 4] {
    [
        (outer[0] - top.max(left)).max(0.0),
        (outer[1] - top.max(right)).max(0.0),
        (outer[2] - bottom.max(right)).max(0.0),
        (outer[3] - bottom.max(left)).max(0.0),
    ]
}
/** CSS scales all radii down together when adjacent corners would overlap. */
fn radii(style: &crate::document::ComputedStyle, width: f64, height: f64) -> [[f64; 4]; 2] {
    let (width, height) = (width.max(0.0), height.max(0.0));
    let horizontal: [f64; 4] = std::array::from_fn(|i| {
        if style.radius_percent[i] > 0.0 {
            (style.radius_px[i] + width * style.radius_percent[i] / 100.0).max(0.0)
        } else {
            style.radius[i].max(0.0)
        }
    });
    let vertical: [f64; 4] = std::array::from_fn(|i| match style.radius_y[i] {
        Some((px, percent)) => (px + height * percent / 100.0).max(0.0),
        None => horizontal[i],
    });
    let [[tl, tr, br, bl], [vtl, vtr, vbr, vbl]] = [horizontal, vertical];
    // CSS Backgrounds 5.5: radii on one side that overlap scale down together.
    let mut scale: f64 = 1.0;
    for (sum, side) in [
        (tl + tr, width),
        (bl + br, width),
        (vtl + vbl, height),
        (vtr + vbr, height),
    ] {
        if sum > 0.0 {
            scale = scale.min(side / sum);
        }
    }
    [horizontal, vertical].map(|axis| axis.map(|value| value * scale.max(0.0)))
}
/** Radius, shadows and gradient layers resolved against the item's final size. */
fn decorate(item: &mut DisplayItem, node: &Node) {
    let style = &node.style;
    let [horizontal, vertical] = radii(style, item.width, item.height);
    item.radius = horizontal;
    item.radius_y = (vertical != horizontal).then_some(vertical);
    if item.tag == "img" || super::flow::frame(node) {
        item.overflow_visible =
            style.overflow_x == Overflow::Visible && style.overflow_y == Overflow::Visible;
        item.object_fit = style.object_fit;
        let [ox, oy] = style.object_position;
        item.object_position = [ox.px, ox.percent, oy.px, oy.percent];
        let [bt, br, bb, bl] = sizing::border_sides(style);
        let [pt, pr, pb, pl] = style.padding;
        let (top, right, bottom, left) = (bt + pt, br + pr, bb + pb, bl + pl);
        if (top, right, bottom, left) != (0.0, 0.0, 0.0, 0.0) {
            item.content_box = Some([
                item.x + left,
                item.y + top,
                (item.width - left - right).max(0.0),
                (item.height - top - bottom).max(0.0),
            ]);
        }
    }
    item.shadows = style
        .shadows
        .iter()
        .filter(|_| !style.hidden)
        .map(|shadow| DisplayShadow {
            x: shadow.x,
            y: shadow.y,
            blur: shadow.blur.max(0.0),
            spread: shadow.spread,
            color: match (shadow.current, &style.color) {
                (true, Some(color)) => color.clone(),
                _ => shadow.color.clone(),
            },
            inset: shadow.inset,
        })
        .collect();
    item.backgrounds = style
        .backgrounds
        .iter()
        .enumerate()
        // A `visibility: hidden` box (a generated one still lays out) paints no layers.
        .filter(|_| !style.masked && !style.hidden)
        .map(|(index, layer)| tiled((index, layer), style, item.width, item.height))
        .collect();
    key_layers(&mut item.backgrounds, |layer| {
        (layer > 0).then(|| crate::document::layer_key(item.local_id, layer))
    });
    item.mask = style.mask.as_ref().map(|mask| {
        let layer = (mask.size, mask.position, mask.repeat);
        image_layer(&mask.url, layer, item.width, item.height)
    });
    super::blur::glow(item, style.filter_blur);
}
/** A background layer of an item `width` by `height`: a gradient spans the tile its
 * `background-size`, `-position` and `-repeat` place (`gradient_tile`), an image as `background`
 * resolves it. */
pub(super) fn tiled(
    (index, layer): (usize, &BackgroundLayer),
    style: &crate::document::ComputedStyle,
    width: f64,
    height: f64,
) -> DisplayBackground {
    let tile = match layer {
        BackgroundLayer::Image { url } => {
            // Each image layer takes its own entry of the size, position and repeat lists;
            // percentages size against the positioning area (`background-origin`'s box).
            let geometry = crate::document::style::background_layers::geometry(style, index);
            let [top, right, bottom, left] = super::gradient_tile::origin(style);
            let area = (
                (width - left - right).max(0.0),
                (height - top - bottom).max(0.0),
            );
            return image_layer(url, geometry, area.0, area.1);
        }
        _ => super::gradient_tile::tile(style, index, (width, height)),
    };
    let Some(tile) = tile else {
        return background(layer, style, width, height);
    };
    background(layer, style, tile.width, tile.height).with_tile(tile)
}
/** Gives each image layer of `layers` the key `key` returns for its place among the image
 * layers (counted from 0), so painters draw it with its own fetched image. */
pub(super) fn key_layers(layers: &mut [DisplayBackground], key: impl Fn(usize) -> Option<usize>) {
    let images = layers.iter_mut().filter_map(|layer| match layer {
        DisplayBackground::Image { key, .. } => Some(key),
        _ => None,
    });
    for (layer, slot) in images.enumerate() {
        *slot = key(layer);
    }
}
/** Gradient geometry per CSS Images: the line through the center whose ends touch the far
 * corners, with a `to <corner>` angle taken from the box so the 50% line joins the other two
 * corners, as Blink's `CSSLinearGradientValue` computes it. */
pub(super) fn background(
    layer: &BackgroundLayer,
    style: &crate::document::ComputedStyle,
    width: f64,
    height: f64,
) -> DisplayBackground {
    match layer {
        BackgroundLayer::Linear {
            angle,
            stops,
            corner,
            repeating,
        } => {
            let degrees = corner.map_or(*angle, |[sx, sy]| {
                (f64::from(sx) * height)
                    .atan2(-f64::from(sy) * width)
                    .to_degrees()
            });
            let radians = degrees.to_radians();
            let (dx, dy) = (radians.sin(), -radians.cos());
            let half = (width * dx.abs() + height * dy.abs()) / 2.0;
            let (cx, cy) = (width / 2.0, height / 2.0);
            DisplayBackground::Linear {
                x0: cx - dx * half,
                y0: cy - dy * half,
                x1: cx + dx * half,
                y1: cy + dy * half,
                stops: resolve_stops(stops, half * 2.0, *repeating),
                tile: None,
            }
        }
        BackgroundLayer::Radial {
            stops,
            shape,
            repeating,
        } => {
            let [cx, cy, rx, ry] = shape.geometry(width, height);
            DisplayBackground::Radial {
                cx,
                cy,
                rx,
                ry,
                stops: resolve_stops(stops, rx, *repeating),
                tile: None,
            }
        }
        BackgroundLayer::Image { url } => {
            // Percentages size against the positioning area, the padding box by default.
            let [top, right, bottom, left] = sizing::border_sides(style);
            image_layer(
                url,
                (
                    style.background_size,
                    style.background_position,
                    style.background_repeat,
                ),
                (width - left - right).max(0.0),
                (height - top - bottom).max(0.0),
            )
        }
    }
}
/** An image layer (background or mask) with its size rule, position and tiling resolved
against the box. */
fn image_layer(
    url: &str,
    (size, position, repeat): (
        crate::document::style::BackgroundSize,
        [crate::document::style::Extent; 2],
        crate::document::style::BackgroundRepeat,
    ),
    width: f64,
    height: f64,
) -> DisplayBackground {
    use super::display::ImageSize;
    use crate::document::style::{BackgroundRepeat, BackgroundSize};
    let size = match size {
        BackgroundSize::Auto => ImageSize::Auto,
        BackgroundSize::Cover => ImageSize::Cover,
        BackgroundSize::Contain => ImageSize::Contain,
        BackgroundSize::Explicit(w, h) => ImageSize::Fixed {
            width: w.and_then(|w| w.resolve(Some(width))),
            height: h.and_then(|h| h.resolve(Some(height))),
        },
    };
    let [x, y] = position;
    use crate::document::style::RepeatStyle::{self, NoRepeat, Repeat, Round, Space};
    let [rx, ry]: [RepeatStyle; 2] = match repeat {
        BackgroundRepeat::Repeat => [Repeat, Repeat],
        BackgroundRepeat::NoRepeat => [NoRepeat, NoRepeat],
        BackgroundRepeat::RepeatX => [Repeat, NoRepeat],
        BackgroundRepeat::RepeatY => [NoRepeat, Repeat],
        BackgroundRepeat::Axes(x, y) => [x, y],
    };
    DisplayBackground::Image {
        url: url.to_owned(),
        size,
        position: [x.px, x.percent, y.px, y.percent],
        repeat: [rx != NoRepeat, ry != NoRepeat],
        space: [rx == Space, ry == Space],
        round: [rx == Round, ry == Round],
        area: None,
        key: None,
    }
}
/** Stops a repeating gradient may expand to before the pattern stops tiling. */
const REPEATED_STOPS: usize = 1024;
/**
 * Stops as fractions of a gradient line `length` px long, per CSS Images' color stop fixup:
 * lengths resolve against the line, a missing first or last position is 0% or 100%, a position
 * below an earlier one moves up to it, and runs of missing positions spread evenly. A repeating
 * gradient then tiles its first-to-last span along the line, and the ramp is clipped to the line
 * with interpolated end colors, as Blink's `Gradient` does with its spread method.
 */
fn resolve_stops(stops: &[ColorStop], length: f64, repeating: bool) -> Vec<GradientStop> {
    let mut positions = stops
        .iter()
        .map(|stop| match (stop.position, stop.px) {
            (None, None) => None,
            (fraction, px) => {
                let px = px.unwrap_or(0.0);
                let extra = if length > 0.0 { px / length } else { 0.0 };
                Some(fraction.unwrap_or(0.0) + extra)
            }
        })
        .collect::<Vec<_>>();
    fix_up(&mut positions);
    let mut ramp: Vec<(f64, [f64; 4])> = stops
        .iter()
        .zip(positions)
        .map(|(stop, at)| (at.unwrap_or(0.0), color(&stop.color)))
        .collect();
    if repeating {
        ramp = tile(&ramp);
    }
    clip_ramp(&ramp)
        .into_iter()
        .map(|(position, rgba)| GradientStop {
            color: super::svg::literal(rgba),
            position,
        })
        .collect()
}
/** Fills missing positions: ends at 0 and 1, nondecreasing, gaps spread evenly. */
fn fix_up(positions: &mut [Option<f64>]) {
    let count = positions.len();
    if let Some(first) = positions.first_mut() {
        first.get_or_insert(0.0);
    }
    if let Some(last) = positions.last_mut() {
        last.get_or_insert(1.0);
    }
    let mut floor = f64::MIN;
    for slot in positions.iter_mut().flatten() {
        floor = floor.max(*slot);
        *slot = floor;
    }
    let mut index = 0;
    while index < count {
        if positions[index].is_some() {
            index += 1;
            continue;
        }
        let start = index - 1;
        let end = (index..count)
            .find(|&i| positions[i].is_some())
            .unwrap_or(count - 1);
        let (from, to) = (
            positions[start].unwrap_or(0.0),
            positions[end].unwrap_or(1.0),
        );
        for (step, slot) in positions[index..end].iter_mut().enumerate() {
            *slot = Some(from + (to - from) * (step + 1) as f64 / (end - start) as f64);
        }
        index = end;
    }
}
/** Premultiplied RGBA of a stop color; a palette keyword the painter resolves counts as black. */
fn color(literal: &str) -> [f64; 4] {
    let [r, g, b, a] = super::svg::channels(literal).unwrap_or([0.0, 0.0, 0.0, 1.0]);
    [r * a, g * a, b * a, a]
}
/** The ramp repeated so its first-to-last span covers [0, 1]; a zero span paints the last color. */
fn tile(ramp: &[(f64, [f64; 4])]) -> Vec<(f64, [f64; 4])> {
    let (Some(first), Some(last)) = (ramp.first(), ramp.last()) else {
        return Vec::new();
    };
    let period = last.0 - first.0;
    if period <= f64::EPSILON {
        return vec![(0.0, last.1), (1.0, last.1)];
    }
    let mut copy = (-first.0 / period).floor() - 1.0;
    let mut tiled = Vec::new();
    while first.0 + copy * period <= 1.0 && tiled.len() < REPEATED_STOPS {
        let shift = copy * period;
        tiled.extend(ramp.iter().map(|(at, rgba)| (at + shift, *rgba)));
        copy += 1.0;
    }
    tiled
}
/** The ramp restricted to [0, 1], with the colors at 0 and 1 interpolated where it crosses. */
fn clip_ramp(ramp: &[(f64, [f64; 4])]) -> Vec<(f64, [f64; 4])> {
    let at = |t: f64| -> [f64; 4] {
        let after = ramp.iter().position(|(position, _)| *position > t);
        match after {
            None => ramp.last().map_or([0.0; 4], |stop| stop.1),
            Some(0) => ramp[0].1,
            Some(next) => {
                let ((p0, c0), (p1, c1)) = (ramp[next - 1], ramp[next]);
                let f = if p1 > p0 { (t - p0) / (p1 - p0) } else { 1.0 };
                std::array::from_fn(|i| c0[i] + (c1[i] - c0[i]) * f)
            }
        }
    };
    let inside = ramp
        .iter()
        .filter(|(position, _)| (0.0..=1.0).contains(position));
    let mut clipped = Vec::new();
    if ramp.first().is_some_and(|stop| stop.0 > 0.0) || ramp.iter().any(|stop| stop.0 < 0.0) {
        clipped.push((0.0, at(0.0)));
    }
    clipped.extend(inside.copied());
    if ramp.last().is_some_and(|stop| stop.0 > 1.0) {
        clipped.push((1.0, at(1.0)));
    }
    // Of stops sharing one position only the first and last matter: they make the hard edge.
    let mut kept: Vec<(f64, [f64; 4])> = Vec::with_capacity(clipped.len());
    for (index, stop) in clipped.iter().enumerate() {
        let same = |other: Option<&(f64, [f64; 4])>| other.is_some_and(|o| o.0 == stop.0);
        if !(same(index.checked_sub(1).map(|i| &clipped[i])) && same(clipped.get(index + 1))) {
            kept.push(*stop);
        }
    }
    kept
}
/** Pure translations move the item's geometry so agent bounds equal paint; anything else is carried. */
fn place(item: &mut DisplayItem, matrix: Matrix) {
    if matrix == IDENTITY {
        return;
    }
    if matrix[..4] != IDENTITY[..4] {
        item.transform = Some(matrix);
        return;
    }
    let (dx, dy) = (matrix[4], matrix[5]);
    item.x += dx;
    item.y += dy;
    if let Some(content) = &mut item.content_box {
        content[0] += dx;
        content[1] += dy;
    }
    for line in item.lines.iter_mut().chain(item.validation.iter_mut()) {
        line.x += dx;
        line.y += dy;
    }
    for path in &mut item.vector {
        path.translate(dx, dy);
    }
}
