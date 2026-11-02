/** Real CSS opacity keyframes advance on a deterministic clock without modifying authored cascade. */
use mars_eye::{Document, OpacityAnimations, TransitionTarget, compute_styles, parse, render};

const FADE: &str = "@keyframes fade{from{opacity:0}to{opacity:1}}";

/** Stable projected identity allows lifecycle assertions across recascades. */
fn styled(animation: &str, extra: &str) -> Document {
    compute_styles(
        &parse("<p id=target>sample</p>").unwrap(),
        &[format!(
            "{FADE} #target{{opacity:.4;animation:{animation};{extra}}}"
        )],
    )
    .unwrap()
}

/** Start the real parsed animation at navigation time. */
fn started(animation: &str) -> OpacityAnimations {
    let mut timeline = OpacityAnimations::default();
    timeline.update(&styled(animation, ""), 0.0).unwrap();
    timeline
}

/** Compare only the ordinary element target; wrappers have independent effects. */
fn sample(timeline: &OpacityAnimations, time: f64) -> Option<f64> {
    timeline
        .effects(time)
        .into_iter()
        .find(|(target, _)| matches!(target, TransitionTarget::Node(_)))
        .map(|(_, value)| value)
}

#[test]
fn linear_keyframes_sample_native_paint_while_authored_opacity_remains_unchanged() {
    let authored = styled("fade 1s linear both", "");
    let mut timeline = OpacityAnimations::default();
    timeline.update(&authored, 0.0).unwrap();
    assert_eq!(sample(&timeline, 500.0), Some(0.5));
    assert_eq!(authored.nodes[0].style.opacity, 0.4);
    let mut painted = authored.clone();
    painted.nodes[0].style.opacity = sample(&timeline, 500.0).unwrap();
    assert!(
        render(&painted, 800.0)
            .unwrap()
            .items
            .iter()
            .any(|item| item.opacity == 0.5)
    );
    assert_eq!(sample(&timeline, 1000.0), Some(1.0));
    assert_eq!(timeline.next_sample_ms(1000.0), None);
}

#[test]
fn delay_fill_and_negative_delay_choose_the_correct_phase() {
    let none = started("fade 1s linear 250ms");
    assert_eq!(sample(&none, 100.0), None);
    assert_eq!(none.next_sample_ms(0.0), Some(250.0));
    assert_eq!(sample(&none, 750.0), Some(0.5));
    assert_eq!(sample(&none, 1250.0), None);
    let both = started("fade 1s linear 250ms both");
    assert_eq!(sample(&both, 100.0), Some(0.0));
    assert_eq!(sample(&both, 1250.0), Some(1.0));
    assert_eq!(
        sample(&started("fade 1s linear -250ms both"), 0.0),
        Some(0.25)
    );
}

#[test]
fn fractional_iterations_direction_and_infinite_loops_preserve_progress() {
    for (direction, end) in [
        ("normal", 0.5),
        ("reverse", 0.5),
        ("alternate", 0.5),
        ("alternate-reverse", 0.5),
    ] {
        let timeline = started(&format!("fade 1s linear 2.5 forwards {direction}"));
        assert_eq!(sample(&timeline, 2500.0), Some(end));
        assert_eq!(timeline.next_sample_ms(2500.0), None);
    }
    let alternate = started("fade 1s linear alternate 2 both");
    assert_eq!(sample(&alternate, 1250.0), Some(0.75));
    assert_eq!(sample(&alternate, 2000.0), Some(0.0));
    let forever = started("fade 1s linear infinite");
    assert_eq!(sample(&forever, 2250.0), Some(0.25));
    assert!(forever.next_sample_ms(2250.0).is_some());
}

#[test]
fn pausing_and_resuming_preserve_local_time_across_recascades() {
    let mut timeline = started("fade 1s linear both");
    timeline
        .update(&styled("fade 1s linear both paused", ""), 300.0)
        .unwrap();
    assert_eq!(sample(&timeline, 800.0), Some(0.3));
    assert_eq!(timeline.next_sample_ms(800.0), None);
    timeline
        .update(&styled("fade 1s linear both", ""), 800.0)
        .unwrap();
    assert_eq!(sample(&timeline, 1000.0), Some(0.5));
}

