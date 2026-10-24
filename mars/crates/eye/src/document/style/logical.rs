/**
 * Flow-relative box properties (`margin-inline-start`, `inset-block`, `border-inline-end-color`,
 * `inline-size`, ...) map onto physical sides per element, after the cascade, from its direction
 * in horizontal writing: block start is the top, inline start the left (the right when `rtl`).
 * Logical and physical spellings of one side then compete by cascade priority, as in CSS.
 */
use super::cascade::{Chosen, choose};
/** Flow-relative side segments, in the order key names are tried. */
const SIDES: [&str; 4] = ["inline-start", "inline-end", "block-start", "block-end"];
/** Box properties with flow-relative two-value shorthands (`margin-inline: start end`). */
const PAIRED: [&str; 6] = [
    "margin",
    "padding",
    "inset",
    "border-*-width",
    "border-*-style",
    "border-*-color",
];
/** The physical longhand for a flow-relative one, or None when `key` is physical. */
pub(super) fn physical(key: &str, rtl: bool) -> Option<String> {
    let (start, end) = if rtl {
        ("right", "left")
    } else {
        ("left", "right")
    };
    if let Some(corner) = corner(key, start, end) {
        return Some(corner);
    }
    for (logical, side) in SIDES.into_iter().zip([start, end, "top", "bottom"]) {
        if let Some(index) = key.find(logical) {
            let mapped = format!("{}{side}{}", &key[..index], &key[index + logical.len()..]);
            return Some(
                mapped
                    .strip_prefix("inset-")
                    .map_or(mapped.clone(), Into::into),
            );
        }
    }
    let size = key
        .replace("inline-size", "width")
        .replace("block-size", "height");
    (size != key).then_some(size)
}
/** `border-{block}-{inline}-radius` corners. */
fn corner(key: &str, start: &str, end: &str) -> Option<String> {
    let inner = key.strip_prefix("border-")?.strip_suffix("-radius")?;
    let (block, inline) = inner.split_once('-')?;
    let vertical = [("start", "top"), ("end", "bottom")]
        .into_iter()
        .find(|(k, _)| *k == block)?
        .1;
    let horizontal = [("start", start), ("end", end)]
        .into_iter()
        .find(|(k, _)| *k == inline)?
        .1;
    Some(format!("border-{vertical}-{horizontal}-radius"))
}
/** Replaces every flow-relative winner with its physical side, keeping the stronger of the two. */
pub(super) fn resolve(chosen: &mut Chosen, rtl: bool) {
    let logical: Vec<String> = chosen
        .keys()
        .filter(|key| physical(key, rtl).is_some())
        .cloned()
        .collect();
    for key in logical {
        if let (Some(name), Some((priority, value))) = (physical(&key, rtl), chosen.remove(&key)) {
            choose(chosen, &name, value, priority);
        }
    }
}
/** The direction the element's own winning `direction` declares, else the inherited one. */
pub(super) fn rtl(chosen: &Chosen, inherited: bool) -> bool {
    match chosen
        .get("direction")
        .map(|(_, value)| value.trim().to_ascii_lowercase())
    {
        Some(value) if value == "rtl" => true,
        Some(value) if value == "ltr" => false,
        _ => inherited,
    }
}
/** Longhands a flow-relative shorthand sets; None when `key` is not one. */
pub(super) fn longhands(key: &str) -> Option<Vec<String>> {
    if let Some(axis) = ["border-inline", "border-block"]
        .into_iter()
        .find(|a| *a == key)
    {
        let parts = ["width", "style", "color"];
        return Some(
            ["start", "end"]
                .iter()
                .flat_map(|e| parts.map(|p| format!("{axis}-{e}-{p}")))
                .collect(),
        );
    }
    if let Some(side) = ["inline-start", "inline-end", "block-start", "block-end"]
        .into_iter()
        .find(|side| key == format!("border-{side}"))
    {
        return Some(
            ["width", "style", "color"]
                .map(|p| format!("border-{side}-{p}"))
                .into(),
        );
    }
    let (base, axis) = paired(key)?;
    Some(
        ["start", "end"]
            .map(|end| base.replace('*', &format!("{axis}-{end}")))
            .into(),
    )
}
/** A two-value shorthand's property pattern and axis: `margin-inline` is ("margin-*", "inline"). */
fn paired(key: &str) -> Option<(String, &'static str)> {
    ["inline", "block"].into_iter().find_map(|axis| {
        PAIRED.iter().find_map(|base| {
            let pattern = if base.contains('*') {
                base.to_string()
            } else {
                format!("{base}-*")
            };
            (pattern.replace('*', axis) == key).then_some((pattern, axis))
        })
    })
}
/** Expands a flow-relative shorthand; `border-inline` takes `border` values, the rest one or two values. */
pub(super) fn expand(key: &str, value: &str) -> Option<Option<Vec<(String, String)>>> {
    let names = longhands(key)?;
    if names.len() != 2 {
        let parts = super::shorthand::border_side(value);
        return Some(parts.map(|parts| names.into_iter().zip(parts.into_iter().cycle()).collect()));
    }
    let values = super::syntax::tokens(value);
    let pair = match values.as_slice() {
        [both] => Some([both.to_string(), both.to_string()]),
        [start, end] => Some([start.to_string(), end.to_string()]),
        _ => None,
    };
    Some(pair.map(|pair| names.into_iter().zip(pair).collect()))
}
