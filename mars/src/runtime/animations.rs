/** Sampled animation effects change a native paint candidate without rewriting authored DOM or styles. */
use crate::document::Document;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{AnimationTarget, OpacityEffect};
use std::borrow::Cow;
use std::collections::BTreeSet;

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
    let style = match effect.target {
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
    };
    style.opacity = effect.value;
    Ok(())
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
}
