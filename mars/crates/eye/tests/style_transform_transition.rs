/** Translation transitions sample the actual native transform while retaining authored geometry. */
use mars_eye::{
    Document, TransitionTarget, Translation, TranslationTransitions, compute_styles,
    painted_bounds, parse, render,
};

/** Recompute one stable box so timing tests exercise public CSS parsing and cascade. */
fn styled(transform: &str, transition: &str, extra: &str) -> Document {
    compute_styles(&parse("<div id=target>sample</div>").unwrap(), &[format!("#target{{width:200px;height:100px;transform:{transform};transition:{transition};{extra}}}")]).unwrap()
}

/** Initial style is a seed; the after-change style starts at time zero. */
fn started(from: &str, to: &str, transition: &str) -> (TranslationTransitions, Document) {
    let mut timeline = TranslationTransitions::default();
    timeline.update(&styled(from, transition, ""), 0.0).unwrap();
    let after = styled(to, transition, "");
    timeline.update(&after, 0.0).unwrap();
    (timeline, after)
}

/** Keep assertions independent of wrapper ordering in the public effect snapshot. */
fn sample(timeline: &TranslationTransitions, time: f64) -> Translation {
    timeline
        .effects(time)
        .into_iter()
        .find(|(target, _)| matches!(target, TransitionTarget::Node(_)))
        .unwrap()
        .1
}

#[test]
fn pixel_translation_changes_painted_bounds_without_rewriting_authored_transform() {
    let (timeline, authored) = started("none", "translate(100px,40px)", "transform 1s linear");
    let value = sample(&timeline, 500.0);
    assert_eq!(
        value,
        Translation {
            x_px: 50.0,
            y_px: 20.0,
            ..Translation::default()
        }
    );
    assert_eq!(authored.nodes[0].style.transform.unwrap()[4], 100.0);
    let mut painted = authored.clone();
    value.apply(&mut painted.nodes[0].style).unwrap();
    let display = render(&painted, 800.0).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == painted.nodes[0].local_id)
        .unwrap();
    let bounds = painted_bounds(item);
    assert!((bounds.x - item.x - 50.0).abs() < 0.000001);
    assert!((bounds.y - item.y - 20.0).abs() < 0.000001);
    assert!(timeline.effects(1000.0).is_empty());
    assert_eq!(timeline.next_sample_ms(1000.0), None);
}

#[test]
fn percentages_resolve_against_the_current_box_after_sampling() {
    let (timeline, mut after) = started("none", "translate(100%,50%)", "transform 1s linear");
    let value = sample(&timeline, 500.0);
    assert_eq!(value.x_percent, 50.0);
    assert_eq!(value.y_percent, 25.0);
    value.apply(&mut after.nodes[0].style).unwrap();
    for width in [200.0, 400.0] {
        after.nodes[0].style.width = Some(width);
        let display = render(&after, 800.0).unwrap();
        let item = display
            .items
            .iter()
            .find(|item| item.local_id == after.nodes[0].local_id)
            .unwrap();
        let bounds = painted_bounds(item);
        assert!((bounds.x - item.x - item.width * 0.5).abs() < 0.000001);
        assert!((bounds.y - item.y - item.height * 0.25).abs() < 0.000001);
    }
}

#[test]
fn mixed_pixel_percentage_components_and_translation_lists_interpolate() {
    let (timeline, _) = started(
        "translateX(20px)translateY(10%)",
        "translate(50%,60px)",
        "transform 1s linear",
    );
    assert_eq!(
        sample(&timeline, 500.0),
        Translation {
            x_px: 10.0,
            y_px: 30.0,
            x_percent: 25.0,
            y_percent: 5.0
        }
    );
}

#[test]
fn positive_negative_and_zero_duration_delays_share_opacity_timing() {
    let (delayed, _) = started("none", "translateX(100px)", "transform 1s linear 250ms");
    assert_eq!(sample(&delayed, 100.0).x_px, 0.0);
    assert_eq!(delayed.next_sample_ms(0.0), Some(250.0));
    assert_eq!(sample(&delayed, 750.0).x_px, 50.0);
    let (advanced, _) = started("none", "translateX(100px)", "transform 1s linear -250ms");
    assert_eq!(sample(&advanced, 0.0).x_px, 25.0);
    let (zero, _) = started("none", "translateX(100px)", "transform 0s linear 250ms");
    assert_eq!(sample(&zero, 100.0).x_px, 0.0);
    assert!(zero.effects(250.0).is_empty());
}

#[test]
fn interruption_starts_at_presentation_and_reversal_shortens_duration() {
    let (mut timeline, _) = started("none", "translateX(100px)", "transform 1s linear");
    timeline
        .update(
            &styled("translateX(80px)", "transform 1s linear", ""),
            400.0,
        )
        .unwrap();
    assert_eq!(sample(&timeline, 900.0).x_px, 60.0);
    let (mut reverse, _) = started("none", "translateX(100px)", "transform 1s linear");
    reverse
        .update(&styled("none", "transform 1s linear", ""), 400.0)
        .unwrap();
    assert_eq!(sample(&reverse, 600.0).x_px, 20.0);
    assert!(reverse.effects(800.0).is_empty());
}

