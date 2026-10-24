/** Supported flex longhands preserve definite pixels, percentages and automatic main sizes. */
use super::{
    AlignItems, BoxSizing, ComputedStyle, FlexBasis, FlexDirection, FlexWrap, JustifyContent,
    constants::ORDER_MAX, unsupported, values::number,
};
use crate::protocol::EngineResult;
/** New flex values are explicit; unrelated properties remain owned by the legacy declaration strategy. */
pub(super) fn apply(style: &mut ComputedStyle, key: &str, value: &str) -> EngineResult<bool> {
    match key {
        "box-sizing" => {
            style.box_sizing = match value {
                "content-box" => BoxSizing::ContentBox,
                "border-box" => BoxSizing::BorderBox,
                _ => return Err(unsupported()),
            }
        }
        "flex-direction" => {
            style.flex_direction = match value {
                // ponytail: reversed directions keep source order; reverse placement when it matters.
                "row" | "row-reverse" => FlexDirection::Row,
                "column" | "column-reverse" => FlexDirection::Column,
                _ => return Err(unsupported()),
            }
        }
        "flex-wrap" => {
            style.flex_wrap = match value {
                "nowrap" => FlexWrap::NoWrap,
                "wrap" | "wrap-reverse" => FlexWrap::Wrap,
                _ => return Err(unsupported()),
            }
        }
        "flex-basis" => style.flex_basis = basis(value)?,
        "flex-grow" => style.flex_grow = number(value)?,
        "flex-shrink" => style.flex_shrink = number(value)?,
        "justify-content" => {
            style.justify_content = match value {
                "start" | "flex-start" | "left" | "normal" | "stretch" => JustifyContent::Start,
                "center" => JustifyContent::Center,
                "end" | "flex-end" | "right" => JustifyContent::End,
                "space-between" => JustifyContent::SpaceBetween,
                "space-around" => JustifyContent::SpaceAround,
                "space-evenly" => JustifyContent::SpaceEvenly,
                _ => return Err(unsupported()),
            }
        }
        "align-items" => style.align_items = align(value)?,
        "align-self" | "justify-self" => {
            let own = (value != "auto").then(|| align(value)).transpose()?;
            if key == "align-self" {
                style.align_self = own;
            } else {
                style.justify_self = own;
            }
        }
        "order" => {
            let order: i64 = value.parse().map_err(|_| unsupported())?;
            style.order = order.clamp(-ORDER_MAX, ORDER_MAX) as i32;
        }
        "min-width" => {
            style.min_width = if value == "auto" {
                None
            } else {
                Some(pixels(value)?)
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}
/** Cross-axis keywords shared by `align-items` and the per-item `align-self` and `justify-self`. */
fn align(value: &str) -> EngineResult<AlignItems> {
    Ok(match value {
        "start" | "flex-start" | "self-start" | "baseline" | "first baseline" | "left" => {
            AlignItems::Start
        }
        "center" => AlignItems::Center,
        "end" | "flex-end" | "self-end" | "last baseline" | "right" => AlignItems::End,
        "stretch" | "normal" => AlignItems::Stretch,
        _ => return Err(unsupported()),
    })
}
/** Basis percentages remain unresolved until layout knows whether the main extent is definite. */
pub(super) fn basis(value: &str) -> EngineResult<FlexBasis> {
    if value == "auto" {
        return Ok(FlexBasis::Auto);
    }
    if let Some(value) = value.strip_suffix('%') {
        return number(value).map(FlexBasis::Percent);
    }
    pixels(value).map(FlexBasis::Pixels)
}
/** New structural lengths accept pixels or unitless zero; unsupported units cannot guess layout. */
pub(super) fn pixels(value: &str) -> EngineResult<f64> {
    if let Some(value) = value.strip_suffix("px") {
        return number(value);
    }
    match number(value)? {
        0.0 => Ok(0.0),
        _ => Err(unsupported()),
    }
}
