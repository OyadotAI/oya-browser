/** Authored CSS transitions sample native opacity with bounded time, interruption and lifecycle behavior. */
use mars_eye::{Document, OpacityTransitions, TransitionTarget, compute_styles, parse, render};

/** Recompute one stable element through the real parser and cascade. */
fn document(opacity: f64, transition: &str) -> Document {
    compute_styles(
        &parse("<p id=target>sample</p>").unwrap(),
        &[format!("#target{{opacity:{opacity};{transition}}}")],
    )
    .unwrap()
}

/** Sample the target through the public transition surface rather than reading private state. */
fn sample(timeline: &OpacityTransitions, time: f64) -> f64 {
    timeline
        .effects(time)
        .into_iter()
        .find(|(target, _)| matches!(target, TransitionTarget::Node(_)))
        .unwrap()
        .1
}

/** Seed an initial cascade then change opacity at the same style-update time. */
fn started(transition: &str) -> (OpacityTransitions, Document) {
    let mut timeline = OpacityTransitions::default();
    timeline.update(&document(0.0, transition), 0.0).unwrap();
    let after = document(1.0, transition);
    timeline.update(&after, 0.0).unwrap();
    (timeline, after)
}

/** First styling is not a transition, and sampled styles never overwrite the authored target. */
#[test]
fn linear_transition_changes_native_paint_without_changing_authored_opacity() {
    let (timeline, authored) = started("transition:opacity 1s linear");
    assert_eq!(sample(&timeline, 500.0), 0.5);
    assert_eq!(authored.nodes[0].style.opacity, 1.0);
    let mut painted = authored.clone();
    painted.nodes[0].style.opacity = sample(&timeline, 500.0);
    assert!(
        render(&painted, 800.0)
            .unwrap()
            .items
            .iter()
            .any(|item| item.opacity == 0.5)
    );
    assert!(timeline.effects(1000.0).is_empty());
    assert_eq!(timeline.next_sample_ms(1000.0), None);
}

/** Delays hold the before-change style; negative delays start with existing active progress. */
#[test]
fn positive_and_negative_delays_use_the_same_active_timeline() {
    let (delayed, _) = started("transition:opacity 1s linear 250ms");
    assert_eq!(sample(&delayed, 100.0), 0.0);
    assert_eq!(delayed.next_sample_ms(0.0), Some(250.0));
    assert_eq!(sample(&delayed, 750.0), 0.5);
    let (advanced, _) = started("transition:opacity 1s linear -250ms");
    assert_eq!(sample(&advanced, 0.0), 0.25);
    assert!(advanced.effects(750.0).is_empty());
}

/** A delayed zero-duration transition holds then jumps, while a nonpositive combined duration never starts. */
#[test]
fn zero_duration_and_completed_negative_delay_do_not_leave_pending_work() {
    let (delayed, _) = started("transition:opacity 0s linear 250ms");
    assert_eq!(sample(&delayed, 100.0), 0.0);
    assert!(delayed.effects(250.0).is_empty());
    for transition in ["transition:opacity 0s", "transition:opacity 1s -2s"] {
        let (timeline, _) = started(transition);
        assert!(timeline.effects(0.0).is_empty());
        assert_eq!(timeline.next_sample_ms(0.0), None);
    }
}

/** Each timing list repeats by property index, and the final matching property wins. */
#[test]
fn property_lists_choose_the_last_matching_entry_and_repeat_timing_lists() {
    let (timeline, _) = started(
        "transition-property:opacity, width, all;transition-duration:1s,2s;transition-timing-function:linear",
    );
    assert_eq!(sample(&timeline, 500.0), 0.5);
    let (timeline, _) = started("transition:all 1s linear,opacity 2s linear");
    assert_eq!(sample(&timeline, 500.0), 0.25);
}

/** Restarting toward a third value begins at the currently sampled opacity rather than either endpoint. */
#[test]
fn interrupted_transition_starts_from_current_presentation() {
    let (mut timeline, _) = started("transition:opacity 1s linear");
    timeline
        .update(&document(0.8, "transition:opacity 1s linear"), 400.0)
        .unwrap();
    assert!((sample(&timeline, 900.0) - 0.6).abs() < 0.000001);
}

