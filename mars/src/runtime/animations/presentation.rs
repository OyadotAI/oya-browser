/** Native presentation combines opacity, translation, rotation and scale overlays for paint and hit geometry. */
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
    /// Angular animation samples are overridden by matching rotation transitions.
    pub rotations: Vec<(TransitionTarget, crate::document::Rotation)>,
    /// Independent scale axes retain transform-transition priority across supported effect families.
    pub scales: Vec<(TransitionTarget, crate::document::Scale)>,
}

/** Overlay sampled effects on one candidate clone so paint, forced layout and hit bounds agree. */
pub(crate) fn effective_presentation<'a>(
    document: &'a Document,
    effects: &PaintEffects,
) -> EngineResult<Cow<'a, Document>> {
    if effects.translations.len() > super::OPACITY_TARGETS
        || effects.rotations.len() > super::OPACITY_TARGETS
        || effects.scales.len() > super::OPACITY_TARGETS
    {
        return Err(EngineError::limit(
            "Transform paint targets",
            super::OPACITY_TARGETS,
        ));
    }
    let mut effective = super::effective_document(document, &effects.opacity)?;
    let mut seen = BTreeSet::new();
    for (target, value) in &effects.rotations {
        if !seen.insert(*target) {
            return Err(EngineError::invalid("Duplicate rotation paint target."));
        }
        value.apply(super::target_style(
            effective.to_mut(),
            &super::native_target(*target),
        )?)?;
    }
    let mut seen = BTreeSet::new();
    for (target, value) in &effects.scales {
        if !seen.insert(*target) {
            return Err(EngineError::invalid("Duplicate scale paint target."));
        }
        value.apply(super::target_style(
            effective.to_mut(),
            &super::native_target(*target),
        )?)?;
    }
    let mut seen = BTreeSet::new();
    // Transform transitions override animation effects; scale samples already exclude rotation-transition targets.
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

    /** Scaling changes visual bounds/hits while the raw layout and observer measurements remain stable. */
    #[test]
    fn scale_presentation_preserves_layout_sizes_and_resize_measurements() {
        let document = crate::document::compute_styles(
            &crate::document::parse("<div>sample</div>").unwrap(),
            &["div{width:200px;height:100px;transform-origin:0 0}".into()],
        )
        .unwrap();
        let target = TransitionTarget::Node(document.nodes[0].local_id);
        let plain = crate::document::render(&document, 800.0).unwrap();
        let mut effects = PaintEffects {
            scales: vec![(target, crate::document::Scale { x: 2.0, y: -0.5 })],
            ..PaintEffects::default()
        };
        let effective = effective_presentation(&document, &effects).unwrap();
        let display = crate::document::render(&effective, 800.0).unwrap();
        let boxes = crate::runtime::scripting::boxes(&display);
        let geometry = boxes
            .iter()
            .find(|entry| entry.node == document.nodes[0].local_id)
            .unwrap();
        let raw = plain
            .items
            .iter()
            .find(|item| item.local_id == geometry.node)
            .unwrap();
        assert_eq!(geometry.layout_size, Some([200.0, 100.0]));
        assert_eq!(geometry.resize_box, raw.resize_box);
        assert_eq!([geometry.width, geometry.height], [400.0, 50.0]);
        assert!(mars_core::geometry::contains_quad(
            &geometry.quad.unwrap(),
            [geometry.x + 200.0, geometry.y + 25.0]
        ));
        assert!(document.nodes[0].style.transform.is_none());
        effects.scales[0].1.x = f64::NAN;
        assert!(effective_presentation(&document, &effects).is_err());
    }

    /** Cross-family transitions retain transform priority even when their authored destination is none. */
    #[test]
    fn active_rotation_transitions_override_scale_keyframes() {
        let source = crate::document::parse("<div>sample</div>").unwrap();
        let styled = |transform| {
            crate::document::compute_styles(&source, &[format!(
            "@keyframes grow{{from{{transform:scale(1)}}to{{transform:scale(3)}}}}div{{transform:{transform};transition:transform 1s linear;animation:grow 2s linear both}}"
        )]).unwrap()
        };
        let before = styled("rotate(90deg)");
        let after = styled("none");
        let mut scripting = crate::runtime::scripting::Scripting::default();
        scripting
            .css_rotation_transitions
            .update(&before, 0.0)
            .unwrap();
        scripting
            .css_rotation_transitions
            .update(&after, 0.0)
            .unwrap();
        scripting.css_scale_animations.update(&after, 0.0).unwrap();
        scripting.clock_ms = 500.0;
        let effects = super::super::paint_effects(&scripting);
        assert!(effects.scales.is_empty());
        assert!(!effects.rotations.is_empty());
        scripting.clock_ms = 1000.0;
        assert!(!super::super::paint_effects(&scripting).scales.is_empty());
    }

    /** Native geometry consumes the sampled rotation while its layout position stays untransformed. */
    #[test]
    fn rotation_presentation_changes_visual_bounds_and_validates_angular_samples() {
        let document = crate::document::compute_styles(
            &crate::document::parse("<div>sample</div>").unwrap(),
            &["div{width:200px;height:100px;transform-origin:0 0}".into()],
        )
        .unwrap();
        let target = TransitionTarget::Node(document.nodes[0].local_id);
        let mut effects = PaintEffects {
            rotations: vec![(
                target,
                crate::document::Rotation {
                    radians: std::f64::consts::FRAC_PI_2,
                },
            )],
            ..PaintEffects::default()
        };
        let effective = effective_presentation(&document, &effects).unwrap();
        assert!(document.nodes[0].style.transform.is_none());
        let display = crate::document::render(&effective, 800.0).unwrap();
        let item = display
            .items
            .iter()
            .find(|item| item.local_id == document.nodes[0].local_id)
            .unwrap();
        let boxes = crate::runtime::scripting::boxes(&display);
        let geometry = boxes
            .iter()
            .find(|entry| entry.node == item.local_id)
            .unwrap();
        assert!((geometry.width - item.height).abs() < 0.000001);
        assert_eq!(geometry.layout_position, Some([item.x, item.y]));
        effects.rotations[0].1.radians = f64::NAN;
        assert!(effective_presentation(&document, &effects).is_err());
    }

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
            rotations: Vec::new(),
            scales: Vec::new(),
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
