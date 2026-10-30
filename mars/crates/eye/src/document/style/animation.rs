/**
 * `@keyframes` parsing, live opacity/rotation tracks, and legacy static fallback for other properties.
 * Unsupported interpolation still contributes the frame seen once an animation has played: the final keyframe
 * under a forwards fill (direction and iteration parity decide which end that is), the base style
 * after a finite animation without one, and for an infinite animation its starting keyframe, or
 * its other end when the start hides, or else the base style. Keyframes Eye could not evaluate
 * (under an unknown condition, in a skipped sheet, past the bounds) never reveal anything; when
 * they could hide, the element stays out of agent reach.
 */
mod collect;
mod model;
mod rotation;
mod sample;
mod timeline;
use super::{
    cascade::{Chosen, Priority, choose},
    constants::{ANIMATIONS, KEYFRAME_SETS, KEYFRAME_STEPS, PERCENT, REVEAL_SECONDS},
    declaration::{self, Declaration},
    hiding::hiding,
    note,
    syntax::{block_end, find_top, split_top, tokens},
};
pub(super) use collect::{tracks, valid_value};
pub use model::{AnimationDirection, AnimationFill, OpacityAnimation, OpacityKeyframe};
pub(super) use rotation::tracks as rotation_tracks;
pub use rotation::{RotationAnimation, RotationAnimations, RotationKeyframe};
use std::collections::BTreeMap;
pub use timeline::OpacityAnimations;
/** Animated values override every normal declaration and lose to `!important` ones, as in CSS. */
const ANIMATED: Priority = (false, true, true, usize::MAX, usize::MAX, usize::MAX);
/** Longhands the `animation` shorthand sets, each a comma-separated list. */
pub(super) const LONGHANDS: [&str; 8] = [
    "animation-name",
    "animation-duration",
    "animation-timing-function",
    "animation-delay",
    "animation-iteration-count",
    "animation-direction",
    "animation-fill-mode",
    "animation-play-state",
];
/** Keyword classes of one shorthand layer, in LONGHANDS order after the name and the two times. */
const DIRECTIONS: [&str; 4] = ["normal", "reverse", "alternate", "alternate-reverse"];
/** Fill modes. */
const FILLS: [&str; 4] = ["none", "forwards", "backwards", "both"];
/** Play states. */
const STATES: [&str; 2] = ["running", "paused"];
/** Named easing keywords; `cubic-bezier()` and `steps()` are recognized by name. */
const EASINGS: [&str; 7] = [
    "linear",
    "ease",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "step-start",
    "step-end",
];
/** One keyframe: its offset in 0..=1 and its declarations, `!important` ones dropped. */
#[derive(Clone, Debug)]
struct Frame {
    /// Offset along one iteration.
    offset: f64,
    /// Validated declarations in source order.
    declarations: Vec<Declaration>,
    /// A declaration Eye dropped here could have hidden the element.
    hides: bool,
}
/** One `@keyframes` rule. */
#[derive(Clone, Debug, Default)]
struct Set {
    /// Frames in source order; several may share an offset.
    frames: Vec<Frame>,
    /// Under an undecidable condition or a skipped sheet: never applied, only its hiding counts.
    masked: bool,
    /// Some declaration, kept or dropped, could hide.
    hiding: bool,
}
/** Every `@keyframes` rule of a cascade by name; a later rule replaces an earlier one, as in CSS. */
#[derive(Clone, Debug, Default)]
pub(super) struct Keyframes {
    /// Sets by case-sensitive name.
    sets: BTreeMap<String, Set>,
    /// Some rule fell past KEYFRAME_SETS, so an unknown name may be one Eye dropped.
    overflow: bool,
}
impl Keyframes {
    /** Records one `@keyframes` rule; `masked` when its condition or sheet was not evaluated. */
    pub(super) fn parse(
        &mut self,
        prelude: &str,
        body: &str,
        masked: bool,
        diagnostics: &mut Vec<String>,
    ) {
        let Some(name) = keyframes_name(prelude) else {
            return note(
                diagnostics,
                "An @keyframes rule without a name was ignored.".into(),
            );
        };
        if self.sets.len() >= KEYFRAME_SETS && !self.sets.contains_key(&name) {
            self.overflow = true;
            return note(
                diagnostics,
                format!("@keyframes rules after the first {KEYFRAME_SETS} were ignored."),
            );
        }
        let mut set = frames(body, diagnostics);
        set.masked = masked;
        self.sets.insert(name, set);
    }
}
/** The name after `@keyframes`, unquoted; None when missing or not one token. */
fn keyframes_name(prelude: &str) -> Option<String> {
    let rest = prelude.split_once(char::is_whitespace)?.1.trim();
    let name = unquote(rest);
    (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| name.to_string())
}
/** A quoted string's contents, or the text itself. */
fn unquote(text: &str) -> &str {
    ['"', '\'']
        .iter()
        .find_map(|q| text.strip_prefix(*q).and_then(|t| t.strip_suffix(*q)))
        .unwrap_or(text)
}
/** Every `selectors { declarations }` block of a keyframes body, within KEYFRAME_STEPS frames. */
fn frames(body: &str, diagnostics: &mut Vec<String>) -> Set {
    let mut set = Set::default();
    let mut index = 0;
    while index < body.len() {
        let open = find_top(body, index, b"{");
        let close = block_end(body, open);
        let selectors = body.get(index..open).unwrap_or("");
        let block = declaration::block(body.get(open + 1..close).unwrap_or(""), diagnostics);
        index = close + 1;
        set.hiding |= block.hiding;
        for offset in split_top(selectors, b',').into_iter().filter_map(offset) {
            if set.frames.len() >= KEYFRAME_STEPS {
                // A cut animation is not the one the page wrote: it applies nothing, only masks.
                set.masked = true;
                continue;
            }
            set.frames.push(Frame {
                offset,
                declarations: animatable(&block.declarations),
                hides: block.hides,
            });
        }
    }
    set
}
/** `from`, `to` or a percentage in 0..=100; timeline range names are not supported. */
fn offset(selector: &str) -> Option<f64> {
    match selector.trim().to_ascii_lowercase().as_str() {
        "from" => Some(0.0),
        "to" => Some(1.0),
        other => {
            let percent: f64 = other.strip_suffix('%')?.trim().parse().ok()?;
            (0.0..=PERCENT)
                .contains(&percent)
                .then_some(percent / PERCENT)
        }
    }
}
/** Keyframes ignore `!important` declarations and animation properties themselves. */
fn animatable(declarations: &[Declaration]) -> Vec<Declaration> {
    declarations
        .iter()
        .filter(|d| {
            !d.important
                && (!d.name.starts_with("animation") || d.name == "animation-timing-function")
        })
        .cloned()
        .collect()
}
/** Expands the `animation` shorthand's layers into the comma-separated longhand lists. */
pub(super) fn shorthand(raw: &str) -> Option<Vec<(String, String)>> {
    let layers = super::transition::parts(raw)?;
    if layers.is_empty() || layers.len() > ANIMATIONS {
        return None;
    }
    let mut lists: [Vec<String>; 8] = Default::default();
    for layer in layers {
        for (list, value) in lists.iter_mut().zip(layer_values(layer)?) {
            list.push(value);
        }
    }
    Some(
        LONGHANDS
            .iter()
            .zip(lists)
            .map(|(name, values)| (name.to_string(), values.join(", ")))
            .collect(),
    )
}
/** One layer's values in LONGHANDS order; a keyword fills its own slot first, as CSS resolves names. */
fn layer_values(layer: &str) -> Option<[String; 8]> {
    let mut slots: [Option<String>; 8] = Default::default();
    for token in tokens(layer) {
        let slot = slot(&token.to_ascii_lowercase(), &slots)?;
        slots[slot] = Some(token.to_string());
    }
    let defaults = ["none", "0s", "ease", "0s", "1", "normal", "none", "running"];
    Some(std::array::from_fn(|i| {
        slots[i].take().unwrap_or_else(|| defaults[i].into())
    }))
}
/** The first free LONGHANDS slot `token` can fill; None when nothing can take it. */
fn slot(token: &str, slots: &[Option<String>; 8]) -> Option<usize> {
    let free = |index: usize| slots[index].is_none().then_some(index);
    let time = token.ends_with('s') && token.trim_end_matches(['m', 's']).parse::<f64>().is_ok();
    let easing = EASINGS.contains(&token)
        || token.starts_with("cubic-bezier(")
        || token.starts_with("steps(");
    if time {
        free(1).or_else(|| free(3))
    } else if easing {
        free(2)
    } else if token == "infinite" || token.parse::<f64>().is_ok() {
        free(4)
    } else if DIRECTIONS.contains(&token) {
        free(5)
    } else if FILLS.contains(&token) {
        free(6).or_else(|| free(0))
    } else if STATES.contains(&token) {
        free(7)
    } else {
        free(0)
    }
}
/**
 * Replaces the element's animation longhands with the declarations of the frame it rests on.
 * True when an animation Eye could not evaluate, or a frame it applied, could have hidden it.
 */
