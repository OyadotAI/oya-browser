/** Premultiplied interpolation avoids color halos around transparent canvas source pixels. */
use super::{Bitmap, CHANNELS};

/** Sampling at pixel centers makes a one-to-one image copy exact. */
pub(super) fn sample(source: &Bitmap, at: [f64; 2], crop: [f64; 4], smooth: bool) -> [u8; 4] {
    if !smooth {
        return pixel(source, at[0].floor(), at[1].floor(), crop);
    }
    let [x, y] = [at[0] - 0.5, at[1] - 0.5];
    let [left, top] = [x.floor(), y.floor()];
    let [horizontal, vertical] = [x - left, y - top];
    let samples = [
        (left, top, (1.0 - horizontal) * (1.0 - vertical)),
        (left + 1.0, top, horizontal * (1.0 - vertical)),
        (left, top + 1.0, (1.0 - horizontal) * vertical),
        (left + 1.0, top + 1.0, horizontal * vertical),
    ];
    interpolate(source, crop, samples)
}

/** Clamp filtering to the selected source rectangle rather than leaking unrelated canvas pixels. */
fn pixel(source: &Bitmap, x: f64, y: f64, crop: [f64; 4]) -> [u8; 4] {
    let [left, top, width, height] = crop;
    let x = x
        .clamp(left.floor(), (left + width).ceil() - 1.0)
        .clamp(0.0, f64::from(source.width) - 1.0) as usize;
    let y = y
        .clamp(top.floor(), (top + height).ceil() - 1.0)
        .clamp(0.0, f64::from(source.height) - 1.0) as usize;
    let start = (y * source.width as usize + x) * CHANNELS;
    source.pixels[start..start + CHANNELS]
        .try_into()
        .expect("validated RGBA pixel")
}

/** Interpolate alpha and premultiplied colors, then convert back to canonical straight RGBA. */
fn interpolate(source: &Bitmap, crop: [f64; 4], samples: [(f64, f64, f64); 4]) -> [u8; 4] {
    let mut sum = [0.0; CHANNELS];
    for (x, y, weight) in samples {
        let color = pixel(source, x, y, crop);
        let alpha = f64::from(color[3]) / 255.0 * weight;
        for channel in 0..3 {
            sum[channel] += f64::from(color[channel]) * alpha;
        }
        sum[3] += alpha;
    }
    if sum[3] == 0.0 {
        return [0; CHANNELS];
    }
    [
        (sum[0] / sum[3]).round() as u8,
        (sum[1] / sum[3]).round() as u8,
        (sum[2] / sum[3]).round() as u8,
        (sum[3] * 255.0).round() as u8,
    ]
}
