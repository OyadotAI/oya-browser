/** Pure 2D scale transitions interpolate authored axis factors, preserving transform provenance. */
use super::{TransitionTarget, interpolation::Interpolation, timeline::targets};
use crate::document::style::constants::TRANSFORM_MAX;
use crate::{
    document::{ComputedStyle, Document},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Independent axis factors retain authored scale/scaleX/scaleY values before native matrix construction. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale {
    /// Horizontal factor; negative values reflect and zero is a singular visual transform.
    pub x: f64,
    /// Vertical factor, independently interpolated under the same CSS timing progress.
    pub y: f64,
}

impl Default for Scale {
    /** The identity endpoint for an absent transform scales both axes by one. */
    fn default() -> Self {
        Self { x: 1.0, y: 1.0 }
    }
}

impl Scale {
    /** Presentation changes only the transform matrix; layout measurements and origin remain authored. */
    pub fn apply(&self, style: &mut ComputedStyle) -> EngineResult<()> {
        if [self.x, self.y]
            .iter()
            .any(|factor| !factor.is_finite() || factor.abs() > TRANSFORM_MAX)
        {
            return Err(EngineError::invalid("Invalid scale sample."));
        }
        style.transform = Some([self.x, 0.0, 0.0, self.y, 0.0, 0.0]);
        style.translate_percent = [0.0; 2];
        style.scale_factors = Some([self.x, self.y]);
        style.rotation_radians = None;
        style.transform_animation_excluded = true;
        Ok(())
    }

    /** Only single authored 2D scale functions and genuinely absent transforms supply endpoints. */
    pub(in crate::document::style) fn of(style: &ComputedStyle) -> Option<Self> {
        let [x, y] = style.scale_factors.or_else(|| {
            (style.transform.is_none() && !style.transform_animation_excluded).then_some([1.0; 2])
        })?;
        let matrix = [x, 0.0, 0.0, y, 0.0, 0.0];
        ([x, y]
            .iter()
            .all(|factor| factor.is_finite() && factor.abs() <= TRANSFORM_MAX)
            && style.transform.is_none_or(|value| value == matrix)
            && style.translate_percent == [0.0; 2])
            .then_some(Self { x, y })
    }
}

/** Page candidates own and clone scale playback independently of the opacity effect stack. */
#[derive(Clone, Debug, Default)]
pub struct ScaleTransitions {
    observed: BTreeMap<TransitionTarget, Observed>,
    running: BTreeMap<TransitionTarget, Interpolation<2>>,
    clock_ms: f64,
}

/** Unsupported endpoints remain observed so later supported styles cannot animate across a gap. */
#[derive(Clone, Copy, Debug)]
struct Observed {
    scale: Option<Scale>,
    hidden: bool,
}

impl ScaleTransitions {
    /** Seed first styling, then admit, continue or cancel matching transform effects atomically. */
    pub fn update(&mut self, document: &Document, now_ms: f64) -> EngineResult<()> {
        if !now_ms.is_finite()
            || now_ms < self.clock_ms
            || document.nodes.len() > crate::constants::NODES
        {
            return Err(EngineError::invalid("Invalid scale transition timeline."));
        }
        let mut observed = BTreeMap::new();
        let mut running = BTreeMap::new();
        for (target, style) in targets(document) {
            if !valid(style) || observed.contains_key(&target) {
                return Err(EngineError::invalid(
                    "Invalid scale transition style or target.",
                ));
            }
            let after = Observed {
                scale: Scale::of(style),
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
    pub fn effects(&self, now_ms: f64) -> Vec<(TransitionTarget, Scale)> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .iter()
            .filter(|(_, effect)| now_ms < effect.end())
            .map(|(target, effect)| {
                (
                    *target,
                    Scale {
                        x: bound(effect.value(now_ms)[0]),
                        y: bound(effect.value(now_ms)[1]),
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
        && style.scale_factors.is_none_or(|factors| {
            factors
                .iter()
                .all(|factor| factor.is_finite() && factor.abs() <= TRANSFORM_MAX)
        })
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
    active: Option<&Interpolation<2>>,
    after: &Observed,
    style: &ComputedStyle,
    now_ms: f64,
) -> Option<Interpolation<2>> {
    let previous = previous.filter(|before| !before.hidden && !after.hidden)?;
    let (before, to) = (previous.scale?, after.scale?);
    let (before, to) = ([before.x, before.y], [to.x, to.y]);
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
