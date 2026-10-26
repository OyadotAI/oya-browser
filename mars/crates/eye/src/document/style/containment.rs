/**
 * Size containment (`contain: size`, `inline-size`, `strict`) and its `contain-intrinsic-size`
 * stand-ins (css-contain-2, css-sizing-4 "intrinsic size override"): a size-contained axis is
 * sized as if the box were empty, or as if it held one child of the `contain-intrinsic-*` size.
 */
use super::{ComputedStyle, unsupported, values::dimension};
use crate::{document::RenderContext, protocol::EngineResult};

/** Applies one longhand this module owns, or reports that `key` belongs elsewhere. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    let axis = match key {
        "contain-intrinsic-width" => 0,
        "contain-intrinsic-height" => 1,
        "contain" => {
            style.contain_size = contain(value);
            // `contain` is also read for paint and stacking, so it is never consumed here.
            return Ok(false);
        }
        _ => return Ok(false),
    };
    style.contain_intrinsic[axis] = intrinsic(value, style.font_size, context)?;
    Ok(true)
}

/** Which axes `contain` size-contains, inline (width) then block (height). */
fn contain(value: &str) -> [bool; 2] {
    let words: Vec<&str> = value.split_whitespace().collect();
    let both = words.iter().any(|word| ["size", "strict"].contains(word));
    [both || words.contains(&"inline-size"), both]
}

/**
 * One `contain-intrinsic-*` value: `none`, a non-negative length, or `auto <length>`. Eye keeps
 * no last remembered size, so `auto <length>` uses the length as a box that was never laid out.
 */
fn intrinsic(value: &str, font: f64, context: RenderContext) -> EngineResult<Option<f64>> {
    let value = value.trim();
    if value == "none" {
        return Ok(None);
    }
    let length = value.strip_prefix("auto ").map_or(value, str::trim);
    let parsed = dimension(length, font, context)?;
    if parsed.percent != 0.0 || parsed.px < 0.0 || !parsed.px.is_finite() {
        return Err(unsupported());
    }
    Ok(Some(parsed.px))
}

/**
 * The `contain-intrinsic-size` shorthand: one value for both axes or width then height, each
 * `none`, a length, or `auto` followed by one of those.
 */
pub(super) fn shorthand(value: &str) -> Option<Vec<(String, String)>> {
    let mut parts: Vec<String> = Vec::new();
    let mut pending_auto = false;
    for word in value.split_whitespace() {
        if word == "auto" && !pending_auto {
            pending_auto = true;
            continue;
        }
        parts.push(if pending_auto {
            format!("auto {word}")
        } else {
            word.to_string()
        });
        pending_auto = false;
    }
    if pending_auto || parts.is_empty() || parts.len() > 2 {
        return None;
    }
    let height = parts.get(1).cloned().unwrap_or_else(|| parts[0].clone());
    Some(vec![
        ("contain-intrinsic-width".into(), parts[0].clone()),
        ("contain-intrinsic-height".into(), height),
    ])
}

/** The shorthand's longhands, for CSS-wide keywords. */
pub(super) const LONGHANDS: [&str; 2] = ["contain-intrinsic-width", "contain-intrinsic-height"];
