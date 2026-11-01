/** Stroked text from glyph coverage: a ring between the dilated and eroded coverage, both over a square window. */
use crate::canvas::{Bitmap, CHANNELS};
use crate::protocol::EngineResult;

/** Coverage is an 8-bit alpha, so the ring keeps its full scale in the output alpha channel. */
const EMPTY: u8 = 0;

/** The band of pixels within `radius` of a glyph edge, as a black layer carrying the ring's coverage. */
pub fn ring(coverage: &Bitmap, radius: usize) -> EngineResult<Bitmap> {
    coverage.validate()?;
    let (width, height) = (coverage.width as usize, coverage.height as usize);
    let alpha: Vec<u8> = coverage
        .pixels
        .chunks_exact(CHANNELS)
        .map(|pixel| pixel[3])
        .collect();
    let grown = separable(&alpha, width, height, radius, u8::max);
    let shrunk = separable(&alpha, width, height, radius, u8::min);
    let mut out = Bitmap::new(coverage.width, coverage.height)?;
    for (pixel, (&high, &low)) in out
        .pixels
        .make_mut()?
        .chunks_exact_mut(CHANNELS)
        .zip(grown.iter().zip(shrunk.iter()))
    {
        pixel.copy_from_slice(&[0, 0, 0, high.saturating_sub(low)]);
    }
    Ok(out)
}

/** A square window is separable, so rows and then columns give the same result as the full neighbourhood. */
fn separable(
    alpha: &[u8],
    width: usize,
    height: usize,
    radius: usize,
    pick: fn(u8, u8) -> u8,
) -> Vec<u8> {
    let mut rows = vec![EMPTY; alpha.len()];
    for (out_row, in_row) in rows
        .chunks_exact_mut(width)
        .zip(alpha.chunks_exact(width))
        .take(height)
    {
        window(in_row, out_row, radius, pick);
    }
    let mut out = vec![EMPTY; alpha.len()];
    let mut column_in = vec![EMPTY; height];
    let mut column_out = vec![EMPTY; height];
    for x in 0..width {
        for (y, cell) in column_in.iter_mut().enumerate() {
            *cell = rows[y * width + x];
        }
        window(&column_in, &mut column_out, radius, pick);
        for (y, cell) in column_out.iter().enumerate() {
            out[y * width + x] = *cell;
        }
    }
    out
}

/** Each output is the pick over a window clipped to the line; positions outside the line are empty. */
fn window(line: &[u8], out: &mut [u8], radius: usize, pick: fn(u8, u8) -> u8) {
    for (index, cell) in out.iter_mut().enumerate() {
        let start = index.saturating_sub(radius);
        let end = index
            .saturating_add(radius)
            .saturating_add(1)
            .min(line.len());
        let mut value = if index < radius || index.saturating_add(radius) >= line.len() {
            EMPTY
        } else {
            line[index]
        };
        for &sample in &line[start..end] {
            value = pick(value, sample);
        }
        *cell = value;
    }
}
