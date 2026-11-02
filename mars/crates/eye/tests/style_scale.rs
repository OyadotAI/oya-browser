/** Pure scale timelines retain independent authored axes, native timing semantics and explicit unsupported boundaries. */
use mars_eye::{
    Document, Scale, ScaleAnimations, ScaleTransitions, TransitionTarget, compute_styles, parse,
};

/** Stable IDs make successive styles represent the same native target. */
fn styled(transform: &str, extra: &str) -> Document {
    compute_styles(&parse("<div id=target>sample</div>").unwrap(), &[format!(
        "#target{{width:200px;height:100px;transform:{transform};transition:transform 1s linear;{extra}}}"
    )]).unwrap()
}

/** Wrapper timelines never substitute for the ordinary target under observation. */
fn axes(effects: Vec<(TransitionTarget, Scale)>) -> [f64; 2] {
    let value = effects
        .into_iter()
        .find(|(target, _)| matches!(target, TransitionTarget::Node(_)))
        .unwrap()
        .1;
    [value.x, value.y]
}

#[test]
fn independent_axes_cross_zero_and_reverse_from_current_presentation() {
    let initial = styled("none", "");
    let after = styled("scale(3,-1)", "");
    let mut timeline = ScaleTransitions::default();
    timeline.update(&initial, 0.0).unwrap();
    timeline.update(&after, 0.0).unwrap();
    assert_eq!(axes(timeline.effects(500.0)), [2.0, 0.0]);
    assert_eq!(after.nodes[0].style.scale_factors, Some([3.0, -1.0]));
    timeline.update(&initial, 500.0).unwrap();
    assert_eq!(axes(timeline.effects(750.0)), [1.5, 0.5]);
    assert_eq!(timeline.next_sample_ms(1000.0), None);
    assert!(timeline.effects(1000.0).is_empty());
    assert_eq!(
        styled("scaleX(2)", "").nodes[0].style.scale_factors,
        Some([2.0, 1.0])
    );
    assert_eq!(
        styled("scaleY(-2)", "").nodes[0].style.scale_factors,
        Some([1.0, -2.0])
    );
}

/** Parsed keyframes use the same underlying transform while pause changes preserve local time. */
fn animated(paused: bool) -> Document {
    compute_styles(&parse("<div id=target>sample</div>").unwrap(), &[format!(
        "@keyframes pulse{{from{{transform:scale(1,2)}}to{{transform:scale(3,4)}}}}#target{{transform:none;animation:pulse 1s linear 2 alternate both;animation-play-state:{}}}",
        if paused { "paused" } else { "running" }
    )]).unwrap()
}

#[test]
fn scale_keyframes_preserve_pause_direction_and_completed_fill_without_ticks() {
    let running = animated(false);
    let mut timeline = ScaleAnimations::default();
    timeline.update(&running, 0.0).unwrap();
    assert_eq!(axes(timeline.effects(250.0)), [1.5, 2.5]);
    timeline.update(&animated(true), 250.0).unwrap();
    assert_eq!(axes(timeline.effects(800.0)), [1.5, 2.5]);
    assert_eq!(timeline.next_sample_ms(800.0), None);
    timeline.update(&running, 1000.0).unwrap();
    assert_eq!(axes(timeline.effects(1750.0)), [3.0, 4.0]);
    assert_eq!(axes(timeline.effects(2250.0)), [2.0, 3.0]);
    assert_eq!(axes(timeline.effects(2750.0)), [1.0, 2.0]);
    assert_eq!(timeline.next_sample_ms(2750.0), None);
    assert!(running.nodes[0].style.transform.is_none());
}

#[test]
fn mixed_lists_matrices_three_dimensions_and_individual_properties_do_not_become_scale_endpoints() {
    for (transform, extra) in [
        ("matrix(2,0,0,2,0,0)", ""),
        ("scale3d(2,2,1)", ""),
        ("scale(2) rotate(0deg)", ""),
        ("scale(2)", "scale:1"),
    ] {
        let mut timeline = ScaleTransitions::default();
        timeline.update(&styled(transform, extra), 0.0).unwrap();
        timeline.update(&styled("scale(3)", ""), 0.0).unwrap();
        assert!(timeline.effects(500.0).is_empty(), "{transform} {extra}");
    }
    let document = compute_styles(&parse("<div></div>").unwrap(), &["@keyframes grow{from{transform:none}to{transform:scale(2)}}@keyframes spin{from{transform:none}to{transform:rotate(1turn)}}div{animation:grow 1s,spin 1s}".into()]).unwrap();
    assert!(document.nodes[0].style.scale_animations.is_empty());
    assert!(document.nodes[0].style.rotation_animations.is_empty());
}
