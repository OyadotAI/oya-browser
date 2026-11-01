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
            IDENTITY4, Matrix4, fit_affine, flatten, from_affine, multiply, orientation,
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
}

/** Whether a box needs the 3D pass: some box on its path is in 3D, or its back face may hide it. */
pub(super) fn needed(tree: &Tree<'_>, node: &Node, item: &DisplayItem) -> EngineResult<bool> {
    if node.style.backface_hidden
        || generated_style(node, item).is_some_and(|style| style.transform_4.is_some())
    {
        return Ok(true);
    }
    let mut current = Some(node);
    while let Some(anchor) = current {
        if anchor.style.transform_4.is_some() || anchor.style.perspective.is_some() {
            return Ok(true);
        }
        current = anchor.parent.map(|local| tree.node(local)).transpose()?;
    }
    Ok(false)
}

/** Places a box in the 3D pass, or None when nothing in its path needs one. */
pub(super) fn placed(
    tree: &Tree<'_>,
    items: &[DisplayItem],
    node: &Node,
    item: &DisplayItem,
) -> EngineResult<Option<Placed>> {
    if !needed(tree, node, item)? {
        return Ok(None);
    }
    let levels = levels(tree, items, node)?;
    let mut full = compose(&levels, 0);
    let [sx, sy] = super::scrolled(tree, node)?;
    full = multiply(&translation([-sx, -sy, 0.0]), &full);
    if let Some(style) = generated_style(node, item) {
        full = multiply(&full, &own_matrix(style, Some(item)));
    }
    Ok(Some(project_box(item, &full, node.style.backface_hidden)))
}

/** The projected corners, the paint affine fitted through three of them, and the facing. */
fn project_box(item: &DisplayItem, full: &Matrix4, backface_hidden: bool) -> Placed {
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
    let facing_away = orientation(quad[0], quad[TOP_RIGHT], quad[BOTTOM_LEFT]) < 0.0;
    Placed {
        affine: fit_affine([x, y, w, h], [quad[0], quad[TOP_RIGHT], quad[BOTTOM_LEFT]]),
        quad,
        hidden: backface_hidden && facing_away,
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

/** Each box from the context root down to `node`, with `node` last. */
fn levels(tree: &Tree<'_>, items: &[DisplayItem], node: &Node) -> EngineResult<Vec<Level>> {
    let mut chain = tree.ancestors(node)?;
    chain.reverse();
    chain.push(node);
    chain
        .into_iter()
        .map(|anchor| level(tree, items, anchor))
        .collect()
}

/** One box's level: its offset and own transform, its perspective, and its preserve-3d flag. */
fn level(tree: &Tree<'_>, items: &[DisplayItem], anchor: &Node) -> EngineResult<Level> {
    let own = own_box(tree, items, anchor);
    let [x, y] = offset(tree, items, anchor);
    Ok(Level {
        local: multiply(&translation([x, y, 0.0]), &own_matrix(&anchor.style, own)),
        perspective: perspective_matrix(&anchor.style, own),
        preserve: preserves_3d(&anchor.style),
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

/** Composes levels from `start` down: each level's local matrix, the perspective of the level
above it, and at every flattening box the flattened subtree below it (its z is dropped there). */
fn compose(levels: &[Level], start: usize) -> Matrix4 {
    let mut out = IDENTITY4;
    for index in start..levels.len() {
        if index > start {
            out = multiply(&out, &levels[index - 1].perspective);
        }
        out = multiply(&out, &levels[index].local);
        if index + 1 < levels.len() && !levels[index].preserve {
            let below = multiply(&levels[index].perspective, &compose(levels, index + 1));
            return multiply(&out, &flatten(&below));
        }
    }
    out
}
