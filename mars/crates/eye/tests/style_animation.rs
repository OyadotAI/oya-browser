/** Static frames rest animations where a person sees them, and never reveal what Eye could not evaluate. */
use mars_eye::{Document, Node, compute_styles, parse};

/** Styles `markup` with one sheet. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document.nodes.iter().find(|n| n.html_id == id).unwrap()
}
/** Opacity of `#a` under `sheet`. */
fn opacity(sheet: &str) -> f64 {
    by_id(&styled("<p id=a>a</p>", sheet), "a").style.opacity
}
/** A fade-in from opacity 0, as entrance animations are written. */
const FADE: &str = "@keyframes fade{0%{opacity:0;transform:translateY(20px)}to{opacity:1;transform:translateY(0)}}";

#[test]
fn a_forwards_fill_rests_on_the_final_keyframe() {
    assert_eq!(
        opacity(&format!(
            "{FADE} p{{opacity:0;animation:.6s ease-out both fade}}"
        )),
        1.0
    );
    assert_eq!(
        opacity(&format!(
            "{FADE} p{{opacity:0;animation-name:fade;animation-fill-mode:forwards}}"
        )),
        1.0
    );
}

#[test]
fn the_final_keyframe_clears_the_entrance_transform() {
    let document = styled(
        "<p id=a>a</p>",
        &format!("{FADE} p{{transform:translateY(14px);animation:fade 1s forwards}}"),
    );
    assert_eq!(
        by_id(&document, "a").style.transform,
        Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    );
}

#[test]
fn without_a_forwards_fill_a_finished_animation_shows_the_base_style() {
    assert_eq!(
        opacity(&format!("{FADE} p{{opacity:.5;animation:fade 1s}}")),
        0.5
    );
}

#[test]
fn reverse_and_alternate_parity_pick_the_end_the_last_iteration_reaches() {
    let sheet = |extra: &str| format!("{FADE} p{{opacity:.5;animation:fade 1s forwards {extra}}}");
    assert_eq!(opacity(&sheet("reverse")), 0.0);
    assert_eq!(opacity(&sheet("alternate 2")), 0.0);
    assert_eq!(opacity(&sheet("alternate 3")), 1.0);
    assert_eq!(opacity(&sheet("alternate-reverse 2")), 1.0);
}

#[test]
fn an_infinite_animation_rests_on_its_start_unless_the_start_hides() {
    let pulse = "@keyframes pulse{0%{opacity:.8}50%{opacity:.2}to{opacity:.9}}";
    assert_eq!(
        opacity(&format!("{pulse} p{{animation:pulse 2s infinite}}")),
        0.8
    );
    let ping = "@keyframes ping{0%{opacity:0}to{opacity:.7}}";
    assert_eq!(
        opacity(&format!("{ping} p{{animation:ping 2s infinite}}")),
        0.7
    );
    let blink = "@keyframes blink{0%{opacity:0}to{opacity:0}}";
    assert_eq!(
        opacity(&format!(
            "{blink} p{{opacity:.4;animation:blink 1s infinite}}"
        )),
        0.4
    );
}

#[test]
fn important_declarations_beat_animations() {
    assert_eq!(
        opacity(&format!(
            "{FADE} p{{opacity:.3!important;animation:fade 1s both}}"
        )),
        0.3
    );
}

#[test]
fn keyframe_names_are_case_sensitive_and_the_last_rule_wins() {
    let sheet =
        "@keyframes Up{to{opacity:.2}} @keyframes Up{to{opacity:.6}} p{animation:Up 1s forwards}";
    assert_eq!(opacity(sheet), 0.6);
    assert_eq!(
        opacity("@keyframes up{to{opacity:.2}} p{animation:Up 1s forwards}"),
        1.0
    );
}

#[test]
fn animations_resolve_through_custom_properties() {
    let sheet = format!("{FADE} :root{{--in:fade .5s both}} p{{opacity:0;animation:var(--in)}}");
    assert_eq!(opacity(&sheet), 1.0);
}

#[test]
fn a_later_animation_in_the_list_wins() {
    let sheet = "@keyframes a{to{opacity:.2}} @keyframes b{to{opacity:.7}} p{animation:a 1s forwards,b 1s forwards}";
    assert_eq!(opacity(sheet), 0.7);
}

