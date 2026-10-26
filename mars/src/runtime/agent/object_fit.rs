/**
 * Where a bitmap `<img>` paints inside its content box under `object-fit` and `object-position`,
 * the windowless twin of the desktop painter's (src/desktop/canvas/background.rs) so both
 * painters agree: `fill` stretches, `contain` and `cover` scale uniformly to fit inside or to
 * cover, `none` keeps the natural size and `scale-down` is the smaller of `none` and `contain`.
 */
use crate::document::{DisplayItem, ObjectFit};

/** The `[x, y, width, height]` the natural `(width, height)` bitmap paints at in `content`. */
pub(super) fn object_rect(item: &DisplayItem, content: [f64; 4], natural: (f64, f64)) -> [f64; 4] {
    let [cx, cy, cw, ch] = content;
    let (iw, ih) = natural;
    if iw <= 0.0 || ih <= 0.0 {
        return content;
    }
    let scale = |cover: bool| {
        let scale = if cover {
            (cw / iw).max(ch / ih)
        } else {
            (cw / iw).min(ch / ih)
        };
        (iw * scale, ih * scale)
    };
    let (dw, dh) = match item.object_fit {
        ObjectFit::Fill => (cw, ch),
        ObjectFit::Contain => scale(false),
        ObjectFit::Cover => scale(true),
        ObjectFit::None => (iw, ih),
        ObjectFit::ScaleDown => {
            let contained = scale(false);
            if contained.0 < iw {
                contained
            } else {
                (iw, ih)
            }
        }
    };
    let [ox_px, ox_pct, oy_px, oy_pct] = item.object_position;
    [
        cx + ox_px + (cw - dw) * ox_pct / 100.0,
        cy + oy_px + (ch - dh) * oy_pct / 100.0,
        dw,
        dh,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A display item with the given fit and the initial `50% 50%` position. */
    fn item(fit: ObjectFit) -> DisplayItem {
        let mut item: DisplayItem = serde_json::from_value(serde_json::json!({
            "local_id": 0, "tag": "img", "role": "image", "x": 0.0, "y": 0.0, "width": 1.0,
            "height": 1.0, "font_size": 16.0, "bold": false, "lines": [],
        }))
        .expect("a minimal display item");
        item.object_fit = fit;
        item.object_position = [0.0, 50.0, 0.0, 50.0];
        item
    }

    #[test]
    fn contain_scales_uniformly_and_centers() {
        let rect = object_rect(
            &item(ObjectFit::Contain),
            [0.0, 0.0, 300.0, 100.0],
            (20.0, 50.0),
        );
        assert_eq!(rect, [130.0, 0.0, 40.0, 100.0]);
    }

    #[test]
    fn fill_stretches_to_the_content_box() {
        let rect = object_rect(
            &item(ObjectFit::Fill),
            [5.0, 5.0, 300.0, 100.0],
            (20.0, 50.0),
        );
        assert_eq!(rect, [5.0, 5.0, 300.0, 100.0]);
    }
}
