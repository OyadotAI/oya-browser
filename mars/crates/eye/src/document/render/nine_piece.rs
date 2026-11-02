/**
 * Nine-slice geometry for `border-image`, as Blink's `NinePieceImageGrid` and
 * `NinePieceImagePainter` compute it: which part of the image each of the nine pieces takes,
 * where it lands in the border image area, and how it tiles there. Both painters draw from
 * these pieces, so the screenshot raster and the desktop window cut the image the same way.
 */
use super::border_image::DisplayBorderImage;
use crate::document::BorderImageRepeat;

/// A percentage's whole.
const PERCENT: f64 = 100.0;
/// Blink's LayoutUnit resolution: slices round to 1/64 px before they are used.
const LAYOUT_UNIT: f64 = 64.0;
/// How far the widths may overshoot the area and still count as abutting (one LayoutUnit).
const ABUTTING: f64 = 1.0 / LAYOUT_UNIT;
/// Pieces tiled per axis at most; a hairline tile over a huge area stops here.
const TILES: f64 = 4096.0;

/// One drawable piece: `source` in image pixels drawn into `dest` in document points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NinePiece {
    /// The image rectangle [x, y, width, height] this piece samples.
    pub source: [f64; 4],
    /// The document rectangle it covers; nothing paints outside it.
    pub dest: [f64; 4],
    /// The drawn size of one tile; `dest`'s size for a stretched piece.
    pub tile: [f64; 2],
    /// Where the first tile starts, from `dest`'s origin (centered `repeat`, `space` gaps).
    pub phase: [f64; 2],
    /// Empty space between tiles (`space`).
    pub gap: [f64; 2],
}
/** One side's slice into the image and width in the area, both in px. */
#[derive(Clone, Copy)]
struct Edge {
    /** Image pixels the slice takes from this side. */
    slice: f64,
    /** Area pixels the side's pieces cover. */
    width: f64,
}
impl Edge {
    /** Blink paints a side only when it has both an image slice and a width. */
    fn drawable(self) -> bool {
        self.slice > 0.0 && self.width > 0.0
    }
    /** How much the slice scales to fill the width; 1 for a side that does not paint. */
    fn scale(self) -> f64 {
        if self.drawable() {
            self.width / self.slice
        } else {
            1.0
        }
    }
}

/**
 * The pieces a border image paints for a box whose border box is `border_box`, from an image
 * of `natural` size ([width, height] in px; a gradient's is the area's size). Corners stretch;
 * edges and the middle tile by the repeat rules; a `space` piece with no room for one whole
 * tile, and the middle without `fill`, paint nothing.
 */
pub fn nine_pieces(
    border_box: [f64; 4],
    image: &DisplayBorderImage,
    natural: [f64; 2],
) -> Vec<NinePiece> {
    let area = snap_area(border_box, image.outset);
    if area[2] <= 0.0 || area[3] <= 0.0 || natural[0] <= 0.0 || natural[1] <= 0.0 {
        return Vec::new();
    }
    let edges = edges(image, natural, [area[2], area[3]]);
    let grid = Grid {
        area,
        natural,
        edges,
        repeat: image.repeat,
    };
    let mut pieces = grid.corners();
    pieces.extend(grid.sides());
    if image.fill {
        pieces.extend(grid.middle());
    }
    pieces
}
/** The border box grown by the outsets, snapped to whole pixels as `ToPixelSnappedRect`. */
fn snap_area([x, y, w, h]: [f64; 4], [top, right, bottom, left]: [f64; 4]) -> [f64; 4] {
    let (x0, y0) = ((x - left).round(), (y - top).round());
    let (x1, y1) = ((x + w + right).round(), (y + h + bottom).round());
    [x0, y0, x1 - x0, y1 - y0]
}
/** Slices against the image, widths with `auto` resolved, scaled down to fit and snapped. */
fn edges(image: &DisplayBorderImage, [iw, ih]: [f64; 2], [aw, ah]: [f64; 2]) -> [Edge; 4] {
    let slices: [f64; 4] = std::array::from_fn(|side| {
        let maximum = if side % 2 == 0 { ih } else { iw };
        let [px, percent] = image.slice[side];
        let resolved = (px + percent / PERCENT * maximum).min(maximum).max(0.0);
        (resolved * LAYOUT_UNIT).round() / LAYOUT_UNIT
    });
    let mut widths: [f64; 4] =
        std::array::from_fn(|side| image.widths[side].unwrap_or(slices[side]).max(0.0));
    let factor = (aw / (widths[1] + widths[3])).min(ah / (widths[0] + widths[2]));
    if factor < 1.0 {
        widths = widths.map(|width| width * factor);
    }
    let [left, right] = snap_pair(widths[3], widths[1], aw);
    let [top, bottom] = snap_pair(widths[0], widths[2], ah);
    let widths = [top, right, bottom, left];
    std::array::from_fn(|side| Edge {
        slice: slices[side],
        width: widths[side],
    })
}
/** Blink's `SnapEdgeWidths` on one axis: abutting widths round and fill the extent exactly,
 * others floor. */