#[test]
fn keyframes_under_an_unknown_condition_never_reveal_and_mask_when_they_could_hide() {
    let document = styled(
        "<p id=a>a</p><a id=b href=/x>b</a>",
        "@container (min-width:1px){@keyframes in{to{opacity:1}} @keyframes out{to{opacity:0}}} \
         p{opacity:0;animation:in 1s forwards} a{animation:out 1s forwards}",
    );
    assert_eq!(by_id(&document, "a").style.opacity, 0.0);
    assert!(!by_id(&document, "b").style.interactive);
}

#[test]
fn a_finished_fade_out_hides_and_withdraws_nothing_it_should_not() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        "@keyframes out{to{opacity:0}} a{animation:out 1s forwards}",
    );
    assert_eq!(by_id(&document, "a").style.opacity, 0.0);
}

#[test]
fn transition_declarations_are_retained_without_diagnostics() {
    let document = styled(
        "<p id=a>a</p>",
        "p{transition:opacity .2s;-webkit-transition:all 1s}",
    );
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("transition"))
    );
    assert_eq!(by_id(&document, "a").style.transition.durations, vec![1000.0]);
}

#[test]
fn animation_properties_leave_no_unsupported_diagnostic() {
    let document = styled(
        "<p id=a>a</p>",
        &format!("{FADE} p{{animation:fade 1s;animation-delay:1s}}"),
    );
    assert!(
        document
            .diagnostics
            .iter()
            .all(|d| !d.contains("animation")),
        "{:?}",
        document.diagnostics
    );
}

#[test]
fn hostile_keyframes_stay_bounded() {
    let frames: String = (0..200)
        .map(|i| format!("{}%{{opacity:{}}}", i % 101, i % 2))
        .collect();
    let sets: String = (0..1100)
        .map(|i| format!("@keyframes k{i}{{to{{opacity:1}}}}"))
        .collect();
    let sheet = format!(
        "@keyframes big{{{frames}}} {sets} @keyframes{{to{{}}}} p{{animation:big 1s forwards}} a{{animation:k1099 1s forwards}}"
    );
    let document = styled("<p id=a>a</p><a id=b href=/x>b</a>", &sheet);
    assert_eq!(by_id(&document, "a").style.opacity, 1.0);
    assert!(!by_id(&document, "b").style.interactive);
}

#[test]
fn minified_adjacent_transform_functions_parse() {
    let document = styled("<p id=a>a</p>", "p{transform:translateY(-8px)scale(.5)}");
    assert_eq!(
        by_id(&document, "a").style.transform,
        Some([0.5, 0.0, 0.0, 0.5, 0.0, -8.0])
    );
}

/** Whether `#a`, a link hidden by its base style and revealed by `animation`, is agent-actionable. */
fn revealed_actionable(animation: &str) -> bool {
    let sheet = format!(
        "{FADE} @keyframes blink{{0%{{opacity:1}}50%{{opacity:0}}to{{opacity:1}}}} a{{opacity:0;animation:{animation}}}"
    );
    by_id(&styled("<a id=a href=/x>a</a>", &sheet), "a")
        .style
        .interactive
}

#[test]
fn a_short_forwards_fade_in_is_agent_actionable() {
    assert!(revealed_actionable("fade .6s ease-out both"));
    assert!(revealed_actionable("fade .6s .3s forwards"));
}

#[test]
fn a_paused_reveal_is_masked_from_agents() {
    assert!(!revealed_actionable("fade .6s both paused"));
}

#[test]
fn a_reveal_after_a_long_delay_is_masked_from_agents() {
    assert!(!revealed_actionable("fade 1s 10s forwards"));
    assert!(!revealed_actionable("fade 2s 3 forwards"));
}

#[test]
fn an_infinite_animation_never_reveals_for_agents() {
    assert!(!revealed_actionable("blink 1s infinite"));
    let document = styled(
        "<a id=a href=/x>a</a>",
        &format!("{FADE} a{{opacity:0;animation:fade 1s infinite reverse}}"),
    );
    let a = &by_id(&document, "a").style;
    assert_eq!(a.opacity, 1.0);
    assert!(!a.interactive);
}

