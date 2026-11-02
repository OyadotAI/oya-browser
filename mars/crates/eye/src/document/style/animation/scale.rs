/** Pure-scale keyframes preserve independent axis factors through live playback and native paint sampling. */
mod collect;
mod model;
mod sample;
mod timeline;
pub(in crate::document::style) use collect::tracks;
pub use model::{ScaleAnimation, ScaleKeyframe};
pub use timeline::ScaleAnimations;
