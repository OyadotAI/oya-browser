/** `background-attachment: fixed` and `local`: image layers positioned against the viewport,
 * or against a scroll container's scrolled contents, not the box. */
use super::{DisplayBackground, DisplayItem, ScrollArea};
use crate::document::{Document, RenderContext};

/**
 * Gives every image layer of a box with `background-attachment: fixed` the initial viewport
 * (at the document's origin) as its positioning area, as Blink's `BackgroundImageGeometry`
 * does for a fixed layer; the box still clips it. Without this a fixed stripe painted at the
 * box's own top-left. Generated boxes keep their own (scrolling) layers.
 */
// ponytail: the area is the unscrolled viewport and percentage sizes still resolve against the
// box; follow the scroll offset and resize against the viewport when a page needs either.
pub(super) fn anchor(document: &Document, items: &mut [DisplayItem], context: RenderContext) {
    let viewport = [0.0, 0.0, context.width, context.height];
    let positions = super::sizing::positions(document);
    for item in items.iter_mut() {
        let Some(&Some(position)) = positions.get(item.local_id) else {
            continue;
        };
        let node = &document.nodes[position];
        if !node.style.background_fixed || super::effects::generated_style(node, item).is_some() {
            continue;
        }
        for layer in &mut item.backgrounds {
            if let DisplayBackground::Image { area, .. } = layer {
                *area = Some(viewport);
            }
        }
    }
}

/**
 * Gives every image layer of a scroll container with `background-attachment: local` its
 * scrollable overflow area, moved by its scroll offset, as its positioning area: the image
 * scrolls with the contents (CSS Backgrounds 3 section 3.4). Without this a scrolled box's
 * image stayed put while its contents moved.
 */
// ponytail: the area is the padding box grown to the scroll size, so `background-origin:
// content-box` and the clip still follow the box; refine when a page needs either.
pub(super) fn scroll_with_contents(
    document: &Document,
    items: &mut [DisplayItem],
    scrollers: &[ScrollArea],
) {
    let positions = super::sizing::positions(document);
    for area in scrollers {
        let Some(&Some(position)) = positions.get(area.local_id) else {
            continue;
        };
        if !document.nodes[position].style.background_local {
            continue;
        }
        let scrolled = scrolled_contents(area);
        for item in items
            .iter_mut()
            .filter(|item| item.local_id == area.local_id)
        {
            position_images(item, scrolled);
        }
    }
}

/** A scroll container's scrollable overflow area at its scroll offset: its padding box grown
 * by how far it scrolls, moved back by how far it has. */
fn scrolled_contents(area: &ScrollArea) -> [f64; 4] {
    [
        area.x - area.scroll_x,
        area.y - area.scroll_y,
        area.width + area.max_x,
        area.height + area.max_y,
    ]
}

/** Gives every image layer of `item` the positioning area `area`. */
fn position_images(item: &mut DisplayItem, area: [f64; 4]) {
    for layer in &mut item.backgrounds {
        if let DisplayBackground::Image { area: own, .. } = layer {
            *own = Some(area);
        }
    }
}
