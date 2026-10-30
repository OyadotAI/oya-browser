/** Bounded opacity transitions keep authored cascade values apart from sampled presentation. */
mod easing;
mod model;
mod parse;
mod timeline;

pub use easing::TransitionEasing;
pub use model::TransitionStyle;
pub(super) use parse::{LONGHANDS, apply, copy, shorthand};
pub use timeline::{OpacityTransitions, TransitionTarget};
