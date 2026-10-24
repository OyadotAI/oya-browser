/**
 * Agent-safety predicate: a declaration Eye drops might have hidden or moved its targets in a
 * real browser, so whatever its rule could match must not become agent-actionable.
 */
use super::constants::INVISIBLE_OPACITY;
/** Whether this declaration could hide, clip, shrink to nothing or move its element away. */
pub(super) fn hiding(name: &str, value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    let zero = || zero(&value);
    match name {
        "display" => value == "none",
        "visibility" => ["hidden", "collapse"].contains(&value.as_str()),
        "opacity" => opacity(&value).is_some_and(|alpha| alpha <= INVISIBLE_OPACITY),
        "content-visibility" => value == "hidden",
        "clip" | "clip-path" | "-webkit-clip-path" | "mask" | "mask-image" | "-webkit-mask"
        | "-webkit-mask-image" | "transform" | "-webkit-transform" | "translate" | "scale"
        | "rotate" => !["none", "auto"].contains(&value.as_str()),
        "position" => ["absolute", "fixed"].contains(&value.as_str()),
        "top" | "left" | "right" | "bottom" | "inset" | "inset-block" | "inset-inline"
        | "inset-block-start" | "inset-block-end" | "inset-inline-start" | "inset-inline-end" => {
            value != "auto" && !zero()
        }
        "text-indent"
        | "margin"
        | "margin-top"
        | "margin-left"
        | "margin-right"
        | "margin-bottom"
        | "margin-inline-start"
        | "margin-inline-end"
        | "margin-inline"
        | "margin-block-start"
        | "margin-block-end"
        | "margin-block" => negative(&value),
        "width" | "height" | "max-width" | "max-height" | "font-size" | "block-size"
        | "inline-size" | "max-block-size" | "max-inline-size" => zero(),
        "z-index" => value.starts_with('-'),
        // Which keyframes a name runs is only known per element, so any animation could hide.
        "animation" | "animation-name" => value != "none",
        "filter" => value.contains("opacity(0"),
        _ => false,
    }
}
/**
 * Some number in the value is negative: a `-` that starts a number, not one inside a custom
 * property name (`var(--x)`) or calc's spaced subtraction, which Tailwind writes on every page.
 */
fn negative(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.iter().enumerate().any(|(index, byte)| {
        let starts =
            index == 0 || !(bytes[index - 1].is_ascii_alphanumeric() || bytes[index - 1] == b'-');
        *byte == b'-'
            && starts
            && bytes
                .get(index + 1)
                .is_some_and(|next| next.is_ascii_digit() || *next == b'.')
    })
}
/** `0`, `0px`, `0%` and similar spell nothing. */
fn zero(value: &str) -> bool {
    let digits = value.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
    digits.parse::<f64>().is_ok_and(|number| number == 0.0)
}
/** Opacity as a fraction from a number or percentage. */
fn opacity(value: &str) -> Option<f64> {
    match value.strip_suffix('%') {
        Some(percent) => percent.trim().parse::<f64>().ok().map(|p| p / 100.0),
        None => value.parse().ok(),
    }
}