#[test]
fn property_lists_and_easing_select_the_last_matching_transform_entry() {
    let (timeline, _) = started(
        "none",
        "translateX(100px)",
        "all 1s linear,transform 2s linear",
    );
    assert_eq!(sample(&timeline, 500.0).x_px, 25.0);
    let (steps, _) = started("none", "translateX(100px)", "transform 1s steps(4,end)");
    assert_eq!(sample(&steps, 400.0).x_px, 25.0);
    let (ease, _) = started("none", "translateX(100px)", "transform 1s ease");
    assert!(sample(&ease, 500.0).x_px > 80.0);
}

#[test]
fn nontranslation_functions_never_gain_support_from_a_flattened_identity_matrix() {
    for unsupported in [
        "rotate(0deg)",
        "rotate(1turn)",
        "scale(1)",
        "skew(0deg)",
        "matrix(1,0,0,1,0,0)",
        "translate3d(10px,0px,0px)",
        "translateZ(0px)",
        "translateX(10px)scale(1)",
    ] {
        let (timeline, after) = started("none", unsupported, "transform 1s linear");
        assert!(
            after.nodes[0].style.transform_animation_excluded,
            "{unsupported}"
        );
        assert!(timeline.effects(500.0).is_empty(), "{unsupported}");
        let (timeline, _) = started(unsupported, "translateX(100px)", "transform 1s linear");
        assert!(timeline.effects(500.0).is_empty(), "{unsupported}");
    }
}

#[test]
fn independent_transform_properties_and_css_wide_resets_preserve_admission() {
    let (mut timeline, _) = started("none", "translateX(100px)", "transform 1s linear");
    timeline
        .update(
            &styled("translateX(100px)", "transform 1s linear", "translate:0px"),
            200.0,
        )
        .unwrap();
    assert!(timeline.effects(300.0).is_empty());
    let reset = styled("rotate(0deg)", "transform 1s linear", "transform:initial");
    assert!(!reset.nodes[0].style.transform_animation_excluded);
    let inherited = compute_styles(
        &parse("<div style='transform:rotate(0deg)'><p style='transform:inherit'>child</p></div>")
            .unwrap(),
        &[],
    )
    .unwrap();
    assert!(
        inherited
            .nodes
            .iter()
            .all(|node| node.style.transform_animation_excluded)
    );
}

#[test]
fn hidden_removed_and_unmatched_targets_cancel_without_restarting_unchanged_values() {
    for extra in ["display:none", "transition:none", "transition:opacity 1s"] {
        let (mut timeline, _) = started("none", "translateX(100px)", "transform 1s linear");
        timeline
            .update(
                &styled("translateX(100px)", "transform 1s linear", extra),
                200.0,
            )
            .unwrap();
        assert!(timeline.effects(300.0).is_empty());
    }
    let (mut timeline, mut after) = started("none", "translateX(100px)", "transform 1s linear");
    after.nodes.clear();
    timeline.update(&after, 200.0).unwrap();
    assert!(timeline.effects(300.0).is_empty());
    timeline
        .update(
            &styled("translateX(100px)", "transform 1s linear", ""),
            400.0,
        )
        .unwrap();
    assert!(timeline.effects(500.0).is_empty());
}

#[test]
fn wrappers_and_malformed_wire_values_keep_distinct_validated_timelines() {
    let source = parse("<p>sample</p>").unwrap();
    let before = compute_styles(
        &source,
        &["html,body{transform:none;transition:transform 1s linear}".into()],
    )
    .unwrap();
    let after = compute_styles(
        &source,
        &["html,body{transform:translateX(100px);transition:transform 1s linear}".into()],
    )
    .unwrap();
    let mut wrappers = TranslationTransitions::default();
    wrappers.update(&before, 0.0).unwrap();
    wrappers.update(&after, 0.0).unwrap();
    assert_eq!(
        wrappers
            .effects(500.0)
            .iter()
            .map(|(target, _)| *target)
            .collect::<Vec<_>>(),
        vec![TransitionTarget::Html, TransitionTarget::Body]
    );
    let (mut timeline, mut bad) = started("none", "translateX(100px)", "transform 1s linear");
    bad.nodes[0].style.translate_percent[0] = f64::NAN;
    assert!(timeline.update(&bad, 200.0).is_err());
    assert_eq!(sample(&timeline, 500.0).x_px, 50.0);
    let invalid = Translation {
        x_px: f64::INFINITY,
        ..Translation::default()
    };
    assert!(invalid.apply(&mut bad.nodes[0].style).is_err());
    assert!(
        timeline
            .update(&styled("none", "transform 1s", ""), -1.0)
            .is_err()
    );
}
