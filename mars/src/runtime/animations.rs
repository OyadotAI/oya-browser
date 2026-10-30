/** Sampled animation effects change a native paint candidate without rewriting authored DOM or styles. */
use crate::document::Document;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{AnimationTarget, OpacityEffect};
use std::borrow::Cow;
use std::collections::BTreeSet;
mod presentation;
pub(super) use presentation::{PaintEffects, effective_presentation};

/** Each cascade updates candidate CSS timelines without altering authored styles. */
pub(super) fn refresh_timelines(page: &mut super::page::Page) -> EngineResult<()> {
    page.scripting
        .css_animations
        .update(&page.document, page.scripting.clock_ms)?;
    page.scripting
        .css_transitions
        .update(&page.document, page.scripting.clock_ms)?;
    page.scripting
        .css_translation_transitions
        .update(&page.document, page.scripting.clock_ms)
}

/** CSS transitions override other sampled animation opacity at the native paint boundary. */
pub(super) fn paint_effects(scripting: &super::scripting::Scripting) -> PaintEffects {
    PaintEffects {
        opacity: combine_effects(
            &scripting.opacity_effects,
            &scripting.css_transitions,
            &scripting.css_animations,
            scripting.clock_ms,
        ),
        translations: scripting
            .css_translation_transitions
            .effects(scripting.clock_ms),
    }
}

/** One effect per target retains strict native validation when animation sources overlap. */
pub(super) fn combine_effects(
    effects: &[OpacityEffect],
    transitions: &crate::document::OpacityTransitions,
    animations: &crate::document::OpacityAnimations,
    now_ms: f64,
) -> Vec<OpacityEffect> {
    let mut merged = animations
        .effects(now_ms)
        .into_iter()
        .map(|(target, value)| (native_target(target), value))
        .collect::<std::collections::BTreeMap<_, _>>();
    merged.extend(
        effects
            .iter()
            .map(|effect| (effect.target.clone(), effect.value)),
    );
    for (target, value) in transitions.effects(now_ms) {
        merged.insert(native_target(target), value);
    }
    merged
        .into_iter()
        .map(|(target, value)| OpacityEffect { target, value })
        .collect()
}

/** Eye and V9 preserve the same wrapper/node distinction without depending on each other. */
fn native_target(target: crate::document::TransitionTarget) -> AnimationTarget {
    match target {
        crate::document::TransitionTarget::Html => AnimationTarget::Html,
        crate::document::TransitionTarget::Body => AnimationTarget::Body,
        crate::document::TransitionTarget::Node(id) => AnimationTarget::Node(id),
    }
}

/** Either CSS timeline can keep native paint advancing without extending script execution. */
pub(super) fn next_sample(scripting: &super::scripting::Scripting) -> Option<f64> {
    [
        scripting.css_transitions.next_sample_ms(scripting.clock_ms),
        scripting.css_animations.next_sample_ms(scripting.clock_ms),
        scripting
            .css_translation_transitions
            .next_sample_ms(scripting.clock_ms),
    ]
    .into_iter()
    .flatten()
    .min_by(f64::total_cmp)
}

/** The real tree and its two document wrappers bound all distinct paint targets. */
const OPACITY_TARGETS: usize = crate::constants::NODES + DOCUMENT_WRAPPERS;
/** Root and body styles are real paint ancestors without ordinary node identities. */
const DOCUMENT_WRAPPERS: usize = 2;

/** A complete effect snapshot overlays only effective styles; an empty snapshot restores the cascade. */
pub(super) fn effective_document<'a>(
    document: &'a Document,
    effects: &[OpacityEffect],
) -> EngineResult<Cow<'a, Document>> {
    validate_effects(effects)?;
    if effects.is_empty() {
        return Ok(Cow::Borrowed(document));
    }
    let mut effective = document.clone();
    for effect in effects {
        apply_opacity(&mut effective, effect)?;
    }
    Ok(Cow::Owned(effective))
}

/** Hostile transport cannot supply duplicate targets, non-finite alpha or an unbounded effect list. */
fn validate_effects(effects: &[OpacityEffect]) -> EngineResult<()> {
    if effects.len() > OPACITY_TARGETS {
        return Err(EngineError::limit(
            "Animation paint targets",
            OPACITY_TARGETS,
        ));
    }
    let mut seen = BTreeSet::new();
    if effects.iter().any(|effect| {
        !effect.value.is_finite()
            || !(0.0..=1.0).contains(&effect.value)
            || !seen.insert(effect.target.clone())
    }) {
        return Err(EngineError::invalid("Invalid animation paint effects."));
    }
    Ok(())
}

/** Eye already composites native style opacity, including document wrappers and nested groups. */
fn apply_opacity(document: &mut Document, effect: &OpacityEffect) -> EngineResult<()> {
    target_style(document, &effect.target)?.opacity = effect.value;
    Ok(())
}

