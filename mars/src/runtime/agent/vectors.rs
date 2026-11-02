/**
 * Inline SVG for the windowless painter: each path Eye resolved is filled or stroked through
 * AppKit (as the desktop canvas draws it) into an offscreen bitmap, whose coverage the raster
 * blends in the path's color, as it does a glyph run's. Without it a screenshot left every
 * inline `<svg>` blank.
 */
use super::glyphs::Coverage;
use crate::document::{DisplayItem, PathCommand, VectorPath};

/// Bitmap edge cap per path, so a pathological path cannot allocate without bound.
const PATH_EDGE: f64 = 4096.0;

/** Which part of a path to draw. */
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Part {
    /// The interior, by the path's fill rule.
    Fill,
    /// The outline, at its stroke width, joins and caps.
    Stroke,
}

/**
 * The coverage of one path part, sized to what the canvas and the item's clip let show; an
 * empty coverage when nothing shows, None when the screenshot's `budget` of pixels is spent or
 * AppKit refused the bitmap. `canvas` is `[left, top, width, height]`, its document position of
 * pixel (0, 0) first.
 */
#[cfg(target_os = "macos")]
pub(super) fn rasterize(
    item: &DisplayItem,
    path: &VectorPath,
    part: Part,
    canvas: [f64; 4],
    budget: &mut usize,
) -> Option<Coverage> {
    use objc2::{AnyThread, rc::autoreleasepool};
    use objc2_app_kit::{NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSGraphicsContext};
    let Some([left, top, right, bottom]) = visible(item, path, part, canvas) else {
        return Some(empty());
    };
    let (w, h) = ((right - left) as usize, (bottom - top) as usize);
    *budget = budget.checked_sub(w * h)?;
    let origin = (canvas[0] + left, canvas[1] + top, h as f64);
    autoreleasepool(|_| {
        // SAFETY: null planes ask AppKit to allocate an 8-bit premultiplied RGBA buffer of the
        // given size, which it owns; the color space name is Apple's immutable constant.
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                w as isize,
                h as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                (w * 4) as isize,
                32,
            )
        }?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let bezier = bezier(path, origin);
        match part {
            Part::Fill => {
                NSColor::whiteColor().setFill();
                bezier.fill();
            }
            Part::Stroke => {
                NSColor::whiteColor().setStroke();
                bezier.setLineWidth(path.stroke_width);
                bezier.setLineJoinStyle(join(path.line_join));
                bezier.setLineCapStyle(cap(path.line_cap));
                bezier.stroke();
            }
        }
        context.flushGraphics();
        NSGraphicsContext::restoreGraphicsState_class();
        let data = rep.bitmapData();
        if data.is_null() {
            return None;
        }
        // SAFETY: the rep owns a w * h * 4 byte buffer (bytesPerRow was given as w * 4).
        let pixels = unsafe { std::slice::from_raw_parts(data, w * h * 4) };
        Some(Coverage {
            left: left as i64,
            top: top as i64,
            width: w,
            height: h,
            alpha: pixels.chunks_exact(4).map(|p| p[3]).collect(),
        })
    })
}

/** Headless builds have no path rasterizer: inline SVG stays unpainted. */
#[cfg(not(target_os = "macos"))]
pub(super) fn rasterize(
    _item: &DisplayItem,
    _path: &VectorPath,
    _part: Part,
    _canvas: [f64; 4],
    _budget: &mut usize,
) -> Option<Coverage> {
    None
}

/** Coverage that paints nothing. */
#[cfg(target_os = "macos")]
fn empty() -> Coverage {
    Coverage {
        left: 0,
        top: 0,
        width: 0,
        height: 0,
        alpha: Vec::new(),
    }
}

