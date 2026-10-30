/** Native presentation combines independent opacity and translation overlays for paint and hit geometry. */
use crate::{
    document::{Document, TransitionTarget, Translation},
    protocol::{EngineError, EngineResult},
    v9::OpacityEffect,
};
use std::{borrow::Cow, collections::BTreeSet};

/** The complete sampled frame changes when either a visual alpha or a geometric transform advances. */
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PaintEffects {
    /// CSS animation, V9 animation, and CSS transition opacity in their existing priority order.
    pub opacity: Vec<OpacityEffect>,
    /// Bounded native translation transitions use the current layout box for percentages.
    pub translations: Vec<(TransitionTarget, Translation)>,
}

/** Overlay both families on one candidate clone so paint, forced layout and hit bounds agree. */
pub(crate) fn effective_presentation<'a>(
    document: &'a Document,
    effects: &PaintEffects,
) -> EngineResult<Cow<'a, Document>> {
    if effects.translations.len() > super::OPACITY_TARGETS {
        return Err(EngineError::limit(
            "Translation paint targets",
            super::OPACITY_TARGETS,
        ));
    }
    let mut seen = BTreeSet::new();
    let mut effective = super::effective_document(document, &effects.opacity)?;
    for (target, value) in &effects.translations {
        if !seen.insert(*target) {
            return Err(EngineError::invalid("Duplicate translation paint target."));
        }
        value.apply(super::target_style(
            effective.to_mut(),
            &super::native_target(*target),
        )?)?;
    }
    Ok(effective)
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Native frames, DOM visual geometry and untransformed layout positions share one sampled candidate. */
    #[test]
    fn translation_paint_and_dom_geometry_advance_without_rewriting_layout_positions() {
        let source = crate::document::parse("<div id=target>sample</div>").unwrap();
        let styled = |transform| {
            crate::document::compute_styles(&source, &[format!("div{{width:200px;height:100px;transform:{transform};transition:transform 1s linear}}")]).unwrap()
        };
        let before = styled("none");
        let after = styled("translate(100px,40px)");
        let mut scripting = crate::runtime::scripting::Scripting::default();
        scripting
            .css_translation_transitions
            .update(&before, 0.0)
            .unwrap();
        scripting
            .css_translation_transitions
            .update(&after, 0.0)
            .unwrap();
        scripting.clock_ms = 500.0;
        let effects = super::super::paint_effects(&scripting);
        assert!(scripting.pending());
        let effective = effective_presentation(&after, &effects).unwrap();
        assert_eq!(effective.nodes[0].style.transform.unwrap()[4], 50.0);
        assert_eq!(after.nodes[0].style.transform.unwrap()[4], 100.0);
        let display = crate::document::render(&effective, 800.0).unwrap();
        let boxes = crate::runtime::scripting::boxes(&display);
        let item = display
            .items
            .iter()
            .find(|item| item.local_id == after.nodes[0].local_id)
            .unwrap();
        let geometry = boxes
            .iter()
            .find(|entry| entry.node == item.local_id)
            .unwrap();
        assert!((geometry.x - item.x - 50.0).abs() < 0.000001);
        assert!((geometry.y - item.y - 20.0).abs() < 0.000001);
        assert_eq!(geometry.layout_position, Some([item.x, item.y]));
        scripting.clock_ms = 1000.0;
        assert!(!scripting.pending());
        assert!(
            super::super::paint_effects(&scripting)
                .translations
                .is_empty()
        );
    }

    /** Both effect families coexist; an absent or duplicate geometric target cannot bypass validation. */
    #[test]
    fn translation_overlays_validate_targets_and_preserve_opacity_effects() {
        let document = crate::document::parse("<p>sample</p>").unwrap();
        let target = TransitionTarget::Node(document.nodes[0].local_id);
        let translation = Translation {
            x_px: 25.0,
            ..Translation::default()
        };
        let mut effects = PaintEffects {
            opacity: vec![OpacityEffect {
                target: crate::v9::AnimationTarget::Node(document.nodes[0].local_id),
                value: 0.5,
            }],
            translations: vec![(target, translation)],
        };
        let effective = effective_presentation(&document, &effects).unwrap();
        assert_eq!(effective.nodes[0].style.opacity, 0.5);
        assert_eq!(effective.nodes[0].style.transform.unwrap()[4], 25.0);
        effects.translations.push((target, translation));
        assert!(effective_presentation(&document, &effects).is_err());
        effects.translations = vec![(TransitionTarget::Node(usize::MAX), translation)];
        assert!(effective_presentation(&document, &effects).is_err());
        effects.translations = vec![(
            target,
            Translation {
                y_px: f64::NAN,
                ..translation
            },
        )];
        assert!(effective_presentation(&document, &effects).is_err());
    }
}
