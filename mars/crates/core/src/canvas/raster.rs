use super::clip::{GRID, SAMPLES, offset};
/** Checked raster plans charge work before one fallible copy-on-write mutation. */
use super::{Affine, Bitmap, CHANNELS, ClipMask, Composite, EDGE, composite};
use crate::protocol::{EngineError, EngineResult};
mod geometry;
mod sampling;

/** Coverage clips a small convex polygon against the four pixel edges. */
const COVERAGE_WORK: usize = 16;
/** Bilinear filtering reads four source pixels in addition to coverage work. */
const SMOOTHING_WORK: usize = 4;

/** Immutable plans cannot be forged through private VM serialization. */
#[derive(Clone, Debug)]
pub struct RasterPlan {
    size: [u32; 2],
    polygon: [[f64; 2]; 4],
    bounds: [u32; 4],
    work: usize,
    image: Option<Image>,
    clip: Option<ClipMask>,
    mode: Composite,
    inverse: Affine,
    rectangle: [f64; 4],
}

/** Image coordinates stay floating point until the source lookup is clipped and bounded. */
#[derive(Clone, Debug)]
struct Image {
    size: [u32; 2],
    crop: [f64; 4],
    placement: [f64; 4],
    inverse: Affine,
    smoothing: bool,
}

impl RasterPlan {
    /** Supported operators preserve existing bounds, clipping and fixed per-pixel work admission. */
    pub fn with_composite(mut self, mode: Composite) -> Self {
        self.mode = mode;
        self
    }

    /** The interpreter admits this amount before any backing allocation or mutation. */
    pub fn work(&self) -> usize {
        self.work
    }
    /** Singular, empty and fully clipped operations do not force a COW allocation. */
    pub fn is_empty(&self) -> bool {
        self.bounds[0] == self.bounds[2] || self.bounds[1] == self.bounds[3]
    }

    /** Optional mask samples are validated and shared before any destination COW can begin. */
    pub fn with_clip(mut self, clip: Option<&ClipMask>) -> EngineResult<Self> {
        if let Some(clip) = clip {
            clip.validate(self.size)?;
        }
        let pixels =
            (self.bounds[2] - self.bounds[0]) as usize * (self.bounds[3] - self.bounds[1]) as usize;
        self.work = self.work - pixels * usize::from(self.clip.is_some())
            + pixels * usize::from(clip.is_some());
        self.clip = clip.cloned();
        Ok(self)
    }
}

/** Plan transformed geometry without allocating pixels or changing script-visible state. */
pub fn plan_rect(size: [u32; 2], rect: [f64; 4], transform: Affine) -> EngineResult<RasterPlan> {
    validate_size(size)?;
    if !transform.finite() {
        return Err(invalid());
    }
    let rect = geometry::normalized(rect)?;
    if rect[2] == 0.0 || rect[3] == 0.0 || transform.inverse().is_none() {
        return Ok(empty(size));
    }
    let polygon = geometry::quadrilateral(rect, transform)?;
    let bounds = geometry::bounds(&polygon, size);
    let pixels = (bounds[2] - bounds[0]) as usize * (bounds[3] - bounds[1]) as usize;
    Ok(RasterPlan {
        size,
        polygon,
        bounds,
        work: pixels * COVERAGE_WORK,
        image: None,
        clip: None,
        mode: Composite::default(),
        inverse: transform.inverse().expect("nonempty rectangle inverse"),
        rectangle: rect,
    })
}

/** Normalize and clip source crop before planning the corresponding destination quadrilateral. */
pub fn plan_blit(
    source: [u32; 2],
    destination: [u32; 2],
    crop: [f64; 4],
    placement: [f64; 4],
    transform: Affine,
    smoothing: bool,
) -> EngineResult<RasterPlan> {
    validate_size(source)?;
    validate_size(destination)?;
    let crop = geometry::normalized(crop)?;
    let placement = geometry::normalized(placement)?;
    let Some((crop, placement)) = clipped_image(source, crop, placement) else {
        return Ok(empty(destination));
    };
    let mut plan = plan_rect(destination, placement, transform)?;
    if plan.is_empty() {
        return Ok(plan);
    }
    let pixels = plan.work / COVERAGE_WORK;
    plan.work += pixels * if smoothing { SMOOTHING_WORK } else { 1 };
    plan.image = Some(Image {
        size: source,
        crop,
        placement,
        inverse: transform.inverse().expect("nonempty inverse"),
        smoothing,
    });
    Ok(plan)
}

/** Solid paint preserves exact fractional coverage and composites with the selected supported operator. */
pub fn fill_rect(
    destination: &mut Bitmap,
    plan: &RasterPlan,
    color: [u8; 4],
    alpha: f64,
) -> EngineResult<()> {
    validate_alpha(alpha)?;
    paint(destination, plan, |pixel, x, y| {
        let coverage = coverage(plan, x, y);
        if coverage == 0.0 {
            return;
        }
        composite(pixel, color, alpha * coverage, plan.mode);
    })
}

/** Clearing removes the covered fraction regardless of current paint alpha. */
pub fn clear_rect(destination: &mut Bitmap, plan: &RasterPlan) -> EngineResult<()> {
    paint(destination, plan, |pixel, x, y| {
        let coverage = coverage(plan, x, y);
        if coverage == 0.0 {
            return;
        }
        pixel[3] = (f64::from(pixel[3]) * (1.0 - coverage)).round() as u8;
        if pixel[3] == 0 {
            pixel.fill(0);
        }
    })
}

