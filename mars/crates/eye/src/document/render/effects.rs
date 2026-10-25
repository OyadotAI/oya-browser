/** After layout: relative offsets, transforms, overflow clips, decorations and stacking paint order. */
use super::{
    ClipRect, DisplayBackground, DisplayItem, DisplayShadow, GradientStop, OpacityGroup, sizing,
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
type Step = (u8, i32, usize, u8);
/** A painting layer's own background, border and text paint first. */
const OWN: u8 = 0;
/** The negative z-order list of a stacking context. */
const NEGATIVE: u8 = 1;
/** Normal-flow block-level descendants: their backgrounds paint in tree order. */
const BLOCK: u8 = 2;
/** Floats, each painted atomically after every block background. */
const FLOAT: u8 = 3;
/** Inline-level content: inline boxes, text runs and atomic inlines, in tree order.
 * ponytail: equal to BLOCK because one display item carries a block's background together with
 * its own text, and text selection reads items in tree order; splitting a block's text into its
 * own item is the upgrade that lets inline content paint over later block backgrounds. */
const INLINE: u8 = BLOCK;
/** The positive z-order list: z-index 0 and auto stacked boxes in tree order, then higher z. */
const POSITIVE: u8 = 5;

/** Read-only facts about the laid-out tree, shared by every per-item computation. */
struct Tree<'a> {
    document: &'a Document,
    positions: Vec<Option<usize>>,
    /// The first item each identity painted, used for transform origins and clip boxes.
    boxes: Vec<Option<usize>>,
    /// Memoized absolute matrices per identity.
    matrices: Vec<Option<Matrix>>,
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
    /** Document order of an identity. */
    fn order(&self, local: usize) -> usize {
        self.positions
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

/// Resolves effects on the laid-out items and returns them in final paint order.
pub(super) fn stack(
    document: &Document,
    items: Vec<DisplayItem>,
) -> EngineResult<Vec<DisplayItem>> {
    let positions = sizing::positions(document);
    let mut boxes = vec![None; positions.len()];
    for (index, item) in items.iter().enumerate() {
        if let Some(slot) = boxes.get_mut(item.local_id)
            && slot.is_none()
        {
            *slot = Some(index);
        }
    }
    let mut tree = Tree {
        document,
        matrices: vec![None; positions.len()],
        positions,
        boxes,
    };
    let mut resolved = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let node = tree.node(item.local_id)?;
        let matrix = absolute(&mut tree, &items, node)?;
        let clip = clip(&mut tree, &items, node)?;
        let key = key(&tree, node, item)?;
        resolved.push((key, index, matrix, clip));
    }
    resolved.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut slots = items.into_iter().map(Some).collect::<Vec<_>>();
    let mut ordered = Vec::with_capacity(slots.len());
    for (key, index, matrix, clip) in resolved {
        let Some(mut item) = slots[index].take() else {
            continue;
        };
        let node = tree.node(item.local_id)?;
        match generated_style(node, &item) {
            Some(style) => {
                let mut generated = node.clone();
                generated.style = style.clone();
                decorate(&mut item, &generated);
            }
            None => decorate(&mut item, node),
        }
        item.stacking_opacity = groups(&tree, node)?;
        item.z = key
            .iter()
            .rev()
            .find(|step| step.0 == NEGATIVE || step.0 == POSITIVE)
            .map_or(0, |s| s.1);
        item.clip = content_clip(node, &item, matrix, clip);
        let matrix = generated_matrix(node, &item, matrix);
        place(&mut item, matrix);
        ordered.push(item);
    }
    Ok(ordered)
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
    let [top, right, bottom, left] = sizing::border_sides(&node.style);
    let [pt, pr, pb, pl] = node.style.padding;
    let content = ClipRect {
        x: item.x + left + pl,
        y: item.y + top + pt,
        width: (item.width - left - right - pl - pr).max(0.0),
        height: (item.height - top - bottom - pt - pb).max(0.0),
        radius: [0.0; 4],
    };
    let content = super::display::transform_rect(&content, Some(matrix));
    Some(match clip {
        Some(outer) => outer.intersect(&content),
        None => content,
    })
}
/** The style of a `::before` or `::after` item, which shares its element's identity. */
pub(super) fn generated_style<'a>(
    node: &'a Node,
    item: &DisplayItem,
) -> Option<&'a crate::document::ComputedStyle> {
    let after = match item.tag.as_str() {
        "::before" => false,
        "::after" => true,
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
/** Floats, atomic inlines and flex and grid items paint all their phases at once, as if they
 * were stacking contexts (Blink's `PaintAllPhasesAtomically`). */
fn atomic(link: &Link<'_>) -> bool {
    link.style.float != crate::document::style::Float::None
        || link.style.inline_block
        || layout_item(link.parent)
}
/** One box on the path from the root to a painted item: its style, its parent's, its order. */
struct Link<'a> {
    style: &'a crate::document::ComputedStyle,
    parent: Option<&'a crate::document::ComputedStyle>,
    order: usize,
    sub: u8,
    text: bool,
    /// The document element, which Blink always makes a stacking context.
    root: bool,
}
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
    for (index, element) in chain.iter().enumerate() {
        path.push(Link {
            style: &element.style,
            parent: index.checked_sub(1).map(|parent| &chain[parent].style),
            order: tree.order(element.local_id),
            sub: 0,
            text: element.tag == "#text",
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
            root: false,
        });
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
    if style.position == Position::Sticky {
        matrix[5] = sticky_offset(tree, items, node, own);
    } else if style.position == Position::Relative {
        let parent_box = node
            .parent
            .and_then(|parent| tree.boxes.get(parent).copied().flatten())
            .map(|index| &items[index])
            .or(own);
        let (pw, ph) = parent_box.map_or((0.0, 0.0), |item| (item.width, item.height));
        let inset = |side: usize, basis: f64| super::positioned::inset(style, side, basis);
        let (top, right, bottom, left) = (inset(0, ph), inset(1, pw), inset(2, ph), inset(3, pw));
        matrix[4] = left.or(right.map(|right| -right)).unwrap_or(0.0);
        matrix[5] = top.or(bottom.map(|bottom| -bottom)).unwrap_or(0.0);
    }
    if transformed(style) {
        matrix = multiply(matrix, own_transform(style, own));
    }
    matrix
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
    let Some(own) = own else {
        return 0.0;
    };
    let parent = node
        .parent
        .and_then(|parent| tree.boxes.get(parent).copied().flatten())
        .map(|index| &items[index]);
    let Some(top) = super::positioned::inset(&node.style, 0, parent.map_or(0.0, |p| p.height))
    else {
        return 0.0;
    };
    let wanted = (top - own.y).max(0.0);
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
    wanted.min(room)
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
    let matrix = absolute(tree, items, node)?;
    Ok(Some(super::display::transform_rect(&own, Some(matrix))))
}
/** Ancestors that clip this element, honoring that absolute boxes escape clips below their containing
 * block and fixed boxes escape all of them. Scroll containers clip at scroll offset 0. */
