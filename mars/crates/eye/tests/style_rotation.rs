/** Authored rotation angles drive real timeline samples and native transformed bounds without matrix lerp. */
use mars_eye::{
    Document, Rotation, RotationAnimations, RotationTransitions, TransitionTarget, compute_styles,
    painted_bounds, parse, render,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/** Stable IDs let each restyle target the same native element. */
fn styled(transform: &str, extra: &str) -> Document {
    compute_styles(&parse("<div id=target>sample</div>").unwrap(), &[format!("#target{{width:200px;height:100px;transform:{transform};transition:transform 1s linear;{extra}}}")]).unwrap()
}

/** Select one ordinary target independently of document wrapper effects. */
fn angle(effects: Vec<(TransitionTarget, Rotation)>) -> Option<f64> {
    effects
        .into_iter()
        .find(|(target, _)| matches!(target, TransitionTarget::Node(_)))
        .map(|(_, sample)| sample.radians)
}

/** Floating angle conversion preserves meaningful turns without requiring identical conversion rounding. */
fn near(actual: Option<f64>, expected: f64) {
    assert!((actual.unwrap() - expected).abs() < 0.000001);
}

#[test]
fn a_full_turn_transition_changes_native_bounds_at_the_existing_transform_origin() {
    let mut timeline = RotationTransitions::default();
    timeline
        .update(&styled("rotate(0deg)", "transform-origin:0 0"), 0.0)
        .unwrap();
    let authored = styled("rotate(360deg)", "transform-origin:0 0");
    timeline.update(&authored, 0.0).unwrap();
    near(angle(timeline.effects(250.0)), FRAC_PI_2);
    near(authored.nodes[0].style.rotation_radians, TAU);
    let mut painted = authored.clone();
    Rotation {
        radians: angle(timeline.effects(250.0)).unwrap(),
    }
    .apply(&mut painted.nodes[0].style)
    .unwrap();
    assert_eq!(
        painted.nodes[0].style.transform_origin,
        authored.nodes[0].style.transform_origin
    );
    let display = render(&painted, 800.0).unwrap();
    let item = display
        .items
        .iter()
        .find(|item| item.local_id == authored.nodes[0].local_id)
        .unwrap();
    let bounds = painted_bounds(item);
    assert!((bounds.width - item.height).abs() < 0.000001);
    assert!((bounds.height - item.width).abs() < 0.000001);
    assert!(timeline.effects(1000.0).is_empty());
    assert_eq!(timeline.next_sample_ms(1000.0), None);
}

#[test]
fn rotation_reversal_starts_at_current_angle_and_shortens_the_remaining_duration() {
    let mut timeline = RotationTransitions::default();
    let initial = styled("none", "");
    timeline.update(&initial, 0.0).unwrap();
    timeline.update(&styled("rotate(1turn)", ""), 0.0).unwrap();
    timeline.update(&initial, 400.0).unwrap();
    near(angle(timeline.effects(400.0)), TAU * 0.4);
    near(angle(timeline.effects(600.0)), TAU * 0.2);
    assert_eq!(timeline.next_sample_ms(800.0), None);
}

#[test]
fn matrices_mixed_transforms_and_individual_properties_never_become_angular_endpoints() {
    for (transform, extra) in [
        ("matrix(1,0,0,1,0,0)", ""),
        ("rotateZ(0deg)", ""),
        ("rotate(0deg) translateX(0px)", ""),
        ("rotate(0deg)", "scale:1"),
        ("rotate(0deg) rotate(0deg)", ""),
    ] {
        let mut timeline = RotationTransitions::default();
        timeline.update(&styled(transform, extra), 0.0).unwrap();
        timeline.update(&styled("rotate(1turn)", ""), 0.0).unwrap();
        assert!(timeline.effects(500.0).is_empty(), "{transform} {extra}");
    }
}

/** Real parsed animation declarations retain their authored transform independently of live angles. */
fn animated(animation: &str, frames: &str, extra: &str) -> Document {
    compute_styles(
        &parse("<div id=target>sample</div>").unwrap(),
        &[format!(
            "@keyframes spin{{{frames}}}#target{{animation:{animation};{extra}}}"
        )],
    )
    .unwrap()
}
const SPIN: &str = "from{transform:rotate(0deg)}to{transform:rotate(360deg)}";

#[test]
fn infinite_spinner_advances_a_full_turn_without_mutating_its_base_transform() {
    let authored = animated("spin 1s linear infinite", SPIN, "opacity:.4");
    assert!(authored.nodes[0].style.transform.is_none());
    let mut timeline = RotationAnimations::default();
    timeline.update(&authored, 0.0).unwrap();
    near(angle(timeline.effects(500.0)), PI);
    near(angle(timeline.effects(1250.0)), FRAC_PI_2);
    assert!(timeline.next_sample_ms(1250.0).is_some());
    assert_eq!(authored.nodes[0].style.opacity, 0.4);
}

#[test]
fn rotation_delay_direction_pause_resume_and_fill_keep_local_time() {
    let mut timeline = RotationAnimations::default();
    let animation = "spin 1s linear 250ms 2 alternate both";
    timeline
        .update(&animated(animation, SPIN, ""), 0.0)
        .unwrap();
    near(angle(timeline.effects(100.0)), 0.0);
    timeline
        .update(
            &animated(animation, SPIN, "animation-play-state:paused"),
            750.0,
        )
        .unwrap();
    near(angle(timeline.effects(1500.0)), PI);
    assert_eq!(timeline.next_sample_ms(1500.0), None);
    timeline
        .update(&animated(animation, SPIN, ""), 1500.0)
        .unwrap();
    near(angle(timeline.effects(2250.0)), TAU * 0.75);
    near(angle(timeline.effects(3000.0)), 0.0);
    assert_eq!(timeline.next_sample_ms(3000.0), None);
}

#[test]
fn missing_rotation_endpoint_uses_authored_angle_and_important_transform_suppresses_playback() {
    let frames = "to{transform:rotate(1turn)}";
    let mut timeline = RotationAnimations::default();
    timeline
        .update(
            &animated("spin 1s linear both", frames, "transform:rotate(180deg)"),
            0.0,
        )
        .unwrap();
    near(angle(timeline.effects(500.0)), PI * 1.5);
    timeline
        .update(
            &animated(
                "spin 1s linear both",
                frames,
                "transform:rotate(90deg)!important",
            ),
            500.0,
        )
        .unwrap();
    assert!(timeline.effects(500.0).is_empty());
    assert_eq!(timeline.next_sample_ms(500.0), None);
}

#[test]
fn any_unsupported_transform_keyframe_keeps_the_track_out_of_live_rotation() {
    for transform in [
        "scale(2)",
        "rotateZ(90deg)",
        "rotate(90deg) translateX(2px)",
    ] {
        let document = animated(
            "spin 1s linear infinite",
            &format!("from{{transform:rotate(0deg)}}to{{transform:{transform}}}"),
            "",
        );
        assert!(document.nodes[0].style.rotation_animations.is_empty());
        let mut timeline = RotationAnimations::default();
        timeline.update(&document, 0.0).unwrap();
        assert!(timeline.effects(500.0).is_empty());
    }
}
