/** Bounded opacity transitions keep authored cascade values apart from sampled presentation. */
mod easing;
mod interpolation;
mod model;
mod parse;
mod rotation;
mod scale;
mod timeline;
mod translation;

pub use easing::TransitionEasing;
pub use model::TransitionStyle;
pub(super) use parse::{LONGHANDS, apply, copy, shorthand};
pub(super) use parse::{parts, time};
pub use rotation::{Rotation, RotationTransitions};
pub use scale::{Scale, ScaleTransitions};
pub use timeline::{OpacityTransitions, TransitionTarget};
pub use translation::{Translation, TranslationTransitions};
