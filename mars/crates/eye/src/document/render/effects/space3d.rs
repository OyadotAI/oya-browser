/**
 * The 3D paint pass. A box in a 3D context (a 3D transform, a `perspective` ancestor, or a hidden
 * back face) is placed by composing its own 4x4 matrix through each ancestor's perspective and
 * preserve-3d rules (CSS Transforms 2), then projecting its layout corners. The projection gives
 * the exact quad for hit testing and agent geometry, and the affine that paints it. Painting
 * stays 2D: a non-affine projection paints through the affine fitted to its three corners.
 */
use super::{Matrix, Tree, generated_style, offset};
use crate::{
    constants::LAYOUT_EXTENT,
    document::{
        ComputedStyle, DisplayItem, Node, Overflow,
        matrix::{
            IDENTITY4, Matrix4, fit_affine, flatten, from_affine, is_back_facing, multiply,
            perspective, project, translation,
        },
        style::default_transform_origin,
    },
    protocol::EngineResult,
};

/** The percentage whose full value is the box's size. */
const PERCENT: f64 = 100.0;
/** The corners of a box in quad order: top-left, top-right, bottom-right, bottom-left. */
const TOP_RIGHT: usize = 1;
/** Bottom-left, the third corner the paint affine is fitted through. */
const BOTTOM_LEFT: usize = 3;

/** A box's place after the 3D pass. */
pub(super) struct Placed {
    /// The affine that paints the box: exact when the projection is affine, else fitted.
    pub affine: Matrix,
    /// The projected corners in quad order; the exact area that hit testing and bounds use.
    pub quad: [[f64; 2]; 4],
    /// Hidden by `backface-visibility` or by a corner behind the eye: paints and hits nothing.
    pub hidden: bool,
}

/** One box on the path from the context root: its local matrix and what its children inherit. */
struct Level {
    /// The box's offset and transform about its origin, in its parent's coordinates.
    local: Matrix4,
    /// The perspective its children see, applied in this box's coordinates; identity without one.
    perspective: Matrix4,
    /// Whether children share this box's 3D rendering context instead of flattening into it.
    preserve: bool,
    /// Whether the box's back face is hidden, which hides its flattened subtree with it.
    backface_hidden: bool,
    /// The box's layout rectangle `[x, y, w, h]`, whose corners must project for it to be seen.
    rect: [f64; 4],
}

/** What a box's 3D context hands its descendants, memoized per identity so a deep tree is
walked once rather than once per box. */
#[derive(Clone, Copy)]
pub(super) struct Space {
    /// Whether this box or an ancestor is in 3D or hides a back face, so the box needs the pass.
    needed: bool,
    /// The box's full matrix from the root: every ancestor's offset, transform and perspective.
    full: Matrix4,
    /// What children compose onto: `full`, flattened when the box flattens, then its perspective.
    down: Matrix4,
    /// The box's matrix relative to its 3D rendering context root, which is where its facing is
    /// judged. A flattening ancestor is a root, so only its perspective carries into this.
    context: Matrix4,
    /// What the box's children compose onto within the same rendering context.
    context_down: Matrix4,
    /// Whether a flattening ancestor's hidden back face hides this box with it.
    hidden: bool,
    /// Whether this box's descendants are hidden: `hidden`, or its own flattening face turned away.
    hidden_down: bool,
}

/** The root's context: nothing in 3D yet. */
const ROOT_SPACE: Space = Space {
    needed: false,
    full: IDENTITY4,
    down: IDENTITY4,
    context: IDENTITY4,
    context_down: IDENTITY4,
    hidden: false,
    hidden_down: false,
};

/** A box's space, computed from its nearest memoized ancestor down and memoized on the way. */
fn space(tree: &mut Tree<'_>, items: &[DisplayItem], node: &Node) -> EngineResult<Space> {
    let mut pending = vec![node];
    let mut base = ROOT_SPACE;
    let mut current = node.parent;
    while let Some(local) = current {
        if let Some(found) = tree.spaces.get(local).copied().flatten() {
            base = found;
            break;
        }
        let ancestor = tree.node(local)?;
        pending.push(ancestor);
        current = ancestor.parent;
    }
    for anchor in pending.into_iter().rev() {
        base = child_space(&base, &level(tree, items, anchor)?, &anchor.style);
        if let Some(slot) = tree.spaces.get_mut(anchor.local_id) {
            *slot = Some(base);
        }
    }
    Ok(base)
}

