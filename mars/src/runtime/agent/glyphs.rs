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
/// Glyph bitmap pixels one screenshot may draw across all of its runs, four full 2048-pixel
/// canvases' worth; past it runs paint as bars, so thousands of runs cannot multiply the cap.
pub(super) const SHOT_GLYPH_PIXELS: usize = 4 * 2048 * 2048;

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
pub(super) fn face(item: &DisplayItem) -> Face {
    Face {
        size: item.font_size,
        weight: item.font_weight,
        family: item.font_family,
        spacing: item.letter_spacing,
        italic: item.italic,
        autospace: item.autospace,
        upright: crate::platform::upright::TURNED,
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
 * The font's own ascent at the item's size, before Blink's classic-face hack: where a glyph's
 * ink tops out for a font whose glyphs fill the em (Ahem).
 */
pub(super) fn true_ascent(face: Face) -> f64 {
    let (ascent, normal) = crate::platform::metrics(face);
    if matches!(face.family, Family::Serif | Family::SansSerif) {
        (ascent - CLASSIC_ASCENT * (normal - ascent)) / (1.0 + CLASSIC_ASCENT)
    } else {
        ascent
    }
}

/**
 * The document y a horizontal run's baseline is drawn at. Blink paints a run on the whole
 * pixel nearest its laid-out baseline, and the hinting rasterizer of the reference renderings
 * snaps the font's ascent line to the pixel grid as well, so a box glyph (Ahem) fills exactly the
 * rows a box of its em fills. CoreText does not hint, so the run is drawn off the pixel by the
 * ascent's fraction instead; without it every Ahem square at a size that is not a multiple of
 * five gains a gray edge row its reference box lacks. `hinted` false (a vertical run, turned
 * after drawing, or a built-in face) keeps the laid-out baseline.
 */
pub(super) fn drawn_baseline(item: &DisplayItem, baseline: f64, hinted: bool) -> f64 {
    if !hinted {
        return baseline;
    }
    let ascent = true_ascent(face(item));
    baseline.round() + ascent - ascent.round()
}

/**
 * Lets CoreText place glyphs at fractional positions on both axes. By default it quantizes a
 * run's origin to whole pixels vertically, which would undo `drawn_baseline`'s fraction.
 */
#[cfg(target_os = "macos")]
fn unquantized(context: &objc2_core_graphics::CGContext) {
    use objc2_core_graphics::CGContext;
    CGContext::set_allows_font_subpixel_quantization(Some(context), false);
    CGContext::set_should_subpixel_quantize_fonts(Some(context), false);
}

/// Coverage this close to clear or solid is rounding residue of an edge on a pixel boundary.
const RESIDUE: u8 = 1;

/** Coverage with an on-boundary edge's residue settled to clear or solid, so a glyph edge that
lands exactly on a pixel row inks it as a box edge would. */
#[cfg(target_os = "macos")]
pub(super) fn settled(alpha: u8) -> u8 {
    match alpha {
        a if a <= RESIDUE => 0,
        a if a >= u8::MAX - RESIDUE => u8::MAX,
        a => a,
    }
}

/**
 * The part of a run's ink box that can show, as canvas pixels `[left, top, right, bottom]`: the
 * box cut to the canvas and the item's clip, then to `RUN_EDGE`. None when nothing shows, so an
 * off-canvas or clipped-out run allocates nothing. `canvas` is `[left, top, width, height]`
 * with its document position of pixel (0, 0) first.
 */
#[cfg(target_os = "macos")]
fn visible(
    item: &DisplayItem,
    line: &PaintLine,
    baseline: f64,
    canvas: [f64; 4],
) -> Option<[f64; 4]> {
    let (size, [ox, oy, width, height]) = (item.font_size, canvas);
    let pad = size * OVERHANG;
    let mut rect = [
        (line.x - pad - ox).floor(),
        (line.y + baseline - 2.0 * size - oy).floor(),
        (line.x + line.width + pad - ox).ceil(),
        (line.y + baseline + size - oy).ceil(),
    ];
    let mut cut = |[l, t, r, b]: [f64; 4]| {
        rect = [
            rect[0].max(l),
            rect[1].max(t),
            rect[2].min(r),
            rect[3].min(b),
        ];
    };
    cut([0.0, 0.0, width, height]);
    if let Some(c) = item.clip {
        cut([
            (c.x - ox).floor(),
            (c.y - oy).floor(),
            (c.x + c.width - ox).ceil(),
            (c.y + c.height - oy).ceil(),
        ]);
    }
    rect[2] = rect[2].min(rect[0] + RUN_EDGE);
    rect[3] = rect[3].min(rect[1] + RUN_EDGE);
    (rect[2] > rect[0] && rect[3] > rect[1]).then_some(rect)
}

/** Coverage that paints nothing: the run shows nowhere, and needs neither a bitmap nor a bar. */
#[cfg(target_os = "macos")]
fn hidden() -> Coverage {
    Coverage {
        left: 0,
        top: 0,
        width: 0,
        height: 0,
        alpha: Vec::new(),
    }
}

/**
 * The coverage of one run drawn at its laid-out place, sized to the part of it the canvas and
 * the item's clip let show, or None when nothing can draw it (no text, zero size, the
 * screenshot's `budget` of glyph pixels is spent, or AppKit refused the offscreen bitmap) so
 * the caller keeps its bar. `canvas` is `[left, top, width, height]`, its document position of
 * pixel (0, 0) first; a drawn run takes its bitmap's pixels from `budget`.
 */
#[cfg(target_os = "macos")]
pub(super) fn rasterize(
    item: &DisplayItem,
    line: &PaintLine,
    baseline: f64,
    canvas: [f64; 4],
    budget: &mut usize,
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
    let Some([left, top, right, bottom]) = visible(item, line, baseline, canvas) else {
        return Some(hidden());
    };
    let (size, origin) = (item.font_size, (canvas[0], canvas[1]));
    let pad = size * OVERHANG;
    let height = bottom - top;
    let (w, h) = ((right - left) as usize, height as usize);
    *budget = budget.checked_sub(w * h)?;
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
        // Only a page's own fonts (Ahem among them) are hinted: the built-in faces keep
        // CoreText's quantized placement, which runs split at different points rely on to draw
        // the same glyphs.
        let hinted = item.writing == 0 && matches!(item.font_family, Family::Web(_));
        if hinted {
            unquantized(&context.CGContext());
        }
        let text = crate::platform::spaced(
            &line.text,
            face(item),
            &NSColor::whiteColor(),
            item.rtl,
            item.word_spacing,
        );
        // The context is unflipped: y grows upward from the bitmap's bottom row, and without
        // the line-fragment option the rect's origin is the baseline.
        let at = NSPoint::new(
            line.x - (origin.0 + left),
            height - (drawn_baseline(item, line.y + baseline, hinted) - (origin.1 + top)),
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
            alpha: pixels.chunks_exact(4).map(|p| settled(p[3])).collect(),
        })
    })
}