/** Reversal shortens active duration as Ladybird's transition decision algorithm does. */
#[test]
fn reversing_transition_preserves_speed_with_shortened_duration() {
    let (mut timeline, _) = started("transition:opacity 1s linear");
    timeline
        .update(&document(0.0, "transition:opacity 1s linear"), 400.0)
        .unwrap();
    assert!((sample(&timeline, 600.0) - 0.2).abs() < 0.000001);
    assert!(timeline.effects(800.0).is_empty());
}

/** Nonlinear timing functions change real interpolation rather than merely preserving their names. */
#[test]
fn cubic_bezier_and_steps_sample_distinct_progress() {
    let (ease, _) = started("transition:opacity 1s ease");
    assert!(sample(&ease, 500.0) > 0.8 && sample(&ease, 500.0) < 0.81);
    let (steps, _) = started("transition:opacity 1s steps(4, jump-end)");
    assert_eq!(sample(&steps, 400.0), 0.25);
    let (start, _) = started("transition:opacity 1s step-start");
    assert_eq!(sample(&start, 0.0), 1.0);
}

/** Removing the matching property, hiding the element, or detaching it cancels retained effects. */
#[test]
fn lifecycle_changes_cancel_transitions_and_new_nodes_do_not_animate() {
    for sheet in [
        "transition:none",
        "transition:width 1s",
        "transition:opacity 1s;display:none",
    ] {
        let (mut timeline, _) = started("transition:opacity 1s linear");
        timeline.update(&document(1.0, sheet), 200.0).unwrap();
        assert!(timeline.effects(200.0).is_empty());
    }
    let (mut timeline, mut after) = started("transition:opacity 1s linear");
    after.nodes.clear();
    timeline.update(&after, 200.0).unwrap();
    assert!(timeline.effects(200.0).is_empty());
    timeline
        .update(&document(0.5, "transition:opacity 1s linear"), 300.0)
        .unwrap();
    assert!(timeline.effects(300.0).is_empty());
}

/** Wrapper identities stay separate and transition declarations do not inherit into descendants. */
#[test]
fn wrappers_transition_independently_and_children_do_not_inherit_lists() {
    let base = parse("<p id=target>sample</p>").unwrap();
    let before = compute_styles(
        &base,
        &["html,body{opacity:0;transition:opacity 1s linear}".into()],
    )
    .unwrap();
    let after = compute_styles(
        &base,
        &["html,body{opacity:1;transition:opacity 1s linear}".into()],
    )
    .unwrap();
    let mut timeline = OpacityTransitions::default();
    timeline.update(&before, 0.0).unwrap();
    timeline.update(&after, 0.0).unwrap();
    assert_eq!(
        timeline.effects(500.0),
        vec![(TransitionTarget::Html, 0.5), (TransitionTarget::Body, 0.5)]
    );
    assert!(after.nodes[0].style.transition.durations.is_empty());
}

/** Invalid declaration layers are dropped atomically while valid CSS-wide resets survive. */
#[test]
fn invalid_transition_values_are_dropped_without_partial_cascade_changes() {
    for value in [
        "opacity -1s",
        "opacity 1s cubic-bezier(2,0,1,1)",
        "opacity 1s steps(1,jump-none)",
        "none 1s,opacity 1s",
        "opacity 1s allow-discrete",
    ] {
        let styled = document(
            1.0,
            &format!("transition:opacity 2s linear;transition:{value}"),
        );
        assert_eq!(
            styled.nodes[0].style.transition.durations,
            vec![2000.0],
            "{value}"
        );
    }
    let styled = document(1.0, "transition:opacity 2s linear;all:initial");
    assert!(styled.nodes[0].style.transition.durations.is_empty());
}

/** Malformed host records and rewound clocks fail before changing the previous timeline. */
#[test]
fn invalid_wire_styles_and_rewound_clocks_leave_the_timeline_unchanged() {
    let (mut timeline, mut after) = started("transition:opacity 1s linear");
    after.nodes[0].style.transition.durations = vec![f64::NAN];
    assert!(timeline.update(&after, 100.0).is_err());
    assert_eq!(sample(&timeline, 500.0), 0.5);
    assert!(
        timeline
            .update(&document(1.0, "transition:opacity 1s linear"), -1.0)
            .is_err()
    );
}