pub(super) fn animate(
    chosen: &mut Chosen,
    keyframes: &Keyframes,
    diagnostics: &mut Vec<String>,
) -> bool {
    let animation: BTreeMap<String, String> = LONGHANDS
        .iter()
        .filter_map(|key| {
            chosen
                .remove(*key)
                .map(|(_, v)| (key.to_string(), v.to_string()))
        })
        .collect();
    chosen.retain(|key, _| !key.starts_with("animation"));
    let Some(names) = animation.get("animation-name") else {
        return false;
    };
    let mut hides = false;
    for (index, name) in split_top(names, b',')
        .into_iter()
        .take(ANIMATIONS)
        .enumerate()
    {
        let pick = |key: &str| pick(&animation, key, index);
        let (hides_here, revealed) = run(chosen, keyframes, unquote(name), &pick);
        hides |= hides_here;
        if revealed && !settles(&pick) {
            note(
                diagnostics,
                "An animation reveals content only after a pause, a long delay or a loop; agents cannot act on it.".into(),
            );
            hides = true;
        }
    }
    hides
}
/**
 * Agent safety: a resting frame that reveals what the base style hid is only trusted when a
 * person sees it soon and for good: running, kept by a forwards fill, finite, and done within
 * REVEAL_SECONDS. Otherwise the reveal still paints for people but withdraws agent actions.
 */