/** The path's ink box (control points included, a stroke's half width and miters around it) cut
to the canvas and the item's clip, as canvas pixels `[left, top, right, bottom]`. */
#[cfg(target_os = "macos")]
fn visible(
    item: &DisplayItem,
    path: &VectorPath,
    part: Part,
    canvas: [f64; 4],
) -> Option<[f64; 4]> {
    let [ox, oy, width, height] = canvas;
    let points = path.commands.iter().flat_map(|command| match *command {
        PathCommand::Move { x, y } | PathCommand::Line { x, y } => vec![(x, y)],
        PathCommand::Cubic {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => vec![(x1, y1), (x2, y2), (x, y)],
        PathCommand::Close => Vec::new(),
    });
    let mut ink = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for (x, y) in points {
        ink = [ink[0].min(x), ink[1].min(y), ink[2].max(x), ink[3].max(y)];
    }
    // A miter can reach far past the half width; a generous margin keeps it in the bitmap.
    let pad = if part == Part::Stroke {
        path.stroke_width * 4.0 + 1.0
    } else {
        1.0
    };
    let mut rect = [
        (ink[0] - pad - ox).floor().max(0.0),
        (ink[1] - pad - oy).floor().max(0.0),
        (ink[2] + pad - ox).ceil().min(width),
        (ink[3] + pad - oy).ceil().min(height),
    ];
    if let Some(c) = item.clip {
        rect = [
            rect[0].max((c.x - ox).floor()),
            rect[1].max((c.y - oy).floor()),
            rect[2].min((c.x + c.width - ox).ceil()),
            rect[3].min((c.y + c.height - oy).ceil()),
        ];
    }
    rect[2] = rect[2].min(rect[0] + PATH_EDGE);
    rect[3] = rect[3].min(rect[1] + PATH_EDGE);
    (rect[2] > rect[0] && rect[3] > rect[1]).then_some(rect)
}

/** The path in bitmap points: `origin` is the document position of the bitmap's top-left
pixel and the bitmap's height, since the context is unflipped (y grows upward). */
#[cfg(target_os = "macos")]
fn bezier(
    path: &VectorPath,
    (x0, y0, height): (f64, f64, f64),
) -> objc2::rc::Retained<objc2_app_kit::NSBezierPath> {
    use objc2_foundation::NSPoint;
    let at = |x: f64, y: f64| NSPoint::new(x - x0, height - (y - y0));
    let bezier = objc2_app_kit::NSBezierPath::bezierPath();
    bezier.setWindingRule(match path.fill_rule {
        crate::document::FillRule::NonZero => objc2_app_kit::NSWindingRule::NonZero,
        crate::document::FillRule::EvenOdd => objc2_app_kit::NSWindingRule::EvenOdd,
    });
    for command in &path.commands {
        // AppKit raises an uncatchable exception for a segment before any current point.
        if bezier.isEmpty() && !matches!(command, PathCommand::Move { .. }) {
            continue;
        }
        match *command {
            PathCommand::Move { x, y } => bezier.moveToPoint(at(x, y)),
            PathCommand::Line { x, y } => bezier.lineToPoint(at(x, y)),
            PathCommand::Cubic {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => bezier.curveToPoint_controlPoint1_controlPoint2(at(x, y), at(x1, y1), at(x2, y2)),
            PathCommand::Close => bezier.closePath(),
        }
    }
    bezier
}

/** SVG line joins map one-to-one onto AppKit's. */
#[cfg(target_os = "macos")]
fn join(join: crate::document::LineJoin) -> objc2_app_kit::NSLineJoinStyle {
    use crate::document::LineJoin;
    use objc2_app_kit::NSLineJoinStyle;
    match join {
        LineJoin::Miter => NSLineJoinStyle::Miter,
        LineJoin::Round => NSLineJoinStyle::Round,
        LineJoin::Bevel => NSLineJoinStyle::Bevel,
    }
}

/** SVG line caps map one-to-one onto AppKit's. */
#[cfg(target_os = "macos")]
fn cap(cap: crate::document::LineCap) -> objc2_app_kit::NSLineCapStyle {
    use crate::document::LineCap;
    use objc2_app_kit::NSLineCapStyle;
    match cap {
        LineCap::Butt => NSLineCapStyle::Butt,
        LineCap::Round => NSLineCapStyle::Round,
        LineCap::Square => NSLineCapStyle::Square,
    }
}
