/** Pure-rotation keyframes preserve signed angles through live playback and native paint sampling. */
mod collect;
mod model;
mod sample;
mod timeline;
pub(in crate::document::style) use collect::tracks;
pub use model::{RotationAnimation, RotationKeyframe};
pub use timeline::RotationAnimations;
