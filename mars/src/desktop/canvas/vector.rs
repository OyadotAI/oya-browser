/** Eye's resolved SVG commands replay through AppKit paths; AppKit never parses SVG. */
use crate::document::{DisplayItem, FillRule, LineCap, LineJoin, PathCommand, VectorPath};
use objc2::rc::Retained;
use objc2_app_kit::{NSBezierPath, NSColor, NSLineCapStyle, NSLineJoinStyle, NSWindingRule};
use objc2_foundation::NSPoint;

/// Fills, then strokes, every path of one svg item at its residual opacity after any group.
pub(super) fn paint(
    item: &DisplayItem,
    alpha: f64,
    color: impl Fn(&str) -> Option<Retained<NSColor>>,
) {
    for vector in &item.vector {
        let path = bezier(vector);
        path.setWindingRule(winding(vector.fill_rule));
        if let Some(fill) = vector.fill.as_deref().and_then(&color) {
            super::faded(&fill, alpha * vector.fill_opacity).setFill();
            path.fill();
        }
        if let Some(stroke) = vector.stroke.as_deref().and_then(&color) {
            super::faded(&stroke, alpha * vector.stroke_opacity).setStroke();
            path.setLineWidth(vector.stroke_width);
            path.setLineJoinStyle(join(vector.line_join));
            path.setLineCapStyle(cap(vector.line_cap));
            path.stroke();
        }
    }
}

/** One AppKit path per vector, in the same absolute document points as every other item. */
fn bezier(vector: &VectorPath) -> Retained<NSBezierPath> {
    let path = NSBezierPath::bezierPath();
    for command in &vector.commands {
        // AppKit raises an uncatchable exception for a segment before any current point.
        if path.isEmpty() && !matches!(command, PathCommand::Move { .. }) {
            continue;
        }
        match *command {
            PathCommand::Move { x, y } => path.moveToPoint(NSPoint::new(x, y)),
            PathCommand::Line { x, y } => path.lineToPoint(NSPoint::new(x, y)),
            PathCommand::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => path.curveToPoint_controlPoint1_controlPoint2(
                NSPoint::new(x, y),
                NSPoint::new(x1, y1),
                NSPoint::new(x2, y2),
            ),
            PathCommand::Close => path.closePath(),
        }
    }
    path
}

/** SVG line joins map one-to-one onto AppKit's. */
fn join(join: LineJoin) -> NSLineJoinStyle {
    match join {
        LineJoin::Miter => NSLineJoinStyle::Miter,
        LineJoin::Round => NSLineJoinStyle::Round,
        LineJoin::Bevel => NSLineJoinStyle::Bevel,
    }
}

/** SVG line caps map one-to-one onto AppKit's. */
fn cap(cap: LineCap) -> NSLineCapStyle {
    match cap {
        LineCap::Butt => NSLineCapStyle::Butt,
        LineCap::Round => NSLineCapStyle::Round,
        LineCap::Square => NSLineCapStyle::Square,
    }
}

/** SVG fill rules map one-to-one onto AppKit's winding rules. */
fn winding(rule: FillRule) -> NSWindingRule {
    match rule {
        FillRule::NonZero => NSWindingRule::NonZero,
        FillRule::EvenOdd => NSWindingRule::EvenOdd,
    }
}
