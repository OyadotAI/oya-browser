/**
 * Kept properties: parsed, canonicalized, inherited and reported by `getComputedStyle` even
 * though no pixel changes. Each rule is one test; invalid values must drop the declaration and
 * leave the initial value in place, as Blink does.
 */
use mars_eye::{COMPUTED_PROPERTIES, Document, Node, compute_styles, computed_css_value, parse};

/** Styles `markup` with one sheet at the default viewport. */
fn styled(markup: &str, sheet: &str) -> Document {
    compute_styles(&parse(markup).unwrap(), &[sheet.into()]).unwrap()
}
/** The node with `id`. */
fn by_id<'a>(document: &'a Document, id: &str) -> &'a Node {
    document
        .nodes
        .iter()
        .find(|node| node.html_id == id)
        .unwrap()
}
/** The resolved value `getComputedStyle` reports for `property` on the node with `id`. */
fn computed(document: &Document, id: &str, property: &str) -> String {
    computed_css_value(&by_id(document, id).style, property).unwrap()
}
/** Whether a diagnostic names `property` as unsupported. */
fn dropped(document: &Document, property: &str) -> bool {
    document
        .diagnostics
        .iter()
        .any(|d| d.contains(property) && d.contains("not supported"))
}

#[test]
fn every_kept_property_is_listed_for_get_computed_style() {
    for name in [
        "appearance",
        "background-blend-mode",
        "font-feature-settings",
        "font-synthesis",
        "font-variation-settings",
        "forced-color-adjust",
        "list-style-position",
        "mask-composite",
        "overscroll-behavior-x",
        "resize",
        "scroll-padding-top",
        "scroll-snap-align",
        "scroll-snap-stop",
        "scrollbar-color",
        "scrollbar-gutter",
        "scrollbar-width",
        "text-size-adjust",
        "touch-action",
        "user-select",
        "view-transition-name",
    ] {
        assert!(COMPUTED_PROPERTIES.contains(&name), "{name} is not listed");
    }
}

#[test]
fn an_unstyled_kept_property_reports_its_initial_value() {
    let document = styled("<p id=a>a</p>", "");
    assert_eq!(computed(&document, "a", "touch-action"), "auto");
    assert_eq!(computed(&document, "a", "user-select"), "auto");
    assert_eq!(computed(&document, "a", "scroll-snap-align"), "none");
    assert_eq!(computed(&document, "a", "list-style-position"), "outside");
}

#[test]
fn touch_action_keeps_pan_axes_and_pinch_zoom_in_source_order() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p><p id=c>c</p>",
        "#a{touch-action:pan-x pan-y pinch-zoom} #b{touch-action:MANIPULATION} #c{touch-action:pan-x pan-left}",
    );
    assert_eq!(
        computed(&document, "a", "touch-action"),
        "pan-x pan-y pinch-zoom"
    );
    assert_eq!(computed(&document, "b", "touch-action"), "manipulation");
    assert_eq!(computed(&document, "c", "touch-action"), "auto");
    assert!(dropped(&document, "touch-action"));
}

#[test]
fn a_pan_axis_cannot_repeat_within_touch_action() {
    let document = styled("<p id=a>a</p>", "#a{touch-action:pan-x pan-right}");
    assert_eq!(computed(&document, "a", "touch-action"), "auto");
}

#[test]
fn overscroll_behavior_shorthand_sets_both_axes() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{overscroll-behavior:contain none} #b{overscroll-behavior:none}",
    );
    assert_eq!(computed(&document, "a", "overscroll-behavior-x"), "contain");
    assert_eq!(computed(&document, "a", "overscroll-behavior-y"), "none");
    assert_eq!(computed(&document, "b", "overscroll-behavior-x"), "none");
    assert_eq!(computed(&document, "b", "overscroll-behavior-y"), "none");
}

