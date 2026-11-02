use super::super::model::{AnimationDirection, AnimationFill};
/** CSS animation phase and iteration calculations feed per-keyframe easing and interpolation. */
use super::model::RotationAnimation;

impl RotationAnimation {
    /** Finite effects finish exactly once; zero-duration infinite effects also settle immediately. */
    pub(super) fn active_duration(&self) -> f64 {
        if self.duration_ms == 0.0 {
            0.0
        } else {
            self.duration_ms * self.iterations.unwrap_or(f64::INFINITY)
        }
    }

    /** Resolve before/active/after phases, including fractional endpoints and alternate direction. */
    pub(super) fn value(&self, local_ms: f64, underlying: f64) -> Option<f64> {
        let active = local_ms - self.delay_ms;
        let total = self.active_duration();
        let before = active < 0.0;
        let after = !before && active >= total;
        if (before && !matches!(self.fill, AnimationFill::Backwards | AnimationFill::Both))
            || (after && !matches!(self.fill, AnimationFill::Forwards | AnimationFill::Both))
        {
            return None;
        }
        let overall = if before {
            0.0
        } else if after {
            self.iterations.unwrap_or(0.0)
        } else {
            active / self.duration_ms
        };
        let mut progress = overall.fract();
        let mut iteration = overall.floor();
        if after && overall > 0.0 && progress == 0.0 {
            progress = 1.0;
            iteration -= 1.0;
        }
        let reverse = match self.direction {
            AnimationDirection::Normal => false,
            AnimationDirection::Reverse => true,
            AnimationDirection::Alternate => iteration % 2.0 == 1.0,
            AnimationDirection::AlternateReverse => iteration % 2.0 == 0.0,
        };
        if reverse {
            progress = 1.0 - progress;
        }
        Some(self.interpolate(progress, underlying, before && !reverse || after && reverse))
    }

    /** Easing belongs to the starting keyframe of each interval; missing endpoints use the lower effect. */
    fn interpolate(&self, progress: f64, underlying: f64, before: bool) -> f64 {
        let index = self
            .frames
            .partition_point(|frame| frame.offset <= progress)
            .saturating_sub(1)
            .min(self.frames.len() - 2);
        let (from, to) = (&self.frames[index], &self.frames[index + 1]);
        let fraction = (progress - from.offset) / (to.offset - from.offset);
        let easing = from.easing.as_ref().unwrap_or(&self.easing);
        let progress = easing.sample_before(fraction.clamp(0.0, 1.0), before);
        let start = from.radians.unwrap_or(underlying);
        (start + (to.radians.unwrap_or(underlying) - start) * progress).clamp(
            -crate::document::style::constants::TRANSFORM_MAX,
            crate::document::style::constants::TRANSFORM_MAX,
        )
    }
}
