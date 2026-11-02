/** Pure 2D rotation transitions retain authored angles rather than interpolating flattened matrices. */
use super::{TransitionTarget, interpolation::Interpolation, timeline::targets};
use crate::document::style::constants::TRANSFORM_MAX;
use crate::{
    document::{ComputedStyle, Document},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Unwrapped radians preserve multiple authored turns and direction through interpolation. */
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotation {
    /// Signed radians, bounded by the existing native transform scalar admission.
    pub radians: f64,
}

impl Rotation {
    /** Resolve the angle only at presentation; the existing transform origin remains untouched. */
    pub fn apply(&self, style: &mut ComputedStyle) -> EngineResult<()> {
        if !self.radians.is_finite() || self.radians.abs() > TRANSFORM_MAX {
            return Err(EngineError::invalid("Invalid rotation sample."));
        }
        let (sin, cos) = self.radians.sin_cos();
        style.transform = Some([cos, sin, -sin, cos, 0.0, 0.0]);
        style.translate_percent = [0.0; 2];
        style.rotation_radians = Some(self.radians);
        style.scale_factors = None;
        style.transform_animation_excluded = true;
        Ok(())
    }

    /** Only an authored rotate() or genuinely absent transform can provide an angular endpoint. */
    pub(in crate::document::style) fn of(style: &ComputedStyle) -> Option<Self> {
        let radians = style.rotation_radians.or_else(|| {
            (style.transform.is_none() && !style.transform_animation_excluded).then_some(0.0)
        })?;
        (radians.is_finite() && radians.abs() <= TRANSFORM_MAX).then_some(Self { radians })
    }
}

/** Page candidates own and clone rotation playback independently of the opacity effect stack. */
#[derive(Clone, Debug, Default)]
pub struct RotationTransitions {
    observed: BTreeMap<TransitionTarget, Observed>,
    running: BTreeMap<TransitionTarget, Interpolation<1>>,
    clock_ms: f64,
}

/** Unsupported endpoints remain observed so later supported styles cannot animate across a gap. */
#[derive(Clone, Copy, Debug)]
struct Observed {
    rotation: Option<Rotation>,
    hidden: bool,
}

impl RotationTransitions {
    /** Seed first styling, then admit, continue or cancel matching transform effects atomically. */
    pub fn update(&mut self, document: &Document, now_ms: f64) -> EngineResult<()> {
        if !now_ms.is_finite()
            || now_ms < self.clock_ms
            || document.nodes.len() > crate::constants::NODES
        {
            return Err(EngineError::invalid(
                "Invalid rotation transition timeline.",
            ));
        }
        let mut observed = BTreeMap::new();
        let mut running = BTreeMap::new();
        for (target, style) in targets(document) {
            if !valid(style) || observed.contains_key(&target) {
                return Err(EngineError::invalid(
                    "Invalid rotation transition style or target.",
                ));
            }
            let after = Observed {
                rotation: Rotation::of(style),
                hidden: style.hidden,
            };
            let active = self
                .running
                .get(&target)
                .filter(|effect| now_ms < effect.end());
            if let Some(effect) = decide(self.observed.get(&target), active, &after, style, now_ms)
            {
                running.insert(target, effect);
            }
            observed.insert(target, after);
        }
        self.observed = observed;
        self.running = running;
        self.clock_ms = now_ms;
        Ok(())
    }

    /** A complete paint overlay disappears when its deadline or target lifetime ends. */
    pub fn effects(&self, now_ms: f64) -> Vec<(TransitionTarget, Rotation)> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .iter()
            .filter(|(_, effect)| now_ms < effect.end())
            .map(|(target, effect)| {
                (
                    *target,
                    Rotation {
                        radians: bound(effect.value(now_ms)[0]),
                    },
                )
            })
            .collect()
    }

    /** Share opacity's delay, frame cadence and precise completion scheduling. */
    pub fn next_sample_ms(&self, now_ms: f64) -> Option<f64> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .values()
            .filter_map(|effect| effect.next_sample_ms(now_ms))
            .min_by(f64::total_cmp)
    }
}

/** Unsupported transforms still obey ordinary wire bounds; CSS provenance only narrows admission. */
fn valid(style: &ComputedStyle) -> bool {
    style.transition.valid()
        && style
            .rotation_radians
            .is_none_or(|angle| angle.is_finite() && angle.abs() <= TRANSFORM_MAX)
        && style
            .transform
            .into_iter()
            .flatten()
            .all(|value| value.is_finite() && value.abs() <= TRANSFORM_MAX)
        && style
            .translate_percent
            .iter()
            .all(|value| value.is_finite() && value.abs() <= crate::constants::CSS_MAX_SCALAR)
}

/** Unchanged destinations preserve timing, interrupted effects restart at current presentation. */
fn decide(
    previous: Option<&Observed>,
    active: Option<&Interpolation<1>>,
    after: &Observed,
    style: &ComputedStyle,
    now_ms: f64,
) -> Option<Interpolation<1>> {
    let previous = previous.filter(|before| !before.hidden && !after.hidden)?;
    let (before, to) = ([previous.rotation?.radians], [after.rotation?.radians]);
    let timing = style.transition.matching("transform")?;
    if let Some(active) = active
        && active.to == to
    {
        return Some(active.clone());
    }
    if before == to {
        return None;
    }
    Interpolation::start(before, active, to, timing, now_ms, bound)
}

/** Native transform constraints also apply to easing overshoot and interrupted presentation values. */
fn bound(value: f64) -> f64 {
    value.clamp(-TRANSFORM_MAX, TRANSFORM_MAX)
}
