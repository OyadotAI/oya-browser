/** Which page node a point on the canvas lands on, for clicks that no native control took:
a div with a click listener, a span with role=button, or a link whose widget has not been
created yet. The topmost painted item wins, as in a browser; the runtime then decides whether
anything listens to the click. */
use crate::document::{DisplayItem, DisplayList, Document};
use objc2_foundation::NSPoint;

/// A press and release this close together is a click rather than a drag selection.
pub(super) const CLICK_SLOP: f64 = 4.0;

/** The topmost visible, interactive item containing the point, by node identity. */
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn node_at(display: &DisplayList, document: &Document, point: NSPoint) -> Option<usize> {
    node_at_scrolled(display, document, point, 0.0)
}
/** As `node_at` with the viewport scrolled to `top`, where sticky and fixed boxes have moved. */
pub(super) fn node_at_scrolled(
    display: &DisplayList,
    document: &Document,
    point: NSPoint,
    top: f64,
) -> Option<usize> {
    display
        .items
        .iter()
        .rev()
        .filter(|item| {
            let shift = item.pin.map_or(0.0, |pin| pin.shift(top));
            contains(item, NSPoint::new(point.x, point.y - shift))
        })
        .find(|item| {
            document
                .nodes
                .iter()
                .find(|node| node.local_id == item.local_id)
                .is_some_and(|node| !node.hidden() && node.style.interactive && !node.disabled)
        })
        .map(|item| item.local_id)
}

/** A frame's painted IDs resolve against its own document before the embedder underneath. */
pub(super) fn node_at_frames(
    display: &DisplayList,
    document: &Document,
    frames: &[(usize, usize, Document)],
    point: NSPoint,
    top: f64,
) -> Option<usize> {
    display
        .items
        .iter()
        .rev()
        .filter(|item| {
            let shift = item.pin.map_or(0.0, |pin| pin.shift(top));
            contains(item, NSPoint::new(point.x, point.y - shift))
        })
        .find_map(|item| {
            let frame = frames
                .iter()
                .find(|(base, end, _)| item.local_id >= *base && item.local_id < *end);
            if let Some((base, _, frame)) = frame {
                let node = frame
                    .nodes
                    .iter()
                    .find(|node| node.local_id == item.local_id - base)?;
                return (!node.hidden() && !node.disabled).then_some(item.local_id);
            }
            document
                .nodes
                .iter()
                .find(|node| node.local_id == item.local_id)
                .filter(|node| !node.hidden() && node.style.interactive && !node.disabled)
                .map(|_| item.local_id)
        })
}

/** Whether the point is inside the item's box and inside its ancestors' clip. */
fn contains(item: &DisplayItem, point: NSPoint) -> bool {
    let point = [point.x, point.y];
    let quad = mars_core::geometry::quad([item.x, item.y, item.width, item.height], item.transform);
    mars_core::geometry::contains_quad(&quad, point)
        && item.clip.is_none_or(|clip| {
            mars_core::geometry::contains_rect([clip.x, clip.y, clip.width, clip.height], point)
        })
}

