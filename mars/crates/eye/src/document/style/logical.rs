/**
 * Flow-relative box properties (`margin-inline-start`, `inset-block`, `border-inline-end-color`,
 * `inline-size`, ...) map onto physical sides per element, after the cascade, from its direction
 * in horizontal writing: block start is the top, inline start the left (the right when `rtl`).
 * Logical and physical spellings of one side then compete by cascade priority, as in CSS.
 */
use super::cascade::{Chosen, choose};
use std::{cell::RefCell, collections::HashMap};
/** Writing modes `physical` is memoized under: one slot per (rtl, vertical, lr). */
const MODES: usize = 8;
/** Keys memoized before the memo starts over; custom properties could otherwise grow it. */
const PHYSICAL_MEMO: usize = 4096;
thread_local! {
    /** `physical` by key and mode: the cascade asks it for every chosen property of every
    element, and its substring searches cost more than the cascade around them. */
    static PHYSICAL: RefCell<HashMap<String, [Option<Option<String>>; MODES]>> =
        RefCell::new(HashMap::new());
}
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
/** The element's writing mode and direction, which decide where each flow-relative side is. */
#[derive(Clone, Copy, Default)]
pub(super) struct Mode {
    /// `direction: rtl`.
    pub rtl: bool,
    /// A vertical `writing-mode`.
    pub vertical: bool,
    /// `vertical-lr` or `sideways-lr`: block start is the left edge.
    pub lr: bool,
}
impl Mode {
    /** The physical (inline-start, inline-end, block-start, block-end) sides (CSS Logical 4):
    lines run left to right (or right to left) in horizontal writing, top to bottom (or bottom
    to top) in vertical writing, where blocks stack from the right (`rl`) or the left (`lr`). */
    fn sides(self) -> [&'static str; 4] {
        let (start, end) = match (self.vertical, self.rtl) {
            (false, false) => ("left", "right"),
            (false, true) => ("right", "left"),
            (true, false) => ("top", "bottom"),
            (true, true) => ("bottom", "top"),
        };
        let (before, after) = match (self.vertical, self.lr) {
            (false, _) => ("top", "bottom"),
            (true, false) => ("right", "left"),
            (true, true) => ("left", "right"),
        };
        [start, end, before, after]
    }
}
/** The physical longhand for a flow-relative one, or None when `key` is physical. */
pub(super) fn physical(key: &str, mode: Mode) -> Option<String> {
    let slot = usize::from(mode.rtl) | usize::from(mode.vertical) << 1 | usize::from(mode.lr) << 2;
    let found = PHYSICAL.with_borrow(|memo| memo.get(key).and_then(|modes| modes[slot].clone()));
    if let Some(found) = found {
        return found;
    }
    let found = map_physical(key, mode);
    PHYSICAL.with_borrow_mut(|memo| {
        if memo.len() >= PHYSICAL_MEMO {
            memo.clear();
        }
        memo.entry(key.to_owned()).or_default()[slot] = Some(found.clone());
    });
    found
}
/** `physical`, computed. */
fn map_physical(key: &str, mode: Mode) -> Option<String> {
    let sides = mode.sides();
    if let Some(corner) = corner(key, sides) {
        return Some(corner);
    }
    for (logical, side) in SIDES.into_iter().zip(sides) {
        if let Some(index) = key.find(logical) {
            let mapped = format!("{}{side}{}", &key[..index], &key[index + logical.len()..]);
            return Some(
                mapped
                    .strip_prefix("inset-")
                    .map_or(mapped.clone(), Into::into),
            );
        }
    }
    let (inline, block) = if mode.vertical {
        ("height", "width")
    } else {
        ("width", "height")
    };
    let size = key
        .replace("inline-size", inline)
        .replace("block-size", block);
    (size != key).then_some(size)
}
/** `border-{block}-{inline}-radius` corners, named vertical side first as CSS spells them. */
fn corner(key: &str, [start, end, before, after]: [&'static str; 4]) -> Option<String> {
    let inner = key.strip_prefix("border-")?.strip_suffix("-radius")?;
    let (block, inline) = inner.split_once('-')?;
    let pick = |which: &str, first: &'static str, last: &'static str| match which {
        "start" => Some(first),
        "end" => Some(last),
        _ => None,
    };
    let block = pick(block, before, after)?;
    let inline = pick(inline, start, end)?;
    let (vertical, horizontal) = if ["top", "bottom"].contains(&block) {
        (block, inline)
    } else {
        (inline, block)
    };
    Some(format!("border-{vertical}-{horizontal}-radius"))
}
/** Replaces every flow-relative winner with its physical side, keeping the stronger of the two. */
pub(super) fn resolve(chosen: &mut Chosen, mode: Mode) {
    let logical: Vec<String> = chosen
        .keys()
        .filter(|key| physical(key, mode).is_some())
        .cloned()
        .collect();
    for key in logical {
        if let (Some(name), Some((priority, value))) = (physical(&key, mode), chosen.remove(&key)) {
            choose(chosen, &name, value, priority);
        }
    }
}
/** The writing mode the element's own winning `writing-mode` declares, else the inherited one,
as (vertical, lr). */
pub(super) fn writing(chosen: &Chosen, inherited: (bool, bool)) -> (bool, bool) {
    match chosen
        .get("writing-mode")
        .map(|(_, value)| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("horizontal-tb" | "lr" | "lr-tb" | "rl" | "rl-tb") => (false, false),
        Some("vertical-rl" | "sideways-rl" | "tb" | "tb-rl") => (true, false),
        Some("vertical-lr" | "sideways-lr") => (true, true),
        _ => inherited,
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
