/**
 * CSS Color 4/5 functions and keywords as Blink resolves them: `lab()`, `lch()`, `hwb()`,
 * `color()`, `color-mix()` (including `currentcolor`) and the system colors per color scheme.
 * Expected values come from the CSS Color 4 sample conversions and WPT's color-mix tests.
 */
use mars_eye::{Appearance, RenderContext, compute_styles_with_context, parse};

/** The computed background of `#t` given `declarations`, in `appearance`. */
fn background(declarations: &str, appearance: Appearance) -> Option<String> {
    let context = RenderContext {
        appearance,
        ..RenderContext::default()
    };
    let sheet = format!(":root {{ color-scheme: light dark }} #t {{ {declarations} }}");
    let document =
        compute_styles_with_context(&parse("<p id=t>x</p>").unwrap(), &[sheet], context).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.background.clone()
}
/** Light-scheme background. */
fn light(declarations: &str) -> Option<String> {
    background(declarations, Appearance::Light)
}

#[test]
fn lab_and_lch_convert_from_d50_to_srgb() {
    assert_eq!(
        light("background-color: lab(100% 0 0)").as_deref(),
        Some("#ffffff")
    );
    assert_eq!(
        light("background-color: lab(0 0 0 / 50%)").as_deref(),
        Some("#00000080")
    );
    // sRGB red is lab(54.29 80.8 69.89) / lch(54.29 106.84 40.85).
    assert_eq!(
        light("background-color: lab(54.29 80.8 69.89)").as_deref(),
        Some("#ff0000")
    );
    assert_eq!(
        light("background-color: lch(54.29 106.84 40.85)").as_deref(),
        Some("#ff0000")
    );
}

#[test]
fn hwb_mixes_white_and_black_into_the_hue() {
    assert_eq!(
        light("background-color: hwb(0 0% 0%)").as_deref(),
        Some("#ff0000")
    );
    assert_eq!(
        light("background-color: hwb(120 0% 50%)").as_deref(),
        Some("#008000")
    );
    assert_eq!(
        light("background-color: hwb(0 60% 60%)").as_deref(),
        Some("#808080")
    );
}

#[test]
fn color_function_reads_srgb_and_display_p3() {
    assert_eq!(
        light("background-color: color(srgb 1 0 0)").as_deref(),
        Some("#ff0000")
    );
    assert_eq!(
        light("background-color: color(display-p3 1 1 1)").as_deref(),
        Some("#ffffff")
    );
}

#[test]
fn color_mix_in_srgb_averages_components() {
    let mixed = light("background-color: color-mix(in srgb, red, blue)");
    assert_eq!(mixed.as_deref(), Some("#800080"));
    let weighted = light("background-color: color-mix(in srgb, red 25%, blue)");
    assert_eq!(weighted.as_deref(), Some("#4000bf"));
}

#[test]
fn color_mix_with_transparent_scales_alpha_premultiplied() {
    let mixed = light("background-color: color-mix(in oklab, red 50%, transparent)");
    assert_eq!(mixed.as_deref(), Some("#ff000080"));
}

#[test]
fn color_mix_percentages_under_100_reduce_alpha() {
    let mixed = light("background-color: color-mix(in srgb, red 20%, red 30%)");
    assert_eq!(mixed.as_deref(), Some("#ff000080"));
}

#[test]
fn color_mix_resolves_currentcolor_against_the_element() {
    let mixed =
        light("color: blue; background-color: color-mix(in srgb, currentcolor 50%, transparent)");
    assert_eq!(mixed.as_deref(), Some("#0000ff80"));
}

#[test]
fn system_colors_follow_the_used_color_scheme() {
    assert_eq!(
        light("background-color: Canvas").as_deref(),
        Some("#ffffff")
    );
    assert_eq!(
        background("background-color: Canvas", Appearance::Dark).as_deref(),
        Some("#121212")
    );
    assert_eq!(
        light("background-color: ButtonFace").as_deref(),
        Some("#efefef")
    );
}