fn settles(pick: &dyn Fn(&str) -> String) -> bool {
    let fill = pick("animation-fill-mode");
    let count = match pick("animation-iteration-count").as_str() {
        "" => Some(1.0),
        count => count.parse::<f64>().ok(),
    };
    let total = count
        .zip(seconds(&pick("animation-duration")))
        .zip(seconds(&pick("animation-delay")))
        .map(|((count, duration), delay)| delay + duration * count);
    pick("animation-play-state") != "paused"
        && ["forwards", "both"].contains(&fill.as_str())
        && total.is_some_and(|total| total.is_finite() && total <= REVEAL_SECONDS)
}
/** A CSS time in seconds; empty is the initial `0s`. */
fn seconds(value: &str) -> Option<f64> {
    if value.is_empty() {
        return Some(0.0);
    }
    if let Some(ms) = value.strip_suffix("ms") {
        return ms.trim().parse::<f64>().ok().map(|ms| ms / 1000.0);
    }
    value.strip_suffix('s')?.trim().parse().ok()
}
/** The `index`th value of one longhand list, repeating the list as CSS does; lowercase. */
fn pick(animation: &BTreeMap<String, String>, key: &str, index: usize) -> String {
    let list = animation
        .get(key)
        .map(|v| split_top(v, b','))
        .unwrap_or_default();
    match list.len() {
        0 => String::new(),
        len => {
            let value = list[index % len].to_ascii_lowercase();
            if super::declaration::wide(&value) {
                String::new()
            } else {
                value
            }
        }
    }
}
/**
 * Applies one named animation's resting frame: whether it could hide the element, and whether
 * it revealed something the style beneath it hid.
 */
