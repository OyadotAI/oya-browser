/** Candidate-owned CSS animation playback keeps recascades and pause changes from restarting time. */
use super::model::OpacityAnimation;
use crate::document::style::constants::{ANIMATIONS, TRANSITION_FRAME_MS};
use crate::{
    document::{Document, TransitionTarget},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** A navigation owns one clock-driven set of CSS opacity effects, cloned for transactional updates. */
#[derive(Clone, Debug, Default)]
pub struct OpacityAnimations {
    /// Stable native target identity owns its effect stack and underlying authored opacity.
    targets: BTreeMap<TransitionTarget, Target>,
    /// A committed style update never rewinds the timeline.
    clock_ms: f64,
}

/** Presentation bookkeeping stays outside serialized computed styles. */
#[derive(Clone, Debug)]
struct Target {
    base: f64,
    blocked: bool,
    animations: Vec<Running>,
}

/** Each matched CSS animation preserves local time independently of its current declarations. */
#[derive(Clone, Debug)]
struct Running {
    spec: OpacityAnimation,
    local_ms: f64,
    anchor_ms: f64,
}

impl OpacityAnimations {
    /** First styling starts playback; subsequent cascades match names from the end of each list. */
    pub fn update(&mut self, document: &Document, now_ms: f64) -> EngineResult<()> {
        if !now_ms.is_finite()
            || now_ms < self.clock_ms
            || document.nodes.len() > crate::constants::NODES
        {
            return Err(EngineError::invalid("Invalid opacity animation timeline."));
        }
        let subjects = [
            (TransitionTarget::Html, &document.formatting.html.style),
            (TransitionTarget::Body, &document.formatting.body.style),
        ]
        .into_iter()
        .chain(
            document
                .nodes
                .iter()
                .map(|n| (TransitionTarget::Node(n.local_id), &*n.style)),
        );
        let mut targets = BTreeMap::new();
        for (target, style) in subjects {
            if !(0.0..=1.0).contains(&style.opacity)
                || style.opacity_animations.len() > ANIMATIONS
                || !style.opacity_animations.iter().all(OpacityAnimation::valid)
                || targets.contains_key(&target)
            {
                return Err(EngineError::invalid(
                    "Invalid opacity animation style or target.",
                ));
            }
            let previous = self
                .targets
                .get(&target)
                .map(|target| target.animations.as_slice())
                .unwrap_or_default();
            let animations = if style.hidden {
                Vec::new()
            } else {
                reconcile(previous, &style.opacity_animations, now_ms)
            };
            targets.insert(
                target,
                Target {
                    base: style.opacity,
                    blocked: style.opacity_animation_blocked,
                    animations,
                },
            );
        }
        self.targets = targets;
        self.clock_ms = now_ms;
        Ok(())
    }

    /** Later contributing animations replace earlier ones; fill does not overwrite authored style. */
    pub fn effects(&self, now_ms: f64) -> Vec<(TransitionTarget, f64)> {
        let now_ms = now_ms.max(self.clock_ms);
        self.targets
            .iter()
            .filter(|(_, target)| !target.blocked)
            .filter_map(|(id, target)| {
                let mut result = None;
                for animation in &target.animations {
                    if let Some(value) = animation
                        .spec
                        .value(animation.local(now_ms), result.unwrap_or(target.base))
                    {
                        result = Some(value);
                    }
                }
                result.map(|value| (*id, value))
            })
            .collect()
    }

    /** Paused and completed fills retain their value with no background wakeup. */
    pub fn next_sample_ms(&self, now_ms: f64) -> Option<f64> {
        let now_ms = now_ms.max(self.clock_ms);
        self.targets
            .values()
            .filter(|target| !target.blocked)
            .flat_map(|target| &target.animations)
            .filter_map(|animation| animation.next(now_ms))
            .min_by(f64::total_cmp)
    }
}

/** Right-to-left matching handles duplicate names and list reorder without accidental restarts. */
fn reconcile(previous: &[Running], specs: &[OpacityAnimation], now_ms: f64) -> Vec<Running> {
    let mut available: Vec<Option<&Running>> = previous.iter().map(Some).collect();
    let mut result = Vec::with_capacity(specs.len());
    for spec in specs.iter().rev() {
        let old = available
            .iter_mut()
            .rev()
            .find(|old| old.is_some_and(|old| old.spec.name == spec.name))
            .and_then(Option::take);
        result.push(Running {
            spec: spec.clone(),
            local_ms: old.map_or(0.0, |old| old.local(now_ms)),
            anchor_ms: now_ms,
        });
    }
    result.reverse();
    result
}

impl Running {
    /** Pause/resume changes sample the old play state before installing the new declaration. */
    fn local(&self, now_ms: f64) -> f64 {
        self.local_ms
            + if self.spec.paused {
                0.0
            } else {
                (now_ms - self.anchor_ms).max(0.0)
            }
    }
    /** Waiting effects wake at their start, active effects at a frame or exact finite end. */
    fn next(&self, now_ms: f64) -> Option<f64> {
        if self.spec.paused {
            return None;
        }
        let local = self.local(now_ms);
        let end = self.spec.delay_ms + self.spec.active_duration();
        if local >= end {
            return None;
        }
        let delta = if local < self.spec.delay_ms {
            self.spec.delay_ms - local
        } else {
            TRANSITION_FRAME_MS.min(end - local)
        };
        Some(now_ms + delta)
    }
}
