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
/** One step of a paint-order key: bucket within a stacking layer, z-index, then document order. */
type Step = (u8, i32, usize);
/** A layer root's own background and border paint first. */
const OWN: u8 = 0;
/** Negative z-index layers paint below in-flow content. */
const NEGATIVE: u8 = 1;
/** In-flow, non-positioned descendants paint in document order. */
const FLOW: u8 = 2;
/** Positioned boxes with z-index auto or 0, and other zero-level stacking contexts. */
const ZERO: u8 = 3;
/** Positive z-index layers paint on top. */
const POSITIVE: u8 = 4;

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
        let key = key(&tree, node)?;
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
            .find(|step| step.0 != FLOW && step.0 != OWN)
            .map_or(0, |s| s.1);
        item.clip = clip;
        place(&mut item, matrix);
        ordered.push(item);
    }
    Ok(ordered)
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
/** Positioned, translucent and transformed boxes form layers painted as a unit. */
fn layer_root(node: &Node) -> bool {
    node.style.position != Position::Static
        || node.style.opacity < 1.0
        || node.style.transform.is_some()
        || node.style.translate_percent != [0.0; 2]
}
/** Where a layer root paints inside its parent layer. */
fn layer_step(tree: &Tree<'_>, node: &Node) -> Step {
    let z = if node.style.position == Position::Static {
        0
    } else {
        node.style.z_index.unwrap_or(0)
    };
    let bucket = match z {
        ..0 => NEGATIVE,
        0 => ZERO,
        _ => POSITIVE,
    };
    (bucket, z, tree.order(node.local_id))
}
/** The paint-order key: each enclosing layer from the outside in, then this item's place in its layer.
 * ponytail: a positioned z-index:auto box is treated as atomic like a real stacking context; split it
 * when interleaving its positioned descendants with siblings matters. */
fn key(tree: &Tree<'_>, node: &Node) -> EngineResult<Vec<Step>> {
    let mut chain = tree
        .ancestors(node)?
        .into_iter()
        .filter(|ancestor| layer_root(ancestor))
        .map(|ancestor| layer_step(tree, ancestor))
        .collect::<Vec<_>>();
    chain.reverse();
    if layer_root(node) {
        chain.push(layer_step(tree, node));
        chain.push((OWN, 0, 0));
    } else {
        chain.push((FLOW, 0, tree.order(node.local_id)));
    }
    Ok(chain)
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
    if matches!(style.position, Position::Relative | Position::Sticky) {
        // ponytail: sticky stays at its relative offset; pin it to the viewport when scrolling is layered.
        let parent_box = node
            .parent
            .and_then(|parent| tree.boxes.get(parent).copied().flatten())
            .map(|index| &items[index])
            .or(own);
        let (pw, ph) = parent_box.map_or((0.0, 0.0), |item| (item.width, item.height));
        let inset = |side: usize, basis: f64| -> Option<f64> {
            if style.inset[side].is_some() || style.inset_percent[side] != 0.0 {
                Some(style.inset[side].unwrap_or(0.0) + basis * style.inset_percent[side] / 100.0)
            } else {
                None
            }
        };
        let (top, right, bottom, left) = (inset(0, ph), inset(1, pw), inset(2, ph), inset(3, pw));
        matrix[4] = left.or(right.map(|right| -right)).unwrap_or(0.0);
        matrix[5] = top.or(bottom.map(|bottom| -bottom)).unwrap_or(0.0);
    }
    if style.transform.is_some() || style.translate_percent != [0.0; 2] {
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
        let about = multiply(
            [1.0, 0.0, 0.0, 1.0, x, y],
            multiply(transform, [1.0, 0.0, 0.0, 1.0, -x, -y]),
        );
        matrix = multiply(matrix, about);
    }
    matrix
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
}
/** Gradient geometry per CSS Images: the line through the center whose ends touch the far corners. */
fn background(
    layer: &BackgroundLayer,
    style: &crate::document::ComputedStyle,
    width: f64,
    height: f64,
) -> DisplayBackground {
    match layer {
        BackgroundLayer::Linear { angle, stops } => {
            let radians = angle.to_radians();
            let (dx, dy) = (radians.sin(), -radians.cos());
            let half = (width * dx.abs() + height * dy.abs()) / 2.0;
            let (cx, cy) = (width / 2.0, height / 2.0);
            DisplayBackground::Linear {
                x0: cx - dx * half,
                y0: cy - dy * half,
                x1: cx + dx * half,
                y1: cy + dy * half,
                stops: resolve_stops(stops),
            }
        }
        BackgroundLayer::Radial { stops, shape } => {
            let [cx, cy, rx, ry] = shape.geometry(width, height);
            DisplayBackground::Radial {
                cx,
                cy,
                rx,
                ry,
                stops: resolve_stops(stops),
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
/** Missing positions spread evenly between known neighbors; positions never decrease. */
fn resolve_stops(stops: &[ColorStop]) -> Vec<GradientStop> {
    let count = stops.len();
    let mut positions = stops.iter().map(|stop| stop.position).collect::<Vec<_>>();
    if let Some(first) = positions.first_mut() {
        first.get_or_insert(0.0);
    }
    if let Some(last) = positions.last_mut() {
        last.get_or_insert(1.0);
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
    let mut floor: f64 = 0.0;
    stops
        .iter()
        .zip(positions)
        .map(|(stop, position)| {
            floor = floor.max(position.unwrap_or(floor).clamp(0.0, 1.0));
            GradientStop {
                color: stop.color.clone(),
                position: floor,
            }
        })
        .collect()
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
