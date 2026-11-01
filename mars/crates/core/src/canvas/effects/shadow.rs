/** A shadow is the blurred, offset and colored coverage of a painted layer, composited beneath the shape. */
use super::blur;
use crate::canvas::{Bitmap, CHANNELS};
use crate::protocol::EngineResult;

/** Shadow blur is specified as twice the Gaussian sigma. */
const SIGMA_PER_BLUR: f64 = 0.5;

/** The Gaussian sigma a canvas shadowBlur value selects; zero means a hard shadow. */
pub fn shadow_sigma(blur_px: f64) -> f64 {
    blur_px * SIGMA_PER_BLUR
}

/** Blur the layer's alpha, move it by the offset, and color it; the result is a layer ready for compositing. */
pub fn shadow(
    layer: &Bitmap,
    color: [u8; 4],
    sigma: f64,
    offset: [f64; 2],
) -> EngineResult<Bitmap> {
    layer.validate()?;
    let (width, height) = (layer.width as usize, layer.height as usize);
    let mut alpha: Vec<f32> = layer
        .pixels
        .chunks_exact(CHANNELS)
        .map(|pixel| f32::from(pixel[3]) / f32::from(u8::MAX))
        .collect();
    blur::blur(&mut alpha, width, height, sigma);
    let mut out = Bitmap::new(layer.width, layer.height)?;
    let pixels = out.pixels.make_mut()?;
    for (index, pixel) in pixels.chunks_exact_mut(CHANNELS).enumerate() {
        let x = (index % width) as f64 + 0.5 - offset[0];
        let y = (index / width) as f64 + 0.5 - offset[1];
        let coverage = bilinear(&alpha, width, height, x, y);
        let shade = (f64::from(color[3]) * coverage).round();
        pixel.copy_from_slice(&[color[0], color[1], color[2], shade as u8]);
    }
    Ok(out)
}

/** Bilinear read at a point in pixel coordinates, with transparent samples outside the layer. */
fn bilinear(alpha: &[f32], width: usize, height: usize, x: f64, y: f64) -> f64 {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let texel = |dx: f64, dy: f64| {
        let (px, py) = (x0 + dx, y0 + dy);
        if px < 0.0 || py < 0.0 || px >= width as f64 || py >= height as f64 {
            return 0.0;
        }
        f64::from(alpha[py as usize * width + px as usize])
    };
    let top = texel(0.0, 0.0) * (1.0 - tx) + texel(1.0, 0.0) * tx;
    let bottom = texel(0.0, 1.0) * (1.0 - tx) + texel(1.0, 1.0) * tx;
    (top * (1.0 - ty) + bottom * ty).clamp(0.0, 1.0)
}
