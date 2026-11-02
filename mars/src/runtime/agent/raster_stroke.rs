/** Strokes of background SVG drawings: each segment as a quad the stroke's width wide, so the
 * scanline filler paints the outline the same way it paints fills. */
use crate::document::PathCommand;

/**
 * The outline of `commands` stroked `width` wide, as edges `[x0, y0, x1, y1]` scaled by `sx`,
 * `sy`: one quad per straight piece (cubics flattened into `steps` pieces), each extended half
 * the width past both ends so right-angled corners meet squarely (a miter join), and wound the
 * same way so the nonzero rule paints their union once.
 */
// ponytail: square ends stand in for every join and cap; round joins on sharp curves and
// butt caps on open paths differ at their ends by up to half the width.
pub(super) fn outline(
    commands: &[PathCommand],
    width: f64,
    (sx, sy): (f64, f64),
    steps: usize,
) -> Vec<[f64; 4]> {
    let half = width * (sx + sy) / 4.0;
    segments(commands, steps)
        .into_iter()
        .flat_map(|[x0, y0, x1, y1]| quad([x0 * sx, y0 * sy], [x1 * sx, y1 * sy], half))
        .collect()
}

/** The four edges of one segment's quad, `half` to each side and past each end. */
fn quad([x0, y0]: [f64; 2], [x1, y1]: [f64; 2], half: f64) -> Vec<[f64; 4]> {
    let length = (x1 - x0).hypot(y1 - y0);
    if length <= 0.0 || half <= 0.0 {
        return Vec::new();
    }
    let (dx, dy) = ((x1 - x0) / length * half, (y1 - y0) / length * half);
    // Clockwise in screen space whichever way the segment runs, so overlapping quads add up.
    let corners = [
        [x0 - dx - dy, y0 - dy + dx],
        [x1 + dx - dy, y1 + dy + dx],
        [x1 + dx + dy, y1 + dy - dx],
        [x0 - dx + dy, y0 - dy - dx],
    ];
    (0..corners.len())
        .map(|at| {
            let [ax, ay] = corners[at];
            let [bx, by] = corners[(at + 1) % corners.len()];
            [ax, ay, bx, by]
        })
        .collect()
}

/** The straight pieces a stroke follows: lines, flattened cubics, and explicit closes only
 * (a fill's implicit close is not stroked). */
fn segments(commands: &[PathCommand], steps: usize) -> Vec<[f64; 4]> {
    let mut out = Vec::new();
    let (mut start, mut at) = ((0.0, 0.0), (0.0, 0.0));
    for command in commands {
        let next = match *command {
            PathCommand::Move { x, y } => {
                (start, at) = ((x, y), (x, y));
                continue;
            }
            PathCommand::Line { x, y } => vec![(x, y)],
            PathCommand::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => cubic(at, [x1, y1, x2, y2, x, y], steps),
            PathCommand::Close => vec![start],
        };
        for point in next {
            out.push([at.0, at.1, point.0, point.1]);
            at = point;
        }
    }
    out
}

/** A cubic from `from` through its control points, as `steps` points along it. */
fn cubic(from: (f64, f64), [x1, y1, x2, y2, x, y]: [f64; 6], steps: usize) -> Vec<(f64, f64)> {
    (1..=steps)
        .map(|step| {
            let t = step as f64 / steps as f64;
            let u = 1.0 - t;
            let point = |a: f64, b: f64, c: f64, d: f64| {
                u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
            };
            (point(from.0, x1, x2, x), point(from.1, y1, y2, y))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A closed square's stroke is four quads, sixteen edges, reaching half the width out. */
    #[test]
    fn a_square_strokes_as_four_quads() {
        let square = [
            PathCommand::Move { x: 0.0, y: 0.0 },
            PathCommand::Line { x: 10.0, y: 0.0 },
            PathCommand::Line { x: 10.0, y: 10.0 },
            PathCommand::Line { x: 0.0, y: 10.0 },
            PathCommand::Close,
        ];
        let edges = outline(&square, 2.0, (1.0, 1.0), 1);
        assert_eq!(edges.len(), 16);
        let lowest = edges
            .iter()
            .map(|e| e[1].min(e[3]))
            .fold(f64::INFINITY, f64::min);
        assert_eq!(lowest, -1.0);
    }
}
