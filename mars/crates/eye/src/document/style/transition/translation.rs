/** Translation-only transform transitions preserve pixel and box-relative offsets until native layout. */
use super::{TransitionTarget, interpolation::Interpolation, timeline::targets};
use crate::document::style::{constants::TRANSFORM_MAX, paint::IDENTITY};
use crate::{
    document::{ComputedStyle, Document},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** A sampled 2D translation retains percentages so resize and hit geometry use the current box. */
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Translation {
    /// Horizontal offset in logical pixels.
    pub x_px: f64,
    /// Vertical offset in logical pixels.
    pub y_px: f64,
    /// Horizontal offset as a percentage of the border-box width.
    pub x_percent: f64,
    /// Vertical offset as a percentage of the border-box height.
    pub y_percent: f64,
}

impl Translation {
    /** Overlay an admitted sample without altering the authored document retained by the caller. */
    pub fn apply(&self, style: &mut ComputedStyle) -> EngineResult<()> {
        if !self
            .components()
            .iter()
            .all(|value| value.is_finite() && value.abs() <= TRANSFORM_MAX)
        {
            return Err(EngineError::invalid(
                "Invalid translation transition sample.",
            ));
        }
        style.transform = Some([1.0, 0.0, 0.0, 1.0, self.x_px, self.y_px]);
        style.translate_percent = [self.x_percent, self.y_percent];
        Ok(())
    }

    /** Matrix shape and CSS provenance both exclude rotations, scales, 3D, and independent properties. */
    fn of(style: &ComputedStyle) -> Option<Self> {
        let matrix = style.transform.unwrap_or(IDENTITY);
        if style.transform_animation_excluded
            || matrix[..4] != IDENTITY[..4]
            || style
                .translate_percent
                .iter()
                .any(|value| value.abs() > TRANSFORM_MAX)
        {
            return None;
        }
        Some(Self {
            x_px: matrix[4],
            y_px: matrix[5],
            x_percent: style.translate_percent[0],
            y_percent: style.translate_percent[1],
        })
    }

    /** Component order is private to interpolation and cannot be confused with affine matrix entries. */
    fn components(self) -> [f64; 4] {
        [self.x_px, self.y_px, self.x_percent, self.y_percent]
    }

    /** Easing overshoot remains within the existing native transform resource bound. */
    fn sampled(values: [f64; 4]) -> Self {
        let [x_px, y_px, x_percent, y_percent] = values.map(bound);
        Self {
            x_px,
            y_px,
            x_percent,
            y_percent,
        }
    }
}

/** Page candidates own and clone translation playback independently of the opacity effect stack. */
#[derive(Clone, Debug, Default)]
pub struct TranslationTransitions {
    observed: BTreeMap<TransitionTarget, Observed>,
    running: BTreeMap<TransitionTarget, Interpolation<4>>,
    clock_ms: f64,
}

/** Unsupported endpoints remain observed so later supported styles cannot animate across a gap. */
#[derive(Clone, Copy, Debug)]
struct Observed {
    translation: Option<Translation>,
    hidden: bool,
}

impl TranslationTransitions {
    /** Seed first styling, then admit, continue or cancel matching transform effects atomically. */
    pub fn update(&mut self, document: &Document, now_ms: f64) -> EngineResult<()> {
        if !now_ms.is_finite()
            || now_ms < self.clock_ms
            || document.nodes.len() > crate::constants::NODES
        {
            return Err(EngineError::invalid(
                "Invalid translation transition timeline.",
            ));
        }
        let mut observed = BTreeMap::new();
        let mut running = BTreeMap::new();
        for (target, style) in targets(document) {
            if !valid(style) || observed.contains_key(&target) {
                return Err(EngineError::invalid(
                    "Invalid translation transition style or target.",
                ));
            }
            let after = Observed {
                translation: Translation::of(style),
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
    pub fn effects(&self, now_ms: f64) -> Vec<(TransitionTarget, Translation)> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .iter()
            .filter(|(_, effect)| now_ms < effect.end())
            .map(|(target, effect)| (*target, Translation::sampled(effect.value(now_ms))))
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
    active: Option<&Interpolation<4>>,
    after: &Observed,
    style: &ComputedStyle,
    now_ms: f64,
) -> Option<Interpolation<4>> {
    let previous = previous.filter(|before| !before.hidden && !after.hidden)?;
    let (before, to) = (
        previous.translation?.components(),
        after.translation?.components(),
    );
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