fn run(
    chosen: &mut Chosen,
    keyframes: &Keyframes,
    name: &str,
    pick: &dyn Fn(&str) -> String,
) -> (bool, bool) {
    if name.eq_ignore_ascii_case("none") {
        return (false, false);
    }
    let Some(set) = keyframes.sets.get(name) else {
        return (keyframes.overflow, false);
    };
    if set.masked {
        return (set.hiding, false);
    }
    // ponytail: `animation-play-state: paused` rests like a running animation, since scroll
    // reveal scripts pause until the box is in view; model the paused time if pages need it.
    let Some(progress) = resting(set, pick) else {
        return (false, false);
    };
    let (mut hides, mut revealed) = (false, false);
    for frame in set.frames.iter().filter(|frame| frame.offset == progress) {
        hides |= frame.hides;
        for declaration in &frame.declarations {
            if declaration.name.starts_with("animation") {
                continue;
            }
            revealed |= reveals(chosen, declaration);
            choose(
                chosen,
                &declaration.name,
                declaration.value.clone(),
                ANIMATED,
            );
        }
    }
    (hides, revealed)
}
/**
 * Whether this frame value would un-hide the element: the value beneath it could hide and this
 * one could not. An inherited visibility is unknown here, so setting it counts as a reveal.
 */
fn reveals(chosen: &Chosen, declaration: &Declaration) -> bool {
    let name = declaration.name.as_str();
    if hiding(name, &declaration.value) {
        return false;
    }
    match chosen.get(name) {
        Some((priority, _)) if *priority > ANIMATED => false,
        Some((_, base)) => hiding(name, base),
        None => name == "visibility",
    }
}
/** The offset the animation rests on, or None for the base style. */
fn resting(set: &Set, pick: &dyn Fn(&str) -> String) -> Option<f64> {
    let direction = pick("animation-direction");
    let count = pick("animation-iteration-count");
    if count == "infinite" {
        let start = if direction.contains("reverse") {
            1.0
        } else {
            0.0
        };
        return [start, 1.0 - start]
            .into_iter()
            .find(|offset| !frame_hides(set, *offset));
    }
    let fill = pick("animation-fill-mode");
    if !["forwards", "both"].contains(&fill.as_str()) {
        return None;
    }
    let count = count
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .unwrap_or(1.0);
    Some(final_offset(count, &direction))
}
/**
 * Where the last iteration ends: its direction alternates with parity under `alternate`; a
 * fractional count stops part way, snapped to the nearer end since Eye does not interpolate.
 */
fn final_offset(count: f64, direction: &str) -> f64 {
    let fraction = count.fract();
    let (iteration, progress) = if count == 0.0 {
        (0.0, 0.0)
    } else if fraction == 0.0 {
        (count - 1.0, 1.0)
    } else {
        (count.floor(), fraction)
    };
    let odd = iteration % 2.0 == 1.0;
    let backward = match direction {
        "reverse" => true,
        "alternate" => odd,
        "alternate-reverse" => !odd,
        _ => false,
    };
    let offset = if backward { 1.0 - progress } else { progress };
    offset.round()
}
/** A frame a person could not see: zero opacity, hidden visibility, or a dropped hiding value. */
fn frame_hides(set: &Set, offset: f64) -> bool {
    set.frames
        .iter()
        .filter(|f| f.offset == offset)
        .any(|frame| {
            frame.hides
                || frame.declarations.iter().any(|d| {
                    ["opacity", "visibility", "display"].contains(&d.name.as_str())
                        && hiding(&d.name, &d.value)
                })
        })
}