/** Whether a release at `up` after a press at `down` is a click rather than a drag. */
pub(super) fn is_click(down: NSPoint, up: NSPoint) -> bool {
    (down.x - up.x).abs() <= CLICK_SLOP && (down.y - up.y).abs() <= CLICK_SLOP
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Node;

    /** Native targeting follows the same transformed rectangle and pre-transform clip as painting. */
    #[test]
    fn rotated_click_targets_reject_empty_aabb_corners_and_obey_clips() {
        let mut target = item(1, 0.0, 0.0, 100.0, 100.0);
        target.transform = Some([0.5, 0.5, -0.5, 0.5, 50.0, 0.0]);
        assert!(contains(&target, NSPoint::new(50.0, 50.0)));
        assert!(!contains(&target, NSPoint::new(5.0, 5.0)));
        target.clip = Some(crate::document::ClipRect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
            radius: [0.0; 4],
        });
        assert!(!contains(&target, NSPoint::new(50.0, 50.0)));
        target.clip = None;
        target.transform = Some([0.0; 6]);
        assert!(!contains(&target, NSPoint::new(0.0, 0.0)));
    }

    /** A node whose style accepts input. */
    fn node(local_id: usize) -> Node {
        let mut node: Node = serde_json::from_value(serde_json::json!({
            "local_id": local_id, "html_id": "", "tag": "div", "text": ""
        }))
        .unwrap();
        node.style.interactive = true;
        node
    }
    /** A box at the given place, painted after everything created before it. */
    fn item(local_id: usize, x: f64, y: f64, width: f64, height: f64) -> DisplayItem {
        serde_json::from_value(serde_json::json!({
            "local_id": local_id, "tag": "div", "role": "group", "x": x, "y": y,
            "width": width, "height": height, "font_size": 16.0, "bold": false, "lines": []
        }))
        .unwrap()
    }
    /** A list and document holding the given items. */
    fn page(items: Vec<DisplayItem>) -> (DisplayList, Document) {
        let mut document: Document =
            serde_json::from_value(serde_json::json!({"nodes": []})).unwrap();
        document.nodes = items.iter().map(|item| node(item.local_id)).collect();
        let display: DisplayList = serde_json::from_value(serde_json::json!({
            "unit": "logical_points", "width": 800.0, "height": 600.0, "items": []
        }))
        .unwrap();
        (DisplayList { items, ..display }, document)
    }

    /** The item painted last wins when boxes overlap. */
    #[test]
    fn topmost_item_under_the_point_wins() {
        let (display, document) = page(vec![
            item(1, 0.0, 0.0, 200.0, 100.0),
            item(2, 10.0, 10.0, 50.0, 20.0),
        ]);
        assert_eq!(
            node_at(&display, &document, NSPoint::new(20.0, 20.0)),
            Some(2)
        );
        assert_eq!(
            node_at(&display, &document, NSPoint::new(150.0, 80.0)),
            Some(1)
        );
        assert_eq!(
            node_at(&display, &document, NSPoint::new(300.0, 300.0)),
            None
        );
    }
    /** A frame tile above its iframe reaches the frame identity the runtime can resolve. */
    #[test]
    fn embedded_tile_wins_over_its_owner() {
        let (display, document) = page(vec![
            item(1, 0.0, 0.0, 200.0, 100.0),
            item(101, 10.0, 10.0, 50.0, 20.0),
        ]);
        let mut frame: Document = serde_json::from_value(serde_json::json!({"nodes":[]})).unwrap();
        frame.nodes.push(node(1));
        assert_eq!(
            node_at_frames(
                &display,
                &document,
                &[(100, 200, frame)],
                NSPoint::new(20.0, 20.0),
                0.0
            ),
            Some(101)
        );
    }

    /** Hidden, disabled and pointer-events:none nodes let the click fall through to what is under them. */
    #[test]
    fn inert_nodes_fall_through() {
        let (display, mut document) = page(vec![
            item(1, 0.0, 0.0, 200.0, 100.0),
            item(2, 0.0, 0.0, 200.0, 100.0),
            item(3, 0.0, 0.0, 200.0, 100.0),
        ]);
        document.nodes[1].disabled = true;
        document.nodes[2].style.interactive = false;
        assert_eq!(
            node_at(&display, &document, NSPoint::new(20.0, 20.0)),
            Some(1)
        );
    }

    /** A point outside an item's clip misses it even inside its own box. */
    #[test]
    fn clipped_items_are_hit_only_inside_the_clip() {
        let mut clipped = item(1, 0.0, 0.0, 200.0, 100.0);
        clipped.clip = serde_json::from_value(serde_json::json!({
            "x": 0.0, "y": 0.0, "width": 50.0, "height": 50.0
        }))
        .unwrap();
        let (display, document) = page(vec![clipped]);
        assert_eq!(
            node_at(&display, &document, NSPoint::new(20.0, 20.0)),
            Some(1)
        );
        assert_eq!(
            node_at(&display, &document, NSPoint::new(150.0, 80.0)),
            None
        );
    }

    /** A release a few points from the press is a click; farther is a drag. */
    #[test]
    fn a_short_move_is_a_click() {
        assert!(is_click(NSPoint::new(10.0, 10.0), NSPoint::new(12.0, 13.0)));
        assert!(!is_click(
            NSPoint::new(10.0, 10.0),
            NSPoint::new(30.0, 10.0)
        ));
    }
}
