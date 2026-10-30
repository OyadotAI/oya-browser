/** SVG construction adapts normalized segments to Canvas bounds without creating partial retained objects. */
use super::*;
use crate::svg_path::{self, Segment};

impl Path {
    /** SVG syntax errors retain complete prefixes; numeric and quota failures refuse the whole construction. */
    pub fn from_svg(source: &str) -> EngineResult<Self> {
        let segments = svg_path::parse_prefix(source)?;
        let mut parsed = Self::default();
        for segment in segments {
            match segment {
                Segment::Move(x, y) => parsed.move_to([x, y], Affine::IDENTITY)?,
                Segment::Line(x, y) => parsed.line_to([x, y], Affine::IDENTITY)?,
                Segment::Cubic([x1, y1, x2, y2, x, y]) => {
                    parsed.bezier_curve_to([x1, y1], [x2, y2], [x, y], Affine::IDENTITY)?
                }
                Segment::Close => parsed.close(),
            }
        }
        let mut output = Self::default();
        output.add_path(&parsed, Affine::IDENTITY)?;
        Ok(output)
    }
}