fn snap_pair(first: f64, second: f64, extent: f64) -> [f64; 2] {
    if extent - (first + second) <= ABUTTING {
        let first = first.round();
        return [first, extent - first];
    }
    [first.floor(), second.floor()]
}

/** The resolved grid one border image paints from. */
struct Grid {
    /** The snapped border image area. */
    area: [f64; 4],
    /** The image's size in px. */
    natural: [f64; 2],
    /** Top, right, bottom and left slices and widths. */
    edges: [Edge; 4],
    /** Horizontal and vertical tiling rules. */
    repeat: [BorderImageRepeat; 2],
}
impl Grid {
    /** The four corners, each stretched from its slice when both of its sides paint. */
    fn corners(&self) -> Vec<NinePiece> {
        let [top, right, bottom, left] = self.edges;
        let [iw, ih] = self.natural;
        let [x, y, w, h] = self.area;
        [(top, left), (top, right), (bottom, left), (bottom, right)]
            .into_iter()
            .enumerate()
            .filter(|(_, (vertical, horizontal))| vertical.drawable() && horizontal.drawable())
            .map(|(corner, (vertical, horizontal))| {
                let (far_x, far_y) = (corner % 2 == 1, corner >= 2);
                let sx = if far_x { iw - horizontal.slice } else { 0.0 };
                let sy = if far_y { ih - vertical.slice } else { 0.0 };
                let dx = if far_x { x + w - horizontal.width } else { x };
                let dy = if far_y { y + h - vertical.width } else { y };
                stretched(
                    [sx, sy, horizontal.slice, vertical.slice],
                    [dx, dy, horizontal.width, vertical.width],
                )
            })
            .collect()
    }
    /** The four edges: each scales its slice to its width across and tiles along its length. */
    fn sides(&self) -> Vec<NinePiece> {
        let mut pieces = self.horizontal_edges();
        pieces.extend(self.vertical_edges());
        pieces
    }
    /** Top and bottom: tiled horizontally between the left and right corners. */
    fn horizontal_edges(&self) -> Vec<NinePiece> {
        let [top, right, bottom, left] = self.edges;
        let [iw, ih] = self.natural;
        let [x, y, w, h] = self.area;
        let (source_w, dest_w) = (iw - left.slice - right.slice, w - left.width - right.width);
        let rules = [self.repeat[0], BorderImageRepeat::Stretch];
        [
            (top, 0.0, y),
            (bottom, ih - bottom.slice, y + h - bottom.width),
        ]
        .into_iter()
        .filter(|(edge, ..)| edge.drawable() && source_w > 0.0 && dest_w > 0.0)
        .filter_map(|(edge, sy, dy)| {
            let source = [left.slice, sy, source_w, edge.slice];
            let dest = [x + left.width, dy, dest_w, edge.width];
            tiled(source, dest, [edge.scale(); 2], rules)
        })
        .collect()
    }
    /** Left and right: tiled vertically between the top and bottom corners. */
    fn vertical_edges(&self) -> Vec<NinePiece> {
        let [top, right, bottom, left] = self.edges;
        let [iw, ih] = self.natural;
        let [x, y, w, h] = self.area;
        let (source_h, dest_h) = (ih - top.slice - bottom.slice, h - top.width - bottom.width);
        let rules = [BorderImageRepeat::Stretch, self.repeat[1]];
        [
            (left, 0.0, x),
            (right, iw - right.slice, x + w - right.width),
        ]
        .into_iter()
        .filter(|(edge, ..)| edge.drawable() && source_h > 0.0 && dest_h > 0.0)
        .filter_map(|(edge, sx, dx)| {
            let source = [sx, top.slice, edge.slice, source_h];
            let dest = [dx, y + top.width, edge.width, dest_h];
            tiled(source, dest, [edge.scale(); 2], rules)
        })
        .collect()
    }
    /** The middle (`fill`): scaled like the edges beside it, stretched or tiled per axis. */
    fn middle(&self) -> Option<NinePiece> {
        let [top, right, bottom, left] = self.edges;
        let [iw, ih] = self.natural;
        let [x, y, w, h] = self.area;
        let source = [
            left.slice,
            top.slice,
            iw - left.slice - right.slice,
            ih - top.slice - bottom.slice,
        ];
        let dest = [
            x + left.width,
            y + top.width,
            w - left.width - right.width,
            h - top.width - bottom.width,
        ];
        if source[2] <= 0.0 || source[3] <= 0.0 || dest[2] <= 0.0 || dest[3] <= 0.0 {
            return None;
        }
        let beside = |near: Edge, far: Edge| {
            [near, far]
                .into_iter()
                .find(|edge| edge.drawable())
                .map_or(1.0, Edge::scale)
        };
        let mut scale = [beside(top, bottom), beside(left, right)];
        for axis in 0..2 {
            if self.repeat[axis] == BorderImageRepeat::Stretch {
                scale[axis] = dest[axis + 2] / source[axis + 2];
            }
        }
        tiled(source, dest, scale, self.repeat)
    }
}
/** A piece drawn once, its source stretched over its destination. */
fn stretched(source: [f64; 4], dest: [f64; 4]) -> NinePiece {
    NinePiece {
        source,
        dest,
        tile: [dest[2], dest[3]],
        phase: [0.0; 2],
        gap: [0.0; 2],
    }
}
/** A piece tiled per axis by its rule, after scaling its source by `scale`; None when a
 * `space` axis has no room for one whole tile, as `ComputeTileParameters` refuses it. */
