/** Embedded documents in the page's display: the display list of a document an iframe, object
 * or embed shows is laid out in the frame's content box (Blink's LayoutEmbeddedContent and its
 * child LocalFrameView) and spliced into the embedder's list right after the frame's own box,
 * so both painters draw it with nothing frame-specific. Its boxes move to the content box's
 * origin, clip to it, and paint in the frame's stacking layer, opacity and transform. */
use super::{ClipRect, DisplayItem, DisplayList, OpacityGroup};
use crate::document::Document;

/**
 * The viewport of the document a frame item shows, `[x, y, width, height]` in the embedder's
 * coordinates: the frame's content box, inside its border and padding (Blink sizes the child
 * frame view to `LayoutReplaced::ReplacedContentRect`).
 */
pub fn frame_viewport(item: &DisplayItem) -> [f64; 4] {
    item.content_box
        .unwrap_or([item.x, item.y, item.width, item.height])
}

/**
 * Splices `child`, the display list of `document` laid out in the viewport of the frame item
 * `parent.items[at]`, into `parent` right after that item. Each child box gets an identity
 * `id_base` above its own, so it never names an embedder node (painters and agents find no
 * embedder node behind it, and images fetched for the child are keyed the same way). The
 * child's canvas is transparent unless its root or body has a background, as a subframe's
 * base background is in Chromium.
 */
pub fn embed_frame(
    parent: &mut DisplayList,
    at: usize,
    child: DisplayList,
    document: &Document,
    id_base: usize,
) {
    let Some(owner) = parent.items.get(at).cloned() else {
        return;
    };
    let [x, y, width, height] = frame_viewport(&owner);
    let viewport = ClipRect {
        x,
        y,
        width,
        height,
        radius: [0.0; 4],
    };
    let viewport = owner
        .clip
        .map_or(viewport, |clip| clip.intersect(&viewport));
    let mut items = canvas(&child, document, id_base);
    items.extend(child.items);
    for item in &mut items {
        adopt(item, &owner, (x, y), viewport, id_base);
    }
    parent.items.splice(at + 1..at + 1, items);
}

/** The child's root background, body decoration and root border as plain boxes in its own
coordinates, painted under its content. */
fn canvas(child: &DisplayList, document: &Document, id_base: usize) -> Vec<DisplayItem> {
    let root = &document.formatting.html.style;
    let body = &document.formatting.body.style;
    let color = [&root.background, &body.background]
        .into_iter()
        .flatten()
        .find(|color| color.as_str() != "transparent")
        .cloned();
    let view = super::DisplayDecoration {
        x: 0.0,
        y: 0.0,
        width: child.width,
        height: child.height,
        color,
        border: 0.0,
        border_color: None,
    };
    [
        Some(&view),
        child.body_decoration.as_ref(),
        child.root_decoration.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter(|decoration| decoration.color.is_some() || decoration.border > 0.0)
    .filter_map(|decoration| plain(decoration, id_base))
    .collect()
}

/** A decoration as a display item with only a background and a uniform border. */
fn plain(decoration: &super::DisplayDecoration, id: usize) -> Option<DisplayItem> {
    serde_json::from_value(serde_json::json!({
        "local_id": id, "tag": "html", "role": "document",
        "x": decoration.x, "y": decoration.y,
        "width": decoration.width, "height": decoration.height,
        "font_size": 0.0, "bold": false, "lines": [],
        "background": decoration.color, "border": decoration.border,
        "border_color": decoration.border_color,
    }))
    .ok()
}

/** Moves one child box into the embedder: identity, position, clip, transform, stacking layer,
opacity groups and viewport pinning all become the frame's. */
fn adopt(
    item: &mut DisplayItem,
    owner: &DisplayItem,
    (dx, dy): (f64, f64),
    viewport: ClipRect,
    id_base: usize,
) {
    item.local_id = item.local_id.saturating_add(id_base);
    super::line::translate(item, dx, dy);
    if let Some(content) = item.content_box.as_mut() {
        content[0] += dx;
        content[1] += dy;
    }
    let clip = item.clip.map(|clip| ClipRect {
        x: clip.x + dx,
        y: clip.y + dy,
        ..clip
    });
    item.clip = Some(clip.map_or(viewport, |clip| clip.intersect(&viewport)));
    let moved = item.transform.map(|own| moved(own, dx, dy));
    item.transform = match (owner.transform, moved) {
        (Some(outer), Some(inner)) => Some(compose(outer, inner)),
        (outer, inner) => outer.or(inner),
    };
    item.z = owner.z;
    item.pin = owner.pin;
    item.opacity *= owner.opacity;
    let groups = item.stacking_opacity.drain(..).map(|group| OpacityGroup {
        id: group.id.saturating_add(id_base),
        ..group
    });
    item.stacking_opacity = owner
        .stacking_opacity
        .iter()
        .copied()
        .chain(groups)
        .collect();
}

/** An absolute affine about the child's origin, re-expressed about the embedder's: conjugated
by the translation to the frame's content box. */
fn moved([a, b, c, d, e, f]: [f64; 6], dx: f64, dy: f64) -> [f64; 6] {
    [
        a,
        b,
        c,
        d,
        e + dx - (a * dx + c * dy),
        f + dy - (b * dx + d * dy),
    ]
}

/** The affine `outer` applied after `inner`. */
fn compose(outer: [f64; 6], inner: [f64; 6]) -> [f64; 6] {
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
