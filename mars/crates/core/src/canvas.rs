/** Bounded, inert canvas pixels and synchronous native text support shared by the engines. */
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
mod affine;
mod clip;
mod composite;
mod paint;
mod path;
mod pixels;
mod raster;
mod text;
pub use affine::Affine;
pub use clip::ClipMask;
pub use composite::{COMPOSITE_WORK, Composite, composite, composite_coverage};
pub use paint::{
    GRADIENT_STOPS, Geometry, Gradient, Paint, Pattern, Repeat, Shader, Stop, TRANSPARENT,
};
pub use path::{
    DASH_INPUTS, DASH_VALUES, DashStyle, FillRule, LineCap, LineJoin, Path, PathPlan, StrokeStyle,
    paint_path, paint_path_with, plan_fill, plan_stroke, plan_stroke_dashed,
};
pub use pixels::Pixels;
pub use pixels::Reservation as Allocation;
pub use raster::{
    RasterPlan, blend, blit, clear_rect, fill_rect, fill_rect_with, plan_blit, plan_rect,
};
pub use text::{
    ALIGNS, Align, BASELINES, Baseline, DIRECTIONS, Direction, FontMetrics, TextMeasure,
    align_shift, baseline_offset, keyword, lookup, measure as text_measure,
};

/// Conservative initial edge limit; this is a Mars resource limit, not a web standard limit.
pub const EDGE: u32 = 512;
/// Straight-alpha sRGB channels per pixel.
pub const CHANNELS: usize = 4;
/// A page includes reachable detached canvases in its allocation allowance.
pub const PAGE_BYTES: usize = 4 * 1024 * 1024;
/// Decoded image sources have their own bounded page allowance, shared by saved candidates.
pub const IMAGE_SOURCE_BYTES: usize = PAGE_BYTES;
/// Saved clipping masks share a separate retained page allowance, without changing pixel admission.
pub const CLIP_BYTES: usize = PAGE_BYTES;
/// Prevent tiny canvases from multiplying context bookkeeping indefinitely.
pub const CONTEXTS: usize = 16;
/// Pixel work admitted per script request, including readback and export.
pub const WORK: usize = 8_000_000;
/// Bound synchronous native shaping input.
pub const TEXT_UNITS: usize = 2048;
/// PNG exports cannot consume an unbounded script string.
pub const PNG_BYTES: usize = 2 * 1024 * 1024;
/// The canonical font text a script reads back is bounded like any other script-supplied string.
pub const FONT_TEXT_BYTES: usize = 1024;
/// CSS numeric font weights run from 1 to 1000; 400 is normal.
pub const WEIGHT_RANGE: std::ops::RangeInclusive<u16> = 1..=1000;
/// HTML's default bitmap dimensions.
pub const DEFAULT_SIZE: [u32; 2] = [300, 150];

/** Share HTML content-attribute dimensions between bitmap allocation and intrinsic layout. */
pub fn dimension(value: &str) -> Option<u32> {
    let value = value.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let negative = value.starts_with('-');
    let value = if negative {
        &value[1..]
    } else {
        value.strip_prefix('+').unwrap_or(value)
    };
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits
        .parse::<u32>()
        .ok()
        .filter(|n| *n <= i32::MAX as u32 && (!negative || *n == 0))
}

/// Row-major, top-left-origin sRGB pixels; no OS handles or ambient authority cross this boundary.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bitmap {
    /// Intrinsic width, independent of CSS sizing.
    pub width: u32,
    /// Intrinsic height, independent of CSS sizing.
    pub height: u32,
    /// Unpremultiplied RGBA channels; an empty backing also represents an excessive size.
    pub pixels: Pixels,
}

impl Bitmap {
    /// Allocate only admitted dimensions; excessive dimensions retain their observable size.
    pub fn new(width: u32, height: u32) -> EngineResult<Self> {
        let len = Self::length(width, height).unwrap_or(0);
        Ok(Self {
            width,
            height,
            pixels: Pixels::zeroed(len)?,
        })
    }
    /// Compute before allocating or indexing; zero and excessive surfaces have no backing.
    pub fn length(width: u32, height: u32) -> Option<usize> {
        if width > EDGE || height > EDGE {
            return None;
        }
        Some(width as usize * height as usize * CHANNELS)
    }
    /// Reject malformed service/state input before any painter sees it.
    pub fn validate(&self) -> EngineResult<()> {
        if self.pixels.len() != Self::length(self.width, self.height).unwrap_or(0) {
            return Err(EngineError::invalid("Invalid canvas bitmap length."));
        }
        Ok(())
    }
    /// Real pixels are present only for nonempty admitted dimensions.
    pub fn drawable(&self) -> bool {
        !self.pixels.is_empty()
    }
}

/// A resolved font: the shorthand's size, weight and posture, the generic family hosts draw with,
/// and the canonical text the font getter reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Font {
    /// CSS pixel size.
    pub size: f64,
    /// Generic family: serif, sans-serif or monospace. Unknown families map to the closest one.
    pub family: String,
    /// CSS numeric weight; 400 is normal and 700 is bold.
    pub weight: u16,
    /// Italic or oblique posture.
    pub italic: bool,
    /// Canonical CSS serialization, such as "bold 11px Arial", that the getter returns verbatim.
    pub text: String,
}
impl Default for Font {
    /// Canvas starts with the web platform's default font.
    fn default() -> Self {
        Self {
            size: 10.0,
            family: "sans-serif".into(),
            weight: 400,
            italic: false,
            text: "10px sans-serif".into(),
        }
    }
}

/// Native drawing is synchronous because script readback observes each operation immediately.
pub trait CanvasHost: Sync {
    /// False means 2D context creation must return null rather than promise drawing support.
    fn available(&self) -> bool {
        false
    }
    /// Ascent and descent of the font box for the same resolution drawing uses.
    fn font_metrics(&self, _font: &Font) -> EngineResult<FontMetrics> {
        Err(unavailable())
    }
    /// Measure through the same font resolution as drawing.
    fn measure(&self, _font: &Font, _text: &str) -> EngineResult<f64> {
        Err(unavailable())
    }
    /// Draw text into a fresh transparent bitmap at an alphabetic baseline, without retaining it.
    fn text(
        &self,
        _bitmap: &Bitmap,
        _font: &Font,
        _text: &str,
        _at: [f64; 2],
        _color: [u8; 4],
    ) -> EngineResult<Bitmap> {
        Err(unavailable())
    }
    /// Hosts that support transforms must rasterize before clipping, not transform clipped glyphs.
    fn text_transformed(
        &self,
        bitmap: &Bitmap,
        font: &Font,
        text: &str,
        at: [f64; 2],
        color: [u8; 4],
        transform: Affine,
    ) -> EngineResult<Bitmap> {
        if transform == Affine::IDENTITY {
            return self.text(bitmap, font, text, at, color);
        }
        Err(EngineError::unsupported(
            "Transformed Canvas text is unavailable in this host.",
        ))
    }
    /// Encode actual pixels. Unsupported MIME types use this PNG path as well.
    fn png(&self, _bitmap: &Bitmap) -> EngineResult<Vec<u8>> {
        Err(unavailable())
    }
}

/// Standalone V9 consumers retain their previous lack of native graphics capabilities.
pub struct Unavailable;
impl CanvasHost for Unavailable {}
/** A missing host never silently fabricates drawing output. */
fn unavailable() -> EngineError {
    EngineError::unsupported("Canvas 2D is unavailable in this host.")
}