/** A box's space from its parent's. Flattening drops the z output (CSS Transforms 2), which
for points in a plane is a left multiply by `flatten(IDENTITY4)`, so the chain is one product. */
fn child_space(parent: &Space, level: &Level, style: &ComputedStyle) -> Space {
    let full = multiply(&parent.down, &level.local);
    let flat = match level.preserve {
        true => full,
        false => flatten_after(&full),
    };
    let context = multiply(&parent.context_down, &level.local);
    let own_face =
        level.backface_hidden && !level.preserve && turned_away(&full, &context, level.rect);
    Space {
        needed: parent.needed
            || style.transform_4.is_some()
            || style.perspective.is_some()
            || style.backface_hidden,
        full,
        down: multiply(&flat, &level.perspective),
        context,
        context_down: multiply(&context_base(level.preserve, &context), &level.perspective),
        hidden: parent.hidden_down,
        hidden_down: parent.hidden_down || own_face,
    }
}

/** What a box's children start from within its rendering context: the box's own matrix when it
preserves 3D, so they share its context, and identity when it flattens, which ends the context. */
fn context_base(preserve: bool, context: &Matrix4) -> Matrix4 {
    match preserve {
        true => *context,
        false => IDENTITY4,
    }
}

/** `full` followed by dropping z: the matrix a flattening box hands its subtree. */
fn flatten_after(full: &Matrix4) -> Matrix4 {
    multiply(full, &flatten(&IDENTITY4))
}

/** Places a box in the 3D pass, or None when nothing in its path needs one. */
pub(super) fn placed(
    tree: &mut Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    item: &DisplayItem,
) -> EngineResult<Option<Placed>> {
    let space = space(tree, items, node)?;
    let generated_3d = generated_style(node, item).is_some_and(|style| style.transform_4.is_some());
    if !space.needed && !generated_3d {
        return Ok(None);
    }
    let [sx, sy] = super::scrolled(tree, node)?;
    let mut full = multiply(&translation([-sx, -sy, 0.0]), &space.full);
    let mut context = space.context;
    if let Some(style) = generated_style(node, item) {
        let own = own_matrix(style, Some(item));
        full = multiply(&full, &own);
        context = multiply(&context, &own);
    }
    let mut placed = project_box(item, &full, &context, node.style.backface_hidden);
    // A flattening ancestor whose hidden back face turned away paints this box in its own plane,
    // so the box hides with it (CSS Transforms 2, backface-visibility).
    placed.hidden |= space.hidden;
    Ok(Some(placed))
}

/** Whether a flattening box's back is turned away from the viewer: its own matrix shows its back,
or a corner has no screen position, which hides it too. */
fn turned_away(full: &Matrix4, context: &Matrix4, [x, y, w, h]: [f64; 4]) -> bool {
    let corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    match all_some(&corners.map(|corner| project(full, corner))) {
        Some(_) => is_back_facing(context),
        None => true,
    }
}

/** The projected corners, the paint affine fitted through three of them, and the facing, which
is judged by the box's matrix within its rendering context rather than its projected area. */
fn project_box(
    item: &DisplayItem,
    full: &Matrix4,
    context: &Matrix4,
    backface_hidden: bool,
) -> Placed {
    let (x, y, w, h) = (item.x, item.y, item.width, item.height);
    let corners = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    let projected = corners.map(|corner| project(full, corner));
    let Some(quad) = all_some(&projected).filter(within_extent) else {
        return Placed {
            affine: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            quad: corners,
            hidden: true,
        };
    };
    Placed {
        affine: fit_affine([x, y, w, h], [quad[0], quad[TOP_RIGHT], quad[BOTTOM_LEFT]]),
        quad,
        hidden: backface_hidden && is_back_facing(context),
    }
}

/** Whether every projected coordinate fits the layout extent that painters accept. A corner
nearly on the eye plane projects arbitrarily far away; such a box is treated as hidden rather
than failing the whole page. */
fn within_extent(quad: &[[f64; 2]; 4]) -> bool {
    quad.iter()
        .flatten()
        .all(|value| value.abs() <= LAYOUT_EXTENT)
}