fn tiled(
    source: [f64; 4],
    dest: [f64; 4],
    scale: [f64; 2],
    rules: [BorderImageRepeat; 2],
) -> Option<NinePiece> {
    if rules == [BorderImageRepeat::Stretch; 2] {
        return Some(stretched(source, dest));
    }
    let x = axis(rules[0], dest[2], source[2] * scale[0])?;
    let y = axis(rules[1], dest[3], source[3] * scale[1])?;
    Some(NinePiece {
        source,
        dest,
        tile: [x.0, y.0],
        phase: [x.1, y.1],
        gap: [x.2, y.2],
    })
}
/** One axis's (tile size, phase, gap) for a rule over a `dest` extent from a `tile` size. */
fn axis(rule: BorderImageRepeat, dest: f64, tile: f64) -> Option<(f64, f64, f64)> {
    if tile <= 0.0 || dest / tile > TILES {
        return None;
    }
    match rule {
        BorderImageRepeat::Stretch => Some((tile, 0.0, 0.0)),
        BorderImageRepeat::Repeat => Some((tile, (dest - tile) / 2.0, 0.0)),
        BorderImageRepeat::Round => {
            let count = (dest / tile).round().max(1.0);
            Some((dest / count, 0.0, 0.0))
        }
        BorderImageRepeat::Space => {
            let count = (dest / tile).floor();
            let gap = (count > 0.0).then(|| (dest - tile * count) / (count + 1.0))?;
            Some((tile, gap, gap))
        }
    }
}