#[test]
fn scroll_padding_shorthand_expands_clockwise_and_canonicalizes_px() {
    let document = styled(
        "<p id=a>a</p>",
        "#a{scroll-padding:0 16px} #a{scroll-padding-top:1rem} #a{scroll-padding-bottom:50%}",
    );
    assert_eq!(computed(&document, "a", "scroll-padding-top"), "16px");
    assert_eq!(computed(&document, "a", "scroll-padding-right"), "16px");
    assert_eq!(computed(&document, "a", "scroll-padding-bottom"), "50%");
    assert_eq!(computed(&document, "a", "scroll-padding-left"), "16px");
}

#[test]
fn negative_scroll_padding_is_dropped() {
    let document = styled("<p id=a>a</p>", "#a{scroll-padding-top:-4px}");
    assert_eq!(computed(&document, "a", "scroll-padding-top"), "auto");
    assert!(dropped(&document, "scroll-padding-top"));
}

#[test]
fn scroll_snap_align_takes_one_or_two_alignments() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p><p id=c>c</p>",
        "#a{scroll-snap-align:start} #b{scroll-snap-align:center end} #c{scroll-snap-align:start start start}",
    );
    assert_eq!(computed(&document, "a", "scroll-snap-align"), "start");
    assert_eq!(computed(&document, "b", "scroll-snap-align"), "center end");
    assert_eq!(computed(&document, "c", "scroll-snap-align"), "none");
}

#[test]
fn scroll_snap_stop_and_resize_accept_only_their_keywords() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{scroll-snap-stop:always;resize:vertical} #b{scroll-snap-stop:sometimes;resize:both-ish}",
    );
    assert_eq!(computed(&document, "a", "scroll-snap-stop"), "always");
    assert_eq!(computed(&document, "a", "resize"), "vertical");
    assert_eq!(computed(&document, "b", "scroll-snap-stop"), "normal");
    assert_eq!(computed(&document, "b", "resize"), "none");
}

#[test]
fn scrollbar_color_canonicalizes_colors_and_rejects_three() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{scrollbar-color:hsl(0,0%,67%) transparent} #b{scrollbar-color:red blue green}",
    );
    assert_eq!(
        computed(&document, "a", "scrollbar-color"),
        "rgb(171, 171, 171) rgba(0, 0, 0, 0)"
    );
    assert_eq!(computed(&document, "b", "scrollbar-color"), "auto");
}

#[test]
fn scrollbar_gutter_and_width_keep_their_keywords() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{scrollbar-gutter:both-edges stable;scrollbar-width:thin} #b{scrollbar-gutter:stable}",
    );
    assert_eq!(
        computed(&document, "a", "scrollbar-gutter"),
        "stable both-edges"
    );
    assert_eq!(computed(&document, "a", "scrollbar-width"), "thin");
    assert_eq!(computed(&document, "b", "scrollbar-gutter"), "stable");
}

#[test]
fn font_feature_settings_canonicalize_tags_and_keep_their_case() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p><p id=c>c</p>",
        "#a{font-feature-settings:'tnum' on,'lnum' on} #b{font-feature-settings:\"ss04\" 1} #c{font-feature-settings:'SS0'}",
    );
    assert_eq!(
        computed(&document, "a", "font-feature-settings"),
        "\"tnum\" 1, \"lnum\" 1"
    );
    assert_eq!(
        computed(&document, "b", "font-feature-settings"),
        "\"ss04\" 1"
    );
    assert_eq!(computed(&document, "c", "font-feature-settings"), "normal");
}

#[test]
fn font_feature_settings_off_is_zero_and_the_tag_stays_capitalized() {
    let document = styled("<p id=a>a</p>", "#a{font-feature-settings:'kern' off}");
    assert_eq!(
        computed(&document, "a", "font-feature-settings"),
        "\"kern\" 0"
    );
}

