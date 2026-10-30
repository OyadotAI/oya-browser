/** Export visual affine geometry and untouched layout measurements from the same native display list. */
use crate::{document::DisplayList, v9::NodeBox};

/** Paint order remains authoritative for DOM hit queries; clips affect hits without shrinking rectangles. */
pub(super) fn boxes(display: &DisplayList) -> Vec<NodeBox> {
    display
        .items
        .iter()
        .map(|item| {
            let quad = mars_core::geometry::quad(
                [item.x, item.y, item.width, item.height],
                item.transform,
            );
            let visual = mars_core::geometry::bounds(&quad);
            let [x, y, width, height] = visual.unwrap_or_default();
            NodeBox {
                node: item.local_id,
                x,
                y,
                width,
                height,
                layout_position: Some([item.x, item.y]),
                layout_size: Some([item.width, item.height]),
                resize_box: item.resize_box,
                quad: visual.and(item.transform.map(|_| quad)),
                clip: item
                    .clip
                    .map(|clip| [clip.x, clip.y, clip.width, clip.height]),
                pin: item.pin.map(|pin| [pin.top, pin.start, pin.room]),
            }
        })
        .collect()
}
