/** `background-attachment: fixed` and `local`: image layers positioned against the viewport,
 * or painted with a scroll container's scrolled contents, not the box. */
use super::{DisplayBackground, DisplayItem, ScrollArea};
use crate::document::{
    Document, RenderContext,
    style::{BackgroundBox, ComputedStyle},
};

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
 * Paints a scroll container's `background-attachment: local` background with its scrolled
 * contents (CSS Backgrounds 3 section 3.4): image layers are positioned against the scrollable
 * overflow area at the scroll offset, and the color and layers are clipped inside the padding
 * box, a `content-box` clip being the scrolled contents' content box. Without this a scrolled
 * box's background stayed put while its contents moved.
 */
// ponytail: gradient tiles still follow the box; move them too when a page scrolls one.
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
        let style = &document.nodes[position].style;
        if !style.background_local {
            continue;
        }
        let (scrolled, clip) = (scrolled_contents(area, style), scrolled_clip(area, style));
        let own = |item: &&mut DisplayItem| item.local_id == area.local_id;
        for item in items.iter_mut().filter(own) {
            position_images(item, scrolled);
            item.background_clip = clip.unwrap_or(item.background_clip);
        }
    }
}

/** A scroll container's scrollable overflow area at its scroll offset (its padding box grown
 * by how far it scrolls, moved back by how far it has), inset by the padding for a
 * `content-box` origin. */
fn scrolled_contents(area: &ScrollArea, style: &ComputedStyle) -> [f64; 4] {
    let inset = match style.background_origin {
        BackgroundBox::ContentBox => style.padding,
        _ => [0.0; 4],
    };
    [
        area.x - area.scroll_x + inset[3],
        area.y - area.scroll_y + inset[0],
        area.width + area.max_x - inset[1] - inset[3],
        area.height + area.max_y - inset[0] - inset[2],
    ]
}

/** The clip of a local background as insets from the border box: the padding box, or for a
 * `content-box` clip the scrolled contents' content box within it. None keeps the border area. */
fn scrolled_clip(area: &ScrollArea, style: &ComputedStyle) -> Option<[f64; 4]> {
    let border = super::sizing::border_sides(style);
    let room = [
        area.scroll_y,
        area.max_x - area.scroll_x,
        area.max_y - area.scroll_y,
        area.scroll_x,
    ];
    match style.background_clip {
        BackgroundBox::BorderArea => None,
        BackgroundBox::ContentBox => Some(std::array::from_fn(|i| {
            border[i] + (style.padding[i] - room[i]).max(0.0)
        })),
        _ => Some(border),
    }
}

/** Gives every image layer of `item` the positioning area `area`. */
fn position_images(item: &mut DisplayItem, area: [f64; 4]) {
    for layer in &mut item.backgrounds {
        if let DisplayBackground::Image { area: own, .. } = layer {
            *own = Some(area);
        }
    }
}