/** Both native effect families select exactly one existing style through the same identity guard. */
fn target_style<'a>(
    document: &'a mut Document,
    target: &AnimationTarget,
) -> EngineResult<&'a mut crate::document::ComputedStyle> {
    Ok(match *target {
        AnimationTarget::Html => &mut document.formatting.html.style,
        AnimationTarget::Body => &mut document.formatting.body.style,
        AnimationTarget::Node(id) => {
            &mut document
                .nodes
                .iter_mut()
                .find(|node| node.local_id == id)
                .ok_or_else(|| EngineError::invalid("Animation targets an absent native node."))?
                .style
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Animation cancellation removes the effective overlay while the authored cascade remains intact. */
    #[test]
    fn effective_opacity_never_rewrites_the_authored_document() {
        let mut document = crate::document::parse("<p id=target>text</p>").unwrap();
        let node = &mut document.nodes[0];
        node.style.opacity = 0.8;
        let effect = OpacityEffect {
            target: AnimationTarget::Node(node.local_id),
            value: 0.25,
        };
        let effective = effective_document(&document, &[effect]).unwrap();
        assert_eq!(effective.nodes[0].style.opacity, 0.25);
        assert_eq!(document.nodes[0].style.opacity, 0.8);
        assert_eq!(
            effective_document(&document, &[]).unwrap().nodes[0]
                .style
                .opacity,
            0.8
        );
    }

    /** Wrapper effects enter the same native ancestor styles as ordinary CSS opacity. */
    #[test]
    fn document_wrappers_receive_effective_opacity() {
        let document = crate::document::parse("<p>text</p>").unwrap();
        let effects = [
            OpacityEffect {
                target: AnimationTarget::Html,
                value: 0.5,
            },
            OpacityEffect {
                target: AnimationTarget::Body,
                value: 0.25,
            },
        ];
        let effective = effective_document(&document, &effects).unwrap();
        assert_eq!(effective.formatting.html.style.opacity, 0.5);
        assert_eq!(effective.formatting.body.style.opacity, 0.25);
        assert_eq!(document.formatting.body.style.opacity, 1.0);
    }

    /** Invalid host output cannot silently paint an arbitrary target or non-finite alpha. */
    #[test]
    fn hostile_effect_snapshots_are_rejected() {
        let document = crate::document::parse("<p>text</p>").unwrap();
        for effects in [
            vec![OpacityEffect {
                target: AnimationTarget::Body,
                value: f64::NAN,
            }],
            vec![OpacityEffect {
                target: AnimationTarget::Body,
                value: 1.5,
            }],
            vec![OpacityEffect {
                target: AnimationTarget::Node(usize::MAX),
                value: 0.5,
            }],
            vec![
                OpacityEffect {
                    target: AnimationTarget::Body,
                    value: 0.2,
                },
                OpacityEffect {
                    target: AnimationTarget::Body,
                    value: 0.3,
                },
            ],
        ] {
            assert!(effective_document(&document, &effects).is_err());
        }
    }

    /** Native painting samples CSS transitions while retaining the authored destination value. */
    #[test]
    fn css_transition_samples_override_other_effects_without_mutating_cascade() {
        let source = crate::document::parse("<p id=target>text</p>").unwrap();
        let styled = |opacity| {
            crate::document::compute_styles(
                &source,
                &[format!(
                    "p{{opacity:{opacity};transition:opacity 1s linear}}"
                )],
            )
            .unwrap()
        };
        let before = styled(0);
        let after = styled(1);
        let target = after
            .nodes
            .iter()
            .find(|node| node.html_id == "target")
            .unwrap()
            .local_id;
        let mut scripting = super::super::scripting::Scripting::default();
        scripting.css_transitions.update(&before, 0.0).unwrap();
        scripting.css_transitions.update(&after, 1000.0).unwrap();
        scripting.clock_ms = 1500.0;
        scripting.opacity_effects = vec![OpacityEffect {
            target: AnimationTarget::Node(target),
            value: 0.2,
        }];
        let effects = paint_effects(&scripting);
        let effective = effective_presentation(&after, &effects).unwrap();
        assert!(
            (effective
                .nodes
                .iter()
                .find(|node| node.local_id == target)
                .unwrap()
                .style
                .opacity
                - 0.5)
                .abs()
                < 0.001
        );
        assert_eq!(
            after
                .nodes
                .iter()
                .find(|node| node.local_id == target)
                .unwrap()
                .style
                .opacity,
            1.0
        );
        assert!(scripting.pending());
        scripting.clock_ms = 2000.0;
        assert_eq!(paint_effects(&scripting).opacity, scripting.opacity_effects);
        assert!(!scripting.pending());
    }

    /** Initial CSS animations drive paint without JavaScript and keep completed fill without ticks. */
    #[test]
    fn css_keyframes_keep_native_paint_active_and_preserve_authored_opacity() {
        let source = crate::document::parse("<p id=target>text</p>").unwrap();
        let document = crate::document::compute_styles(
            &source,
            &["@keyframes fade{from{opacity:0}to{opacity:1}}p{opacity:.8;animation:fade 1s linear forwards}".into()],
        ).unwrap();
        let target = document
            .nodes
            .iter()
            .find(|node| node.html_id == "target")
            .unwrap();
        let mut scripting = super::super::scripting::Scripting::default();
        scripting.css_animations.update(&document, 0.0).unwrap();
        scripting.clock_ms = 500.0;
        assert!(scripting.pending());
        let effects = paint_effects(&scripting);
        let opacity = effects
            .opacity
            .iter()
            .find(|effect| effect.target == AnimationTarget::Node(target.local_id))
            .unwrap()
            .value;
        assert!((opacity - 0.5).abs() < 0.001);
        assert_eq!(target.style.opacity, 0.8);
        scripting.opacity_effects = vec![OpacityEffect {
            target: AnimationTarget::Node(target.local_id),
            value: 0.25,
        }];
        assert_eq!(paint_effects(&scripting).opacity, scripting.opacity_effects);
        scripting.opacity_effects.clear();
        scripting.clock_ms = 1000.0;
        assert!(!scripting.pending());
        assert_eq!(
            paint_effects(&scripting)
                .opacity
                .iter()
                .find(|effect| effect.target == AnimationTarget::Node(target.local_id))
                .unwrap()
                .value,
            1.0
        );
    }
}
