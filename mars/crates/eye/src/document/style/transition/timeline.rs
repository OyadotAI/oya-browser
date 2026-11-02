/** Candidate-owned transition state samples only native targets that remain in the styled document. */
use super::interpolation::Interpolation;
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

/** Opacity is the single-component instance of the shared bounded transition interpolator. */
type Running = Interpolation<1>;

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
            .map(|(target, animation)| (*target, animation.value(now_ms)[0].clamp(0.0, 1.0)))
            .collect()
    }

    /** Wake at delay completion, the next frame, or the precise end, with no permanently pending entry. */
    pub fn next_sample_ms(&self, now_ms: f64) -> Option<f64> {
        let now_ms = now_ms.max(self.clock_ms);
        self.running
            .values()
            .filter_map(|animation| animation.next_sample_ms(now_ms))
            .min_by(f64::total_cmp)
    }
}

/** Every distinct native opacity ancestor is observed, including wrappers missing from the projected tree. */
pub(super) fn targets(
    document: &Document,
) -> impl Iterator<Item = (TransitionTarget, &ComputedStyle)> {
    [
        (TransitionTarget::Html, &document.formatting.html.style),
        (TransitionTarget::Body, &document.formatting.body.style),
    ]
    .into_iter()
    .chain(
        document
            .nodes
            .iter()
            .map(|node| (TransitionTarget::Node(node.local_id), &*node.style)),
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
    let timing = style.transition.matching("opacity")?;
    if let Some(active) = active
        && active.to == [style.opacity]
    {
        return Some(active.clone());
    }
    if previous.opacity == style.opacity {
        return None;
    }
    Running::start(
        [previous.opacity],
        active,
        [style.opacity],
        timing,
        now_ms,
        |value| value.clamp(0.0, 1.0),
    )
}