#[test]
fn font_variation_settings_keep_the_axis_case_and_signed_values() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{font-variation-settings:'DRKM' 1, 'wght' -0.5} #b{font-variation-settings:none}",
    );
    assert_eq!(
        computed(&document, "a", "font-variation-settings"),
        "\"DRKM\" 1, \"wght\" -0.5"
    );
    assert_eq!(
        computed(&document, "b", "font-variation-settings"),
        "normal"
    );
}

#[test]
fn font_synthesis_names_each_fake_face_once() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{font-synthesis:none} #b{font-synthesis:weight weight}",
    );
    assert_eq!(computed(&document, "a", "font-synthesis"), "none");
    assert_eq!(
        computed(&document, "b", "font-synthesis"),
        "weight style small-caps position"
    );
}

#[test]
fn inheritable_kept_properties_reach_children() {
    let document = styled(
        "<div id=a><p id=b>b</p></div>",
        "#a{list-style-position:inside;scrollbar-width:none;touch-action:none}",
    );
    assert_eq!(computed(&document, "b", "list-style-position"), "inside");
    assert_eq!(computed(&document, "b", "scrollbar-width"), "none");
    assert_eq!(computed(&document, "b", "touch-action"), "auto");
}

#[test]
fn inherit_copies_the_parent_and_initial_resets_it() {
    let document = styled(
        "<div id=a><p id=b>b</p><p id=c>c</p></div>",
        "#a{user-select:none} #b{user-select:inherit} #c{user-select:none;user-select:initial}",
    );
    assert_eq!(computed(&document, "b", "user-select"), "none");
    assert_eq!(computed(&document, "c", "user-select"), "auto");
}

#[test]
fn appearance_and_mask_composite_take_their_keyword_lists() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{appearance:none;mask-composite:add, intersect} #b{appearance:menulist-plain}",
    );
    assert_eq!(computed(&document, "a", "appearance"), "none");
    assert_eq!(computed(&document, "a", "mask-composite"), "add, intersect");
    assert_eq!(computed(&document, "b", "appearance"), "auto");
}

#[test]
fn background_blend_mode_lists_one_mode_per_layer() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{background-blend-mode:multiply, soft-light} #b{background-blend-mode:nope}",
    );
    assert_eq!(
        computed(&document, "a", "background-blend-mode"),
        "multiply, soft-light"
    );
    assert_eq!(computed(&document, "b", "background-blend-mode"), "normal");
}

#[test]
fn view_transition_name_keeps_its_identifier_case() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p><p id=c>c</p>",
        "#a{view-transition-name:Hero-Image} #b{view-transition-name:none} #c{view-transition-name:default}",
    );
    assert_eq!(
        computed(&document, "a", "view-transition-name"),
        "Hero-Image"
    );
    assert_eq!(computed(&document, "b", "view-transition-name"), "none");
    assert_eq!(computed(&document, "c", "view-transition-name"), "none");
}

#[test]
fn text_size_adjust_and_forced_color_adjust_validate_their_grammar() {
    let document = styled(
        "<p id=a>a</p><p id=b>b</p>",
        "#a{text-size-adjust:100%;forced-color-adjust:preserve-parent-color} #b{text-size-adjust:big;forced-color-adjust:maybe}",
    );
    assert_eq!(computed(&document, "a", "text-size-adjust"), "100%");
    assert_eq!(
        computed(&document, "a", "forced-color-adjust"),
        "preserve-parent-color"
    );
    assert_eq!(computed(&document, "b", "text-size-adjust"), "auto");
    assert_eq!(computed(&document, "b", "forced-color-adjust"), "auto");
}

#[test]
fn user_select_and_touch_action_stay_out_of_the_way_of_other_properties() {
    let document = styled("<p id=a>a</p>", "#a{user-select:none;color:red}");
    assert_eq!(computed(&document, "a", "user-select"), "none");
    assert_eq!(by_id(&document, "a").style.color.as_deref(), Some("red"));
}
