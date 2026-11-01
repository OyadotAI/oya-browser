/** Shadows, blurs and stroked-text rings: blurred, offset and colored copies of a painted layer. */
mod blur;
mod outline;
mod shadow;
pub use blur::{blur as blur_plane, box_widths as blur_widths};
pub use outline::ring as outline_ring;
pub use shadow::{shadow as shadow_layer, shadow_sigma};
