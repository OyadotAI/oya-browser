/** Candidate-owned transition state samples only native targets that remain in the styled document. */
use super::TransitionEasing;
use crate::document::style::constants::TRANSITION_FRAME_MS;
use crate::{
    document::{ComputedStyle, Document},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;

/** Root wrappers and ordinary elements have distinct, noninterchangeable target identities. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionTarget {
    /// The root element is an opacity ancestor of body and native nodes.
    Html,
    /// The body wrapper is not an ordinary projected node.
    Body,
    /// A document-owned stable local element identity.
    Node(usize),
}

/** Page candidates clone this state; discarded candidates cannot advance the committed timeline. */
#[derive(Clone, Debug, Default)]
pub struct OpacityTransitions {
    /// Last unanimated computed values, retained for detecting style changes.
    observed: BTreeMap<TransitionTarget, Observed>,
    /// Only running transitions retain timing and reversal state.
    running: BTreeMap<TransitionTarget, Running>,
    /// A navigation starts a fresh timeline; requests cannot rewind an existing one.
    clock_ms: f64,
}

/** Only before-change facts needed by CSS transition admission are retained. */
#[derive(Clone, Copy, Debug)]
struct Observed {
    opacity: f64,
    hidden: bool,
}

/** One admitted opacity interpolation, independent of subsequent changes to timing declarations. */
#[derive(Clone, Debug)]
struct Running {
    from: f64,
    to: f64,
    start_ms: f64,
    duration_ms: f64,
    easing: TransitionEasing,
    reversing_start: f64,
    shortening: f64,
}

impl OpacityTransitions {
    /** Seed the first style without animation; later style changes admit or interrupt transitions atomically. */
    pub fn update(&mut self, document: &Document, now_ms: f64) -> EngineResult<()> {
        if !now_ms.is_finite()
            || now_ms < self.clock_ms
            || document.nodes.len() > crate::constants::NODES
        {
            return Err(EngineError::invalid("Invalid opacity transition timeline."));
        }
        let mut observed = BTreeMap::new();
        let mut running = BTreeMap::new();
        for (target, style) in targets(document) {
            if !style.transition.valid()
                || !(0.0..=1.0).contains(&style.opacity)
                || observed
                    .insert(
                        target,
                        Observed {
                            opacity: style.opacity,
                            hidden: style.hidden,
                        },
                    )
                    .is_some()
            {
                return Err(EngineError::invalid(
                    "Invalid opacity transition style or target.",
                ));
            }
            let previous = self.observed.get(&target);
            let active = self
                .running
                .get(&target)
                .filter(|animation| now_ms < animation.end());
            if let Some(animation) = decide(previous, active, style, now_ms) {
                running.insert(target, animation);
            }
        }
        self.observed = observed;
        self.running = running;
        self.clock_ms = now_ms;
        Ok(())
    }

    /** Sample a complete opacity overlay; an ended transition yields to the authored destination value. */
    pub fn effects(&self, now_ms: f64) -> Vec<(TransitionTarget, f64)> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .iter()
            .filter(|(_, animation)| now_ms < animation.end())
            .map(|(target, animation)| (*target, animation.value(now_ms)))
            .collect()
    }

    /** Wake at delay completion, the next frame, or the precise end, with no permanently pending entry. */
    pub fn next_sample_ms(&self, now_ms: f64) -> Option<f64> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .values()
            .filter(|animation| now_ms < animation.end())
            .map(|animation| {
                if now_ms < animation.start_ms {
                    animation.start_ms
                } else {
                    (now_ms + TRANSITION_FRAME_MS).min(animation.end())
                }
            })
            .min_by(f64::total_cmp)
    }
}

/** Every distinct native opacity ancestor is observed, including wrappers missing from the projected tree. */
fn targets(document: &Document) -> impl Iterator<Item = (TransitionTarget, &ComputedStyle)> {
    [
        (TransitionTarget::Html, &document.formatting.html.style),
        (TransitionTarget::Body, &document.formatting.body.style),
    ]
    .into_iter()
    .chain(
        document
            .nodes
            .iter()
            .map(|node| (TransitionTarget::Node(node.local_id), &node.style)),
    )
}

/** Matching unchanged transitions continue; newly inserted or display:none elements do not animate. */
fn decide(
    previous: Option<&Observed>,
    active: Option<&Running>,
    style: &ComputedStyle,
    now_ms: f64,
) -> Option<Running> {
    let previous = previous.filter(|previous| !previous.hidden && !style.hidden)?;
    let timing = style.transition.opacity()?;
    if let Some(active) = active
        && active.to == style.opacity
    {
        return Some(active.clone());
    }
    if previous.opacity == style.opacity {
        return None;
    }
    start(previous.opacity, active, style.opacity, timing, now_ms)
}

/** CSS reversing-shortening preserves speed when a running transition returns toward its start. */
fn start(
    before: f64,
    active: Option<&Running>,
    to: f64,
    timing: (f64, f64, TransitionEasing),
    now_ms: f64,
) -> Option<Running> {
    let (mut duration, mut delay, easing) = timing;
    if duration + delay <= 0.0 {
        return None;
    }
    let from = active.map_or(before, |active| active.value(now_ms));
    if from == to {
        return None;
    }
    let (mut shortening, mut reversing_start) = (1.0, from);
    if let Some(active) = active
        && active.reversing_start == to
    {
        shortening = (active.progress(now_ms) * active.shortening + (1.0 - active.shortening))
            .abs()
            .clamp(0.0, 1.0);
        reversing_start = active.to;
        duration *= shortening;
        if delay < 0.0 {
            delay *= shortening;
        }
    }
    let transition = Running {
        from,
        to,
        start_ms: now_ms + delay,
        duration_ms: duration,
        easing,
        reversing_start,
        shortening,
    };
    (transition.end() > now_ms).then_some(transition)
}

impl Running {
    /** Positive delay retains the start value and zero active duration jumps at its end. */
    fn progress(&self, now_ms: f64) -> f64 {
        if now_ms < self.start_ms {
            return 0.0;
        }
        if self.duration_ms == 0.0 {
            return 1.0;
        }
        self.easing
            .sample(((now_ms - self.start_ms) / self.duration_ms).clamp(0.0, 1.0))
    }
    /** Opacity clamps timing overshoot after interpolating the endpoints. */
    fn value(&self, now_ms: f64) -> f64 {
        (self.from + (self.to - self.from) * self.progress(now_ms)).clamp(0.0, 1.0)
    }
    /** Time includes the scaled active duration after any positive or negative delay. */
    fn end(&self) -> f64 {
        self.start_ms + self.duration_ms
    }
}
