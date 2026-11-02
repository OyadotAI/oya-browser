/** Blink ports for CSS Values and CSS Text: math functions (calc, clamp, round, mod, rem), range
 * clamping of math results, and text spacing, each pinned by box geometry. */
use mars_eye::{compute_styles, parse, render};

/** (x, y, width, height) of `#id` after laying out `markup` at 800px with no body margin. */
fn boxed(markup: &str, id: &str) -> (f64, f64, f64, f64) {
    let markup = format!("<body style='margin:0'>{markup}");
    let document = compute_styles(&parse(&markup).unwrap(), &[]).unwrap();
    let display = render(&document, 800.0).unwrap();
    let local = document
        .nodes
        .iter()
        .find(|n| n.html_id == id)
        .unwrap()
        .local_id;
    let item = display.items.iter().find(|i| i.local_id == local).unwrap();
    (item.x, item.y, item.width, item.height)
}

/** The computed style of `#t` under `sheet`, inside `<div id=p>` styled by `parent`. */
fn style(parent: &str, own: &str) -> mars_eye::ComputedStyle {
    let sheet = format!("#p {{ {parent} }} #t {{ {own} }}");
    let markup = "<div id=p><span id=t>x</span></div>";
    let document = compute_styles(&parse(markup).unwrap(), &[sheet]).unwrap();
    (*document
        .nodes
        .iter()
        .find(|n| n.html_id == "t")
        .unwrap()
        .style)
        .clone()
}
/** The computed width of a `#t` block whose width is `value`. */
fn width(value: &str) -> f64 {
    boxed(
        &format!("<div id=t style='height:1px;width:{value}'></div>"),
        "t",
    )
    .2
}

/** `round()` defaults to `nearest`, ties toward positive infinity; the strategies pick a side. */
#[test]
fn round_steps_to_the_chosen_multiple() {
    assert_eq!(width("round(37px, 10px)"), 40.0);
    assert_eq!(width("round(35px, 10px)"), 40.0);
    assert_eq!(width("round(down, 37px, 10px)"), 30.0);
    assert_eq!(width("round(up, 31px, 10px)"), 40.0);
    assert_eq!(width("round(to-zero, 39px, 10px)"), 30.0);
    assert_eq!(width("calc(round(2.5) * 10px)"), 30.0);
}

/** `mod()` takes the divisor's sign and `rem()` the dividend's. */
#[test]
fn mod_and_rem_differ_in_sign() {
    assert_eq!(width("calc(100px + mod(-18px, 5px))"), 102.0);
    assert_eq!(width("calc(100px + rem(-18px, 5px))"), 97.0);
    assert_eq!(width("calc(mod(18px, 5px) * 10)"), 30.0);
}

/** `abs()`, `sign()` and the constants evaluate; `sign(0)` is 0, not 1. */
#[test]
fn sign_related_functions_and_constants() {
    assert_eq!(width("abs(-25px)"), 25.0);
    assert_eq!(width("calc(10px * sign(-3px) + 50px)"), 40.0);
    assert_eq!(width("calc(10px * sign(0) + 50px)"), 50.0);
    assert_eq!(width("calc(round(pi * 10) * 1px)"), 31.0);
}

/** A zero step has no value, so the declaration drops (auto width stays the full 800px). */
#[test]
fn a_zero_step_is_invalid() {
    assert_eq!(width("round(37px, 0px)"), 800.0);
}

/** A math result below a nonnegative property's range clamps to zero instead of dropping the
 * declaration (calc-invalid-range-clamping: `border-radius: 10px; border-radius: calc(-10px)`
 * leaves square corners). */
#[test]
fn a_negative_calc_clamps_to_the_range() {
    let radius = style("", "border-radius: 10px; border-radius: calc(-10px)").radius;
    assert_eq!(radius, [0.0; 4]);
}

/** `calc(1/32 * 100%)` keeps its `/` inside the `border-radius` shorthand (calc-border-radius-1). */
#[test]
fn a_division_inside_calc_is_not_the_radius_slash() {
    let radius = style(
        "",
        "width: 512px; border-radius: calc((1/32 * 100%) + 5px) 3px / 4px",
    )
    .radius;
    assert_eq!(radius, [21.0, 3.0, 21.0, 3.0]);
}

/** A word-spacing percentage is of the element's font size, as Blink's
 * FontDescription::WordSpacing resolves it (word-spacing-percent-001). */
#[test]
fn word_spacing_percent_is_of_the_font_size() {
    let own = style("", "font-size: 20px; word-spacing: 50%");
    assert_eq!(own.word_spacing, 10.0);
    assert_eq!(
        style("", "font-size: 20px; word-spacing: calc(0.5em + 50%)").word_spacing,
        20.0
    );
}

/** An inherited percentage re-resolves against the child's own font size; an inherited em does
 * not. */
#[test]
fn inherited_spacing_percent_follows_the_childs_font() {
    let child = style("font-size: 20px; word-spacing: 100%", "font-size: 10px");
    assert_eq!(child.word_spacing, 10.0);
    let child = style("font-size: 20px; word-spacing: 1em", "font-size: 10px");
    assert_eq!(child.word_spacing, 20.0);
    let child = style("font-size: 20px; letter-spacing: 10%", "font-size: 40px");
    assert_eq!(child.letter_spacing, 4.0);
}