/** Every corner's projection, or None when any corner has no screen position. */
fn all_some(points: &[Option<[f64; 2]>; 4]) -> Option<[[f64; 2]; 4]> {
    let mut quad = [[0.0; 2]; 4];
    for (slot, point) in quad.iter_mut().zip(points) {
        *slot = (*point)?;
    }
    Some(quad)
}

/** One box's level: its offset and own transform, its perspective, and its preserve-3d flag. */
fn level(tree: &Tree<'_>, items: &[DisplayItem], anchor: &Node) -> EngineResult<Level> {
    let own = own_box(tree, items, anchor);
    let [x, y] = offset(tree, items, anchor);
    Ok(Level {
        local: multiply(&translation([x, y, 0.0]), &own_matrix(&anchor.style, own)),
        perspective: perspective_matrix(&anchor.style, own),
        preserve: preserves_3d(&anchor.style),
        backface_hidden: anchor.style.backface_hidden,
        rect: own.map_or([0.0; 4], |item| [item.x, item.y, item.width, item.height]),
    })
}

/** The box's own item, the layout geometry its origins and percentages resolve against. */
fn own_box<'a>(
    tree: &Tree<'_>,
    items: &'a [DisplayItem],
    anchor: &Node,
) -> Option<&'a DisplayItem> {
    tree.boxes
        .get(anchor.local_id)
        .copied()
        .flatten()
        .map(|index| &items[index])
}

/** `transform-style: preserve-3d` takes effect only where no grouping property flattens it:
overflow other than visible, or opacity below one (css-transforms-2, grouping properties). */
fn preserves_3d(style: &ComputedStyle) -> bool {
    style.preserve_3d && style.overflow == Overflow::Visible && style.opacity == 1.0
}

/** The box's transform about its transform origin, with its percentage translation added. */
fn own_matrix(style: &ComputedStyle, own: Option<&DisplayItem>) -> Matrix4 {
    let transform = style
        .transform_4
        .unwrap_or_else(|| style.transform.map_or(IDENTITY4, from_affine));
    let (w, h) = own.map_or((0.0, 0.0), |item| (item.width, item.height));
    let mut moved = transform;
    let [cross_y, cross_x] = style.translate_percent_cross;
    moved[12] += (w * style.translate_percent[0] + h * cross_x) / PERCENT;
    moved[13] += (w * cross_y + h * style.translate_percent[1]) / PERCENT;
    let [depth_w, depth_h] = style.translate_percent_depth;
    moved[14] += (w * depth_w + h * depth_h) / PERCENT;
    let origin = origin_point(
        style
            .transform_origin
            .unwrap_or_else(default_transform_origin),
        style.transform_origin_z,
        own,
    );
    multiply(
        &translation(origin),
        &multiply(&moved, &translation(origin.map(|v| -v))),
    )
}

/** A box's `[x, y, z]` origin in its layout coordinates, from `[x px, x %, y px, y %]` of the
border box plus the z of a third value. */
fn origin_point(origin: [f64; 4], z: f64, own: Option<&DisplayItem>) -> [f64; 3] {
    let [ox, ox_percent, oy, oy_percent] = origin;
    let (x, y, w, h) = own.map_or((0.0, 0.0, 0.0, 0.0), |item| {
        (item.x, item.y, item.width, item.height)
    });
    [
        x + ox + w * ox_percent / PERCENT,
        y + oy + h * oy_percent / PERCENT,
        z,
    ]
}

/** `perspective` on a box: a matrix about its perspective origin, which its children inherit. */
fn perspective_matrix(style: &ComputedStyle, own: Option<&DisplayItem>) -> Matrix4 {
    let Some(distance) = style.perspective else {
        return IDENTITY4;
    };
    let [px, px_percent, py, py_percent] = style
        .perspective_origin
        .unwrap_or_else(default_transform_origin);
    let (x, y, w, h) = own.map_or((0.0, 0.0, 0.0, 0.0), |item| {
        (item.x, item.y, item.width, item.height)
    });
    let origin = [
        x + px + w * px_percent / PERCENT,
        y + py + h * py_percent / PERCENT,
        0.0,
    ];
    multiply(
        &translation(origin),
        &multiply(&perspective(distance), &translation(origin.map(|v| -v))),
    )
}
