/**
 * Image keys for background layers past an element's first, and for the canvas's propagated
 * root or body background. The app stores each fetched image under a key; an element's first
 * image is keyed by its identity (`background_urls`), and these keys put each further layer, and
 * the canvas's layers, beside it without colliding with any element's identity.
 */
use super::{BackgroundLayer, ComputedStyle};
use crate::document::Document;

/// Bits above which a key holds its layer number; below them, the owner's identity.
const LAYER_SHIFT: u32 = 40;
/// The owner bits of a canvas key: no element has this identity.
const CANVAS_OWNER: usize = (1 << LAYER_SHIFT) - 1;

/** The key of image layer `layer` (counting image layers from 0) of element `local`. Layer 0
 * is the element's own identity, as `background_urls` has always keyed it. */
pub fn layer_key(local: usize, layer: usize) -> usize {
    (local & CANVAS_OWNER) | (layer << LAYER_SHIFT)
}

/** The key of the canvas's image layer `layer`. */
pub fn canvas_key(layer: usize) -> usize {
    CANVAS_OWNER | ((layer + 1) << LAYER_SHIFT)
}

/** The element whose layer an image key belongs to; None for a canvas key, which belongs to
 * the page itself (always wanted). */
pub fn background_owner(key: usize) -> Option<usize> {
    let owner = key & CANVAS_OWNER;
    (owner != CANVAS_OWNER).then_some(owner)
}

/** The `url()` of each image layer of `layers`, in order. */
pub(super) fn image_urls(layers: &[BackgroundLayer]) -> impl Iterator<Item = &String> {
    layers.iter().filter_map(|layer| match layer {
        BackgroundLayer::Image { url } => Some(url),
        _ => None,
    })
}

/** Each element's image layers after its first, and the canvas's, as (key, authored url). */
pub(super) fn extra_layers(document: &Document) -> Vec<(usize, &String)> {
    let elements = document
        .nodes
        .iter()
        .filter(|node| node.tag != "img" && !node.hidden() && node.style.mask.is_none())
        .flat_map(|node| {
            image_urls(&node.style.backgrounds)
                .enumerate()
                .skip(1)
                .map(move |(layer, url)| (layer_key(node.local_id, layer), url))
        });
    let canvas = propagating(document)
        .into_iter()
        .flat_map(|style| image_urls(&style.backgrounds).enumerate())
        .map(|(layer, url)| (canvas_key(layer), url));
    elements.chain(canvas).collect()
}

/** The style whose background the canvas takes: the root's when it paints any, else the
 * body's when it has a box; none when the root has no box (CSS Backgrounds 3 section 2.11.2). */
pub fn propagating(document: &Document) -> Option<&ComputedStyle> {
    let (root, body) = (
        &document.formatting.html.style,
        &document.formatting.body.style,
    );
    if root.hidden {
        return None;
    }
    let paints = |style: &ComputedStyle| {
        !style.backgrounds.is_empty()
            || style
                .background
                .as_deref()
                .is_some_and(|color| color != "transparent")
    };
    if paints(root) {
        return Some(root);
    }
    (!body.hidden && !body.display_contents && paints(body)).then_some(body)
}
