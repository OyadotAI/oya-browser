/**
 * Math functions in `<integer>` properties resolve to the nearest integer, halves toward positive
 * infinity, as CSS Values 4 and Blink's CSSMathFunctionValue::ComputeInteger do. Tailwind 4
 * writes `z-index: calc(-10)` for `-z-10`.
 */
use mars_eye::{compute_styles, parse};

/** The computed z-index of a positioned `#t` given `value`. */
fn z_index(value: &str) -> Option<i32> {
    let sheet = format!("#t {{ position: relative; z-index: {value} }}");
    let document = compute_styles(&parse("<p id=t>x</p>").unwrap(), &[sheet]).unwrap();
    let node = document.nodes.iter().find(|n| n.html_id == "t").unwrap();
    node.style.z_index
}

#[test]
fn a_calc_of_a_negative_number_is_that_integer() {
    assert_eq!(z_index("calc(-10)"), Some(-10));
}

#[test]
fn a_fractional_result_rounds_halves_up() {
    assert_eq!(z_index("calc(5 / 2)"), Some(3));
    assert_eq!(z_index("calc(-5 / 2)"), Some(-2));
}

#[test]
fn min_and_max_resolve_in_integer_properties() {
    assert_eq!(z_index("max(1, 4)"), Some(4));
}

#[test]
fn plain_integers_still_parse() {
    assert_eq!(z_index("4"), Some(4));
}