fn overflow_clip(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
) -> EngineResult<Option<ClipRect>> {
    let mut escaping = match node.style.position {
        Position::Fixed => return Ok(None),
        Position::Absolute => true,
        _ => false,
    };
    let mut result: Option<ClipRect> = None;
    for ancestor in tree.ancestors(node)? {
        let style = &ancestor.style;
        if escaping && (style.position != Position::Static || style.transform.is_some()) {
            escaping = false;
        }
        if !escaping
            && style.overflow != Overflow::Visible
            && let Some(rect) = clip_box(tree, items, ancestor)?
        {
            result = Some(match result {
                Some(inner) => inner.intersect(&rect),
                None => rect,
            });
        }
        match style.position {
            Position::Absolute => escaping = true,
            Position::Fixed => break,
            _ => {}
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
        let matrix = absolute(tree, items, node)?;
        let own = super::display::transform_rect(&own, Some(matrix));
        result = Some(match result {
            Some(inner) => inner.intersect(&own),
            None => own,
        });
    }
    Ok(result)
}
/** A clipping ancestor's padding box in document space; rotated boxes clip to their bounding box. */
fn clip_box(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
) -> EngineResult<Option<ClipRect>> {
    let Some(index) = tree.boxes.get(node.local_id).copied().flatten() else {
        return Ok(None);
    };
    let item = &items[index];
    let [top, right, bottom, left] = sizing::border_sides(&node.style);
    let matrix = absolute(tree, items, node)?;
    let padding = ClipRect {
        x: item.x + left,
        y: item.y + top,
        width: (item.width - left - right).max(0.0),
        height: (item.height - top - bottom).max(0.0),
        radius: [0.0; 4],
    };
    let mut rect = super::display::transform_rect(&padding, Some(matrix));
    if matrix[..4] == IDENTITY[..4] {
        rect.radius = inner_radius(
            radii(&node.style, item.width, item.height),
            [top, right, bottom, left],
        );
    }
    Ok(Some(rect))
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
fn radii(style: &crate::document::ComputedStyle, width: f64, height: f64) -> [f64; 4] {
    let basis = width.min(height).max(0.0);
    let [tl, tr, br, bl] = std::array::from_fn(|i| {
        if style.radius_percent[i] > 0.0 {
            (basis * style.radius_percent[i] / 100.0).max(0.0)
        } else {
            style.radius[i].max(0.0)
        }
    });
    let mut scale: f64 = 1.0;
    for (sum, side) in [
        (tl + tr, width),
        (bl + br, width),
        (tl + bl, height),
        (tr + br, height),
    ] {
        if sum > 0.0 {
            scale = scale.min(side / sum);
        }
    }
    [tl, tr, br, bl].map(|value| value * scale.max(0.0))
}
/** Radius, shadows and gradient layers resolved against the item's final size. */
fn decorate(item: &mut DisplayItem, node: &Node) {
    let style = &node.style;
    item.radius = radii(style, item.width, item.height);
    if item.tag == "img" {
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
        .map(|shadow| DisplayShadow {
            x: shadow.x,
            y: shadow.y,
            blur: shadow.blur.max(0.0),
            spread: shadow.spread,
            color: shadow.color.clone(),
            inset: shadow.inset,
        })
        .collect();
    item.backgrounds = style
        .backgrounds
        .iter()
        .filter(|_| !style.masked)
        .map(|layer| background(layer, style, item.width, item.height))
        .collect();
    item.mask = style.mask.as_ref().map(|mask| {
        let layer = (mask.size, mask.position, mask.repeat);
        image_layer(&mask.url, layer, item.width, item.height)
    });
    super::blur::glow(item, style.filter_blur);
}
/** Gradient geometry per CSS Images: the line through the center whose ends touch the far
 * corners, with a `to <corner>` angle taken from the box so the 50% line joins the other two
 * corners, as Blink's `CSSLinearGradientValue` computes it. */
fn background(
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
            }
        }
        BackgroundLayer::Image { url } => image_layer(
            url,
            (
                style.background_size,
                style.background_position,
                style.background_repeat,
            ),
            width,
            height,
        ),
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
    let repeat = match repeat {
        BackgroundRepeat::Repeat => [true, true],
        BackgroundRepeat::NoRepeat => [false, false],
        BackgroundRepeat::RepeatX => [true, false],
        BackgroundRepeat::RepeatY => [false, true],
    };
    DisplayBackground::Image {
        url: url.to_owned(),
        size,
        position: [x.px, x.percent, y.px, y.percent],
        repeat,
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
