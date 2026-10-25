/**
 * Real glyphs for the windowless painter: each text run Eye shaped is drawn through CoreText
 * (AppKit string drawing, the same shaping `platform::measure` used for layout) into an
 * offscreen bitmap, whose coverage the raster blends in the item's ink. Placement follows
 * Blink's line box: the baseline sits the half-leading plus the font's rounded ascent below the
 * line's top, as `FontMetrics::AscentDescentWithHacks` and `CalculateLeadingSpace` compute them.
 */
use crate::document::{DisplayItem, PaintLine};
use crate::platform::{Face, Family};

/// Device pixels per CSS px Eye computes font metrics at (Blink's zoom-for-DSF on Retina Macs);
/// the baseline must round where layout rounded it.
const DEVICE_SCALE: f64 = 2.0;
/// Blink's macOS ascent hack for Times and Helvetica: 15% of the rounded height.
const CLASSIC_ASCENT: f64 = 0.15;
/// The used line advance when a display item predates `line_height`.
const NORMAL_LINE_HEIGHT: f64 = 1.2;
/// Ink may overhang the advance box (italics, accents): the bitmap reaches this many ems past it.
const OVERHANG: f64 = 1.0;
/// Bitmap edge cap per run, so a pathological run cannot allocate without bound.
const RUN_EDGE: f64 = 4096.0;

/** A run's antialiased coverage on the canvas pixel grid: `left`, `top` in canvas pixels. */
pub(super) struct Coverage {
    pub left: i64,
    pub top: i64,
    pub width: usize,
    pub height: usize,
    /// One byte per pixel, rows top first: 0 clear, 255 fully inked.
    pub alpha: Vec<u8>,
}

/** The face a display item measured and draws its text with. */
fn face(item: &DisplayItem) -> Face {
    Face {
        size: item.font_size,
        weight: item.font_weight,
        family: item.font_family,
        spacing: item.letter_spacing,
        italic: item.italic,
    }
}

/**
 * Ascent and `line-height: normal` as Blink rounds them: at the device font size ascent and
 * descent each round to whole device pixels and Times and Helvetica gain the classic ascent
 * hack on the rounded height. The platform folds that hack in unrounded, so it is taken out
 * first; a web font (Ahem) is used as its tables say.
 */
fn blink_metrics(face: Face) -> (f64, f64) {
    let device = Face {
        size: face.size * DEVICE_SCALE,
        ..face
    };
    let (ascent, normal) = crate::platform::metrics(device);
    let classic = matches!(face.family, Family::Serif | Family::SansSerif);
    let below = normal - ascent;
    let ascent = if classic {
        (ascent - CLASSIC_ASCENT * below) / (1.0 + CLASSIC_ASCENT)
    } else {
        ascent
    };
    let (ascent, below) = (ascent.round(), below.round());
    let hack = if classic {
        ((ascent + below) * CLASSIC_ASCENT + 0.5).floor()
    } else {
        0.0
    };
    (
        (ascent + hack) / DEVICE_SCALE,
        (ascent + hack + below) / DEVICE_SCALE,
    )
}

/** Distance from a line's top to its baseline: the floored half-leading plus the ascent. */
pub(super) fn baseline_offset(item: &DisplayItem) -> f64 {
    let (ascent, normal) = blink_metrics(face(item));
    let advance = item
        .line_height
        .unwrap_or(item.font_size * NORMAL_LINE_HEIGHT);
    let leading = ((advance - normal) / 2.0 * DEVICE_SCALE).floor() / DEVICE_SCALE;
    leading + ascent
}

/**
 * The coverage of one run drawn at its laid-out place, or None when nothing can draw it (no
 * text, zero size, or AppKit refused the offscreen bitmap) so the caller keeps its bar.
 * `origin` is the canvas's document position of pixel (0, 0).
 */
#[cfg(target_os = "macos")]
pub(super) fn rasterize(
    item: &DisplayItem,
    line: &PaintLine,
    baseline: f64,
    origin: (f64, f64),
) -> Option<Coverage> {
    use objc2::{AnyThread, rc::autoreleasepool};
    use objc2_app_kit::{
        NSAttributedStringNSExtendedStringDrawing, NSBitmapImageRep, NSColor,
        NSDeviceRGBColorSpace, NSGraphicsContext, NSStringDrawingOptions,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    if line.text.trim().is_empty() || item.font_size <= 0.0 {
        return None;
    }
    let size = item.font_size;
    let pad = size * OVERHANG;
    let left = (line.x - pad - origin.0).floor();
    let top = (line.y + baseline - 2.0 * size - origin.1).floor();
    let width = ((line.x + line.width + pad - origin.0).ceil() - left).clamp(1.0, RUN_EDGE);
    let height = ((line.y + baseline + size - origin.1).ceil() - top).clamp(1.0, RUN_EDGE);
    let (w, h) = (width as usize, height as usize);
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
        // Grayscale antialiasing only: subpixel smoothing would tint the coverage.
        objc2_core_graphics::CGContext::set_should_smooth_fonts(Some(&context.CGContext()), false);
        let text =
            crate::platform::styled(&line.text, face(item), &NSColor::whiteColor(), item.rtl);
        // The context is unflipped: y grows upward from the bitmap's bottom row, and without
        // the line-fragment option the rect's origin is the baseline.
        let at = NSPoint::new(
            line.x - (origin.0 + left),
            height - (line.y + baseline - (origin.1 + top)),
        );
        text.drawWithRect_options_context(
            NSRect::new(at, NSSize::new(line.width + pad, size)),
            NSStringDrawingOptions(0),
            None,
        );
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

/** Headless builds have no font rasterizer: every run keeps its bar. */
#[cfg(not(target_os = "macos"))]
pub(super) fn rasterize(
    _item: &DisplayItem,
    _line: &PaintLine,
    _baseline: f64,
    _origin: (f64, f64),
) -> Option<Coverage> {
    None
}