/** Headless builds have no font rasterizer: every run keeps its bar. */
#[cfg(not(target_os = "macos"))]
pub(super) fn rasterize(
    _item: &DisplayItem,
    _line: &PaintLine,
    _baseline: f64,
    _canvas: [f64; 4],
    _budget: &mut usize,
) -> Option<Coverage> {
    None
}

/// The scratch canvas a vertical run rasterizes on before it turns: big enough that nothing is
/// cut (RUN_EDGE bounds the run), centered on the run's origin.
const TURN_CANVAS: f64 = 2.0 * RUN_EDGE;

/**
 * A run in a vertical writing mode: rasterized as a horizontal run at the origin, then its
 * coverage turned a quarter clockwise so a local point (u, v) lands at (x + advance - v, y + u),
 * the run's `line.x`, `line.y` being its physical top-left and `advance` its line box's width.
 * The desktop canvas turns its drawing context the same way.
 */
pub(super) fn rasterize_vertical(
    item: &DisplayItem,
    line: &PaintLine,
    baseline: f64,
    canvas: [f64; 4],
    budget: &mut usize,
) -> Option<Coverage> {
    let advance = item
        .line_height
        .unwrap_or(item.font_size * NORMAL_LINE_HEIGHT);
    let mut flat = item.clone();
    flat.clip = None;
    let local = PaintLine {
        text: line.text.clone(),
        x: 0.0,
        y: 0.0,
        width: line.width,
        color: line.color.clone(),
    };
    let scratch = [
        -TURN_CANVAS / 2.0,
        -TURN_CANVAS / 2.0,
        TURN_CANVAS,
        TURN_CANVAS,
    ];
    let flat = rasterize(&flat, &local, baseline, scratch, budget)?;
    // The run starts on a whole pixel along its inline axis, as Blink snaps a text origin.
    let start = line.y.round();
    Some(if item.writing == crate::document::WRITING_CCW {
        turn_back(&flat, scratch, (line.x, start + line.width), canvas)
    } else {
        turn(&flat, scratch, (line.x + advance, start), canvas)
    })
}

/** Coverage turned a quarter counter-clockwise about the local origin, which lands at document
point `at`: a local point (u, v) goes to (at.x + v, at.y - u), as `sideways-lr` text and a
horizontal box inside a `vertical-rl` one read bottom to top. */
fn turn_back(flat: &Coverage, scratch: [f64; 4], at: (f64, f64), canvas: [f64; 4]) -> Coverage {
    let (w, h) = (flat.width, flat.height);
    let (u0, v0) = (scratch[0] + flat.left as f64, scratch[1] + flat.top as f64);
    let left = (at.0 + v0 - canvas[0]).round() as i64;
    let top = (at.1 - (u0 + w as f64) - canvas[1]).round() as i64;
    let mut alpha = vec![0; w * h];
    for row in 0..h {
        for column in 0..w {
            // Turned back: the flat row becomes the column, the flat column counts up from the bottom.
            alpha[(w - 1 - column) * h + row] = flat.alpha[row * w + column];
        }
    }
    Coverage {
        left,
        top,
        width: h,
        height: w,
        alpha,
    }
}

/** Coverage rasterized on `scratch` turned a quarter clockwise about the local origin, which
lands at document point `at`, onto `canvas` pixels. */
fn turn(flat: &Coverage, scratch: [f64; 4], at: (f64, f64), canvas: [f64; 4]) -> Coverage {
    let (w, h) = (flat.width, flat.height);
    // The flat bitmap's local top-left, then where its top-right corner lands once turned.
    let (u0, v0) = (scratch[0] + flat.left as f64, scratch[1] + flat.top as f64);
    let left = (at.0 - (v0 + h as f64) - canvas[0]).round() as i64;
    let top = (at.1 + u0 - canvas[1]).round() as i64;
    let mut alpha = vec![0; w * h];
    for row in 0..h {
        for column in 0..w {
            // Turned: the flat column becomes the row, the flat row counts in from the right.
            alpha[column * h + (h - 1 - row)] = flat.alpha[row * w + column];
        }
    }
    Coverage {
        left,
        top,
        width: h,
        height: w,
        alpha,
    }
}