/** Immutable source storage naturally snapshots overlapping self-copies before destination COW. */
pub fn blit(
    destination: &mut Bitmap,
    source: &Bitmap,
    plan: &RasterPlan,
    alpha: f64,
) -> EngineResult<()> {
    source.validate()?;
    validate_alpha(alpha)?;
    if plan.is_empty() {
        return validate_destination(destination, plan);
    }
    let image = plan.image.as_ref().ok_or_else(invalid)?;
    if image.size != [source.width, source.height] || !source.drawable() {
        return Err(invalid());
    }
    paint(destination, plan, |pixel, x, y| {
        let coverage = coverage(plan, x, y);
        if coverage == 0.0 {
            return;
        }
        let local = image.inverse.map([f64::from(x) + 0.5, f64::from(y) + 0.5]);
        let at = image_position(image, local);
        let color = sampling::sample(source, at, image.crop, image.smoothing);
        composite(pixel, color, alpha * coverage, plan.mode);
    })
}

/** Preserve the public source-over entry point while every painter shares the same compositor. */
pub fn blend(pixel: &mut [u8], color: [u8; 4], alpha: f64) {
    composite(pixel, color, alpha, Composite::SourceOver);
}

/** Once backing is acquired, the loop has no fallible allocation or callback into script. */
fn paint(
    destination: &mut Bitmap,
    plan: &RasterPlan,
    mut operation: impl FnMut(&mut [u8], u32, u32),
) -> EngineResult<()> {
    validate_destination(destination, plan)?;
    if plan.is_empty() {
        return Ok(());
    }
    let width = destination.width as usize;
    let pixels = destination.pixels.make_mut()?;
    let [left, top, right, bottom] = plan.bounds;
    for y in top..bottom {
        for x in left..right {
            let start = (y as usize * width + x as usize) * CHANNELS;
            operation(&mut pixels[start..start + CHANNELS], x, y);
        }
    }
    Ok(())
}

/** Source clipping removes the matching fraction from destination placement. */
fn clipped_image(
    size: [u32; 2],
    crop: [f64; 4],
    placement: [f64; 4],
) -> Option<([f64; 4], [f64; 4])> {
    if crop[2] == 0.0 || crop[3] == 0.0 || placement[2] == 0.0 || placement[3] == 0.0 {
        return None;
    }
    let [x, y, width, height] = crop;
    let left = x.max(0.0);
    let top = y.max(0.0);
    let right = (x + width).min(f64::from(size[0]));
    let bottom = (y + height).min(f64::from(size[1]));
    if right <= left || bottom <= top {
        return None;
    }
    let [dx, dy, dw, dh] = placement;
    Some((
        [left, top, right - left, bottom - top],
        [
            dx + (left - x) / width * dw,
            dy + (top - y) / height * dh,
            (right - left) / width * dw,
            (bottom - top) / height * dh,
        ],
    ))
}

/** Inverse mapping samples the unclipped source positions represented by this placement. */
fn image_position(image: &Image, local: [f64; 2]) -> [f64; 2] {
    let [x, y, width, height] = image.placement;
    let [sx, sy, sw, sh] = image.crop;
    [
        sx + (local[0] - x) / width * sw,
        sy + (local[1] - y) / height * sh,
    ]
}

/** Plans are tied to dimensions so resize during JavaScript conversion cannot reuse stale bounds. */
fn validate_destination(bitmap: &Bitmap, plan: &RasterPlan) -> EngineResult<()> {
    bitmap.validate()?;
    if plan.size != [bitmap.width, bitmap.height] {
        return Err(invalid());
    }
    Ok(())
}

/** Even callers outside V9 must obey bounded raster dimensions. */
fn validate_size(size: [u32; 2]) -> EngineResult<()> {
    if size.iter().any(|edge| *edge > EDGE) {
        return Err(invalid());
    }
    Ok(())
}

/** A forged drawing state must be rejected before the first pixel changes. */
fn validate_alpha(alpha: f64) -> EngineResult<()> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return Err(invalid());
    }
    Ok(())
}

/** Empty plans retain dimensions for stale-plan validation without allocating raster storage. */
fn empty(size: [u32; 2]) -> RasterPlan {
    RasterPlan {
        size,
        polygon: [[0.0; 2]; 4],
        bounds: [0; 4],
        work: 0,
        image: None,
        clip: None,
        mode: Composite::default(),
        inverse: Affine::IDENTITY,
        rectangle: [0.0; 4],
    }
}

/** Invalid native contracts never become silent successful painting. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid canvas raster plan or state.")
}

/** Intersect partial clip pixels with actual rectangle samples instead of multiplying two edge coverages. */
fn coverage(plan: &RasterPlan, x: u32, y: u32) -> f64 {
    let Some(clip) = &plan.clip else {
        return geometry::coverage(&plan.polygon, x, y);
    };
    let bits = clip.bits(x, y);
    if bits == 0 {
        return 0.0;
    }
    if bits == u16::MAX {
        return geometry::coverage(&plan.polygon, x, y);
    }
    let [left, top, width, height] = plan.rectangle;
    let mut count = 0u32;
    for row in 0..GRID {
        for column in 0..GRID {
            if bits & (1 << (row * GRID + column)) == 0 {
                continue;
            }
            let local = plan
                .inverse
                .map([x as f64 + offset(column), y as f64 + offset(row)]);
            count += u32::from(
                local[0] >= left
                    && local[0] < left + width
                    && local[1] >= top
                    && local[1] < top + height,
            );
        }
    }
    f64::from(count) / SAMPLES as f64
}