#[test]
fn an_animation_that_does_not_reveal_keeps_actions() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        "@keyframes spin{to{transform:rotate(360deg)}} a{animation:spin 1s infinite}",
    );
    assert!(by_id(&document, "a").style.interactive);
}

#[test]
fn a_dropped_rule_with_calc_subtraction_margins_does_not_mask_everything() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        ":where(.s>:not(:last-child)){margin-block-end:calc(var(--sp) * calc(1 - var(--r)))}",
    );
    assert!(by_id(&document, "a").style.interactive);
    assert!(document.formatting.html.style.interactive);
}

/** A margin only shifts a box within its flow, so a dropped rule setting a negative one never
masks: spacing utilities (X's `-space-x-3`) would otherwise withdraw every link. */
#[test]
fn a_dropped_rule_with_a_negative_margin_never_masks() {
    let document = styled(
        "<a id=a href=/x>a</a>",
        ":where(a>b, a){margin-left:calc(var(--sp) * -4)}",
    );
    assert!(by_id(&document, "a").style.interactive);
}

#[test]
fn dropped_hiding_rules_that_cannot_match_the_root_leave_the_page_actionable() {
    for sheet in [
        ".client .wrap > :is(p,thead + tbody){display:none}",
        ":is(.a .b):nth-child(n+4){display:none}",
        ":is(){position:absolute;left:-9999px}",
        ":is(.a .b):before{display:none}",
    ] {
        let document = styled("<a id=a href=/x>a</a>", sheet);
        assert!(document.formatting.html.style.interactive, "{sheet}");
    }
}

#[test]
fn a_dropped_hiding_rule_whose_subject_could_be_anything_still_masks() {
    let document = styled("<a id=a href=/x>a</a>", ":is(a, b c){display:none}");
    assert!(!by_id(&document, "a").style.interactive);
}

#[test]
fn a_dropped_is_list_masks_each_argument_subject_not_everything() {
    let document = styled(
        "<p id=p>p</p><a id=a href=/x>a</a><table><tbody><tr><td><a id=t href=/y>t</a></td></tr></tbody></table>",
        ".wrap > :is(p, thead + tbody){display:none}",
    );
    assert!(by_id(&document, "a").style.interactive);
    assert!(document.formatting.body.style.interactive);
}

/** Unsupported animation percentages cannot become universal hiding selectors. */
#[test]
fn vendor_keyframe_descriptors_leave_unanimated_links_actionable() {
    for prefix in ["-moz", "-o", "-ms"] {
        let frames =
            format!("@{prefix}-keyframes spinner{{0%,39%,100%{{opacity:0}}40%{{opacity:1}}}}");
        for sheet in [frames.clone(), format!("@media screen{{{frames}}}")] {
            let document = styled("<a id=a href=/next>Next</a>", &sheet);
            assert!(document.formatting.html.style.interactive, "{sheet}");
            assert!(document.formatting.body.style.interactive, "{sheet}");
            assert!(by_id(&document, "a").style.interactive, "{sheet}");
            assert!(
                document
                    .diagnostics
                    .iter()
                    .any(|message| message.contains(&format!("@{prefix}-keyframes")))
            );
        }
    }
}

/** Ignoring known descriptor syntax never relaxes ordinary or uncertain hiding rules. */
#[test]
fn vendor_keyframes_preserve_real_and_unknown_hiding_masks() {
    let frames = "@-moz-keyframes spinner{0%{opacity:0}to{opacity:1}}";
    for rule in [
        "#a{display:none}",
        "#a{opacity:0}",
        "@future-condition{#a{display:none}}",
    ] {
        let document = styled("<a id=a href=/next>Next</a>", &format!("{frames}{rule}"));
        let node = by_id(&document, "a");
        if rule.contains("opacity") {
            assert_eq!(node.style.opacity, 0.0);
            let display = mars_eye::render(&document, 800.0).unwrap();
            assert!(
                !display
                    .items
                    .iter()
                    .any(|item| item.local_id == node.local_id && mars_eye::agent_visible(item))
            );
        } else {
            assert!(!node.style.interactive, "{rule}");
        }
    }
}
