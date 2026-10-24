/** `clip: rect()` and `clip-path: inset()` as box insets the painter intersects with the border box. */
use super::{
    Extent,
    constants::PERCENT,
    layout_props::extent,
    syntax::{split_top, tokens},
    unsupported,
};
use crate::{document::RenderContext, protocol::EngineResult};
/**
 * `rect(top, right, bottom, left)`, commas optional as in the legacy syntax. Its edges are offsets
 * from the box's top-left corner, so the right and bottom ones become insets of `100% - edge`;
 * `auto` is the box's own edge.
 */
pub(super) fn rect(value: &str, font: f64, context: RenderContext) -> EngineResult<[Extent; 4]> {
    let inner = arguments(value, "rect(")?;
    let parts: Vec<&str> = if inner.contains(',') {
        split_top(inner, b',').into_iter().map(str::trim).collect()
    } else {
        tokens(inner)
    };
    let [top, right, bottom, left] = parts.as_slice() else {
        return Err(unsupported());
    };
    let edge = |part: &str| -> EngineResult<Option<Extent>> {
        if part == "auto" {
            Ok(None)
        } else {
            extent(part, font, context).map(Some)
        }
    };
    let from_far = |part: &str| -> EngineResult<Extent> {
        Ok(edge(part)?.map_or(Extent::default(), |edge| Extent {
            px: -edge.px,
            percent: PERCENT - edge.percent,
        }))
    };
    Ok([
        edge(top)?.unwrap_or_default(),
        from_far(right)?,
        from_far(bottom)?,
        edge(left)?.unwrap_or_default(),
    ])
}
/** `inset(t [r [b [l]]] [round radii])`, sides repeating like `margin`; the rounding is dropped. */
pub(super) fn inset(value: &str, font: f64, context: RenderContext) -> EngineResult<[Extent; 4]> {
    let inner = arguments(value, "inset(")?;
    let parts = tokens(inner);
    let sides = parts
        .iter()
        .take_while(|part| **part != "round")
        .map(|part| extent(part, font, context))
        .collect::<EngineResult<Vec<_>>>()?;
    Ok(match sides.as_slice() {
        [all] => [*all; 4],
        [vertical, horizontal] => [*vertical, *horizontal, *vertical, *horizontal],
        [top, horizontal, bottom] => [*top, *horizontal, *bottom, *horizontal],
        [top, right, bottom, left] => [*top, *right, *bottom, *left],
        _ => return Err(unsupported()),
    })
}
/** The text between `name(` and the matching final `)`; a trailing geometry box keyword is ignored. */
fn arguments<'v>(value: &'v str, name: &str) -> EngineResult<&'v str> {
    let rest = value.strip_prefix(name).ok_or_else(unsupported)?;
    let close = rest.rfind(')').ok_or_else(unsupported)?;
    let trailing = rest[close + 1..].trim();
    let boxes = ["", "border-box", "padding-box", "content-box", "margin-box"];
    if !boxes.contains(&trailing) {
        return Err(unsupported());
    }
    Ok(rest[..close].trim())
}
