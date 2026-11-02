/** Paint sources (solid colors, gradients and patterns) resolve per device pixel through the drawing transform. */
use super::Affine;
use crate::protocol::EngineResult;
use serde::{Deserialize, Serialize};
mod gradient;
mod pattern;
pub use gradient::{Geometry, Gradient, Stop};
pub use pattern::{Pattern, Repeat};

/** Stops are copied into every saved drawing state that uses them, so their count is bounded. */
pub const GRADIENT_STOPS: usize = 256;
/** Paint that covers nothing reads as fully transparent straight-alpha color. */
pub const TRANSPARENT: [u8; 4] = [0; 4];

/** A stored fill or stroke style. Gradients and patterns are mutable objects, so they are cloned at draw time. */
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Paint {
    /** Straight-alpha sRGB from a color assignment. */
    Solid([u8; 4]),
    /** Geometry is declared in the coordinates current when the gradient is painted. */
    Gradient(Gradient),
    /** A bitmap tiled through the pattern's own transform. */
    Pattern(Pattern),
}

impl Paint {
    /** Serialized paint is checked before any raster loop reads its stops or pixels. */
    pub fn validate(&self) -> EngineResult<()> {
        match self {
            Self::Solid(_) => Ok(()),
            Self::Gradient(gradient) => gradient.validate(),
            Self::Pattern(pattern) => pattern.validate(),
        }
    }
}

/** A paint bound to one drawing transform: device pixel centers map back into the paint's own space. */
#[derive(Clone, Debug)]
pub struct Shader {
    paint: Paint,
    inverse: Affine,
}

impl Shader {
    /** Nothing is painted when the transform has no inverse, matching a degenerate drawing transform. */
    pub fn new(paint: Paint, transform: Affine) -> Option<Self> {
        let user = transform.inverse()?;
        let inverse = match &paint {
            Paint::Pattern(pattern) => pattern.transform.inverse()?.compose(user),
            _ => user,
        };
        Some(Self { paint, inverse })
    }

    /** Solid colors ignore geometry, so a plain color can never fail to resolve. */
    pub fn solid(color: [u8; 4]) -> Self {
        Self {
            paint: Paint::Solid(color),
            inverse: Affine::IDENTITY,
        }
    }

    /** Straight-alpha color at a device point, where callers pass pixel centers. */
    pub fn color_at(&self, x: f64, y: f64) -> [u8; 4] {
        match &self.paint {
            Paint::Solid(color) => *color,
            Paint::Gradient(gradient) => gradient.color_at(self.inverse.map([x, y])),
            Paint::Pattern(pattern) => pattern.color_at(self.inverse.map([x, y])),
        }
    }
}