#[test]
fn zero_duration_and_zero_iterations_settle_without_permanent_wakeups() {
    assert_eq!(sample(&started("fade 0s linear forwards"), 0.0), Some(1.0));
    let empty = started("fade 1s linear 0 forwards");
    assert_eq!(sample(&empty, 0.0), Some(0.0));
    assert_eq!(empty.next_sample_ms(0.0), None);
    let forever = started("fade 0s infinite both");
    assert_eq!(forever.next_sample_ms(0.0), None);
}

#[test]
fn recascade_changes_timing_and_missing_endpoints_without_restarting() {
    let mut timeline = started("fade 1s linear both");
    timeline
        .update(&styled("fade 2s linear both", ""), 500.0)
        .unwrap();
    assert_eq!(sample(&timeline, 500.0), Some(0.25));
    let document = compute_styles(
        &parse("<p>sample</p>").unwrap(),
        &["@keyframes end{to{opacity:1}} p{opacity:.2;animation:end 1s linear both}".into()],
    )
    .unwrap();
    let mut partial = OpacityAnimations::default();
    partial.update(&document, 0.0).unwrap();
    assert!((sample(&partial, 500.0).unwrap() - 0.6).abs() < 0.000001);
}

#[test]
fn keyframe_easing_duplicate_offsets_and_custom_properties_are_resolved() {
    let document = compute_styles(&parse("<p>sample</p>").unwrap(), &["@keyframes pulse{0%{opacity:0;animation-timing-function:steps(2,end)}50%{opacity:.5}50%{opacity:var(--middle)}100%{opacity:0}} p{--middle:1;animation:pulse 1s linear both}".into()]).unwrap();
    let mut timeline = OpacityAnimations::default();
    timeline.update(&document, 0.0).unwrap();
    assert_eq!(sample(&timeline, 100.0), Some(0.0));
    assert_eq!(sample(&timeline, 300.0), Some(0.5));
    assert_eq!(sample(&timeline, 750.0), Some(0.5));
    let steps = started("fade 1s step-start 250ms both");
    assert_eq!(sample(&steps, 0.0), Some(0.0));
    assert_eq!(sample(&steps, 250.0), Some(1.0));
}

#[test]
fn important_opacity_suppresses_effects_without_restarting_playback() {
    let mut timeline = started("fade 1s linear both");
    timeline
        .update(
            &styled("fade 1s linear both", "opacity:.8!important"),
            200.0,
        )
        .unwrap();
    assert_eq!(sample(&timeline, 300.0), None);
    timeline
        .update(&styled("fade 1s linear both", ""), 500.0)
        .unwrap();
    assert_eq!(sample(&timeline, 500.0), Some(0.5));
}

#[test]
fn hidden_removed_and_reintroduced_targets_restart_playback() {
    let mut timeline = started("fade 1s linear both");
    timeline
        .update(&styled("fade 1s linear both", "display:none"), 300.0)
        .unwrap();
    assert_eq!(sample(&timeline, 400.0), None);
    timeline
        .update(&styled("fade 1s linear both", ""), 500.0)
        .unwrap();
    assert_eq!(sample(&timeline, 600.0), Some(0.1));
    let mut removed = styled("fade 1s linear both", "");
    removed.nodes.clear();
    timeline.update(&removed, 700.0).unwrap();
    assert!(timeline.effects(700.0).is_empty());
}

#[test]
fn invalid_timing_and_hostile_wire_values_preserve_prior_state() {
    for invalid in ["fade -1s", "fade 1s steps(0,end)", "fade 1s -2", "fade 1s,"] {
        let document = styled("fade 2s linear both", &format!("animation:{invalid}"));
        assert_eq!(
            document.nodes[0].style.opacity_animations[0].duration_ms,
            2000.0
        );
    }
    let mut timeline = started("fade 1s linear both");
    let mut bad = styled("fade 1s linear both", "");
    bad.nodes[0].style.opacity_animations[0].duration_ms = f64::NAN;
    assert!(timeline.update(&bad, 200.0).is_err());
    assert!(
        timeline
            .update(&styled("fade 1s linear both", ""), -1.0)
            .is_err()
    );
    assert_eq!(sample(&timeline, 500.0), Some(0.5));
}
