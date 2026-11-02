/** Canvas patterns tile a bitmap in pattern space, which the pattern's own transform places in user space. */
use super::TRANSPARENT;
use crate::canvas::{Affine, Bitmap, CHANNELS};
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};

/** The four repetition keywords; an unknown keyword is refused before a pattern exists. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Repeat {
    /** Tile in both directions. */
    #[default]
    Both,
    /** Tile horizontally only; vertical extent is transparent outside the bitmap. */
    X,
    /** Tile vertically only. */
    Y,
    /** Paint the bitmap once; everything outside it is transparent. */
    Neither,
}

impl Repeat {
    /** Empty and "repeat" both mean tiling in both directions, as in the canvas specification. */
    pub fn named(text: &str) -> Option<Self> {
        Some(match text {
            "" | "repeat" => Self::Both,
            "repeat-x" => Self::X,
            "repeat-y" => Self::Y,
            "no-repeat" => Self::Neither,
            _ => return None,
        })
    }

    /** Whether the horizontal axis wraps. */
    fn wraps_x(self) -> bool {
        matches!(self, Self::Both | Self::X)
    }

    /** Whether the vertical axis wraps. */
    fn wraps_y(self) -> bool {
        matches!(self, Self::Both | Self::Y)
    }
}

/** Pattern source pixels are copied at creation, as Chrome snapshots the image. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    /** Straight-alpha pixels that get tiled. Must be drawable. */
    pub bitmap: Bitmap,
    /** Which axes wrap. */
    pub repeat: Repeat,
    /** Pattern space to user space; setTransform replaces it. */
    pub transform: Affine,
}

impl Pattern {
    /** A new pattern uses the identity transform, so its pixels map one-to-one onto user space. */
    pub fn new(bitmap: Bitmap, repeat: Repeat) -> Self {
        Self {
            bitmap,
            repeat,
            transform: Affine::IDENTITY,
        }
    }

    /** Serialized patterns must have real pixels and a finite transform before they are sampled. */
    pub fn validate(&self) -> EngineResult<()> {
        self.bitmap.validate()?;
        if !self.bitmap.drawable() || !self.transform.finite() {
            return Err(EngineError::invalid("Invalid canvas pattern."));
        }
        Ok(())
    }

    /** Nearest source pixel of a pattern-space point, or transparent where the bitmap does not repeat. */
    pub fn color_at(&self, point: [f64; 2]) -> [u8; 4] {
        let (width, height) = (f64::from(self.bitmap.width), f64::from(self.bitmap.height));
        let x = if self.repeat.wraps_x() {
            point[0].rem_euclid(width)
        } else {
            point[0]
        };
        let y = if self.repeat.wraps_y() {
            point[1].rem_euclid(height)
        } else {
            point[1]
        };
        if !(0.0..width).contains(&x) || !(0.0..height).contains(&y) {
            return TRANSPARENT;
        }
        let index = (y as usize * self.bitmap.width as usize + x as usize) * CHANNELS;
        self.bitmap.pixels[index..index + CHANNELS]
            .try_into()
            .unwrap_or(TRANSPARENT)
    }
}
