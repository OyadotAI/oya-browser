/** Paint-time caches for one display list: which items touch each band of the page, each line's
prepared text and each decoded image, so a scroll frame paints only what it reveals and never
rebuilds text or decodes images again. Rebuilt whenever the display list or its images change. */
use crate::document::{DisplayItem, DisplayList};
use objc2::{AnyThread, rc::Retained};
use objc2_app_kit::NSImage;
use objc2_foundation::{NSAttributedString, NSData, NSRect};
use std::collections::HashMap;

/// Height of one index band, in document points; about a quarter of a window.
const BAND: f64 = 256.0;

/** Spatial index and draw caches for the current display list. */
#[derive(Default)]
pub(super) struct Painted {
    /// Item indices per band, in paint order.
    bands: Vec<Vec<u32>>,
    /// Items with no static extent (transformed or shadowed), painted for every dirty rect.
    always: Vec<u32>,
    /// Prepared text per (item index, line index).
    text: HashMap<(u32, u32), Retained<NSAttributedString>>,
    /// Decoded images per node identity; a failed decode is remembered too.
    images: HashMap<usize, Option<Retained<NSImage>>>,
}

impl Painted {
    /** Indexes every item by the bands its vertical extent covers. */
    pub fn new(display: &DisplayList) -> Self {
        let mut painted = Self::default();
        for (index, item) in display.items.iter().enumerate() {
            let index = index as u32;
            let Some((first, last)) = extent(item) else {
                painted.always.push(index);
                continue;
            };
            if painted.bands.len() <= last {
                painted.bands.resize_with(last + 1, Vec::new);
            }
            for band in &mut painted.bands[first..=last] {
                band.push(index);
            }
        }
        painted
    }
    /** Items that may touch `dirty`, in paint order; callers still apply the exact test. */
    pub fn visible(&self, dirty: NSRect) -> Vec<u32> {
        let first = band(dirty.origin.y);
        let last = band(dirty.origin.y + dirty.size.height);
        let mut items = self.always.clone();
        let last = last.min(self.bands.len().saturating_sub(1));
        for band in self.bands.get(first..=last).into_iter().flatten() {
            items.extend_from_slice(band);
        }
        items.sort_unstable();
        items.dedup();
        items
    }
    /** The prepared text of one line, built once per display list. */
    pub fn text(
        &mut self,
        key: (u32, u32),
        make: impl FnOnce() -> Retained<NSAttributedString>,
    ) -> Retained<NSAttributedString> {
        self.text.entry(key).or_insert_with(make).clone()
    }
    /** The decoded image for a node's fetched bytes, decoded once per display list. */
    pub fn image(&mut self, id: usize, bytes: Option<&[u8]>) -> Option<Retained<NSImage>> {
        let bytes = bytes?;
        self.images
            .entry(id)
            .or_insert_with(|| decode(bytes))
            .clone()
    }
}

/** First and last band of an item's painted extent, or None when it must always paint. */
fn extent(item: &DisplayItem) -> Option<(usize, usize)> {
    if item.transform.is_some() || !item.shadows.is_empty() || item.pin.is_some() {
        return None;
    }
    let bottom = item.y
        + item.height.max(item.control_height.unwrap_or(0.0))
        + item.validation.len() as f64 * item.font_size * 2.0;
    (item.y.is_finite() && bottom.is_finite()).then(|| (band(item.y), band(bottom)))
}
/** The band holding a document y; anything above the page is band zero. */
fn band(y: f64) -> usize {
    (y.max(0.0) / BAND) as usize
}
/** One decode of fetched bytes; AppKit keeps the decoded bitmap with the image. */
fn decode(bytes: &[u8]) -> Option<Retained<NSImage>> {
    // SAFETY: the bytes outlive the call and NSData copies them.
    let data = unsafe { NSData::dataWithBytes_length(bytes.as_ptr().cast(), bytes.len() as _) };
    NSImage::initWithData(NSImage::alloc(), &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_foundation::{NSPoint, NSSize};

    /** A dirty rect sees exactly the items whose bands it covers, in paint order. */
    #[test]
    fn visible_items_come_from_covered_bands_in_paint_order() {
        let painted = Painted {
            bands: vec![vec![0, 2], vec![2, 3], vec![4]],
            always: vec![1],
            ..Default::default()
        };
        let dirty = NSRect::new(NSPoint::new(0.0, 300.0), NSSize::new(100.0, 100.0));
        assert_eq!(painted.visible(dirty), vec![1, 2, 3]);
    }

    /** A dirty rect below the page sees only the always-painted items. */
    #[test]
    fn dirty_rect_below_the_page_sees_only_always_items() {
        let painted = Painted {
            bands: vec![vec![0]],
            always: vec![5],
            ..Default::default()
        };
        let dirty = NSRect::new(NSPoint::new(0.0, 9000.0), NSSize::new(100.0, 100.0));
        assert_eq!(painted.visible(dirty), vec![5]);
    }
}
