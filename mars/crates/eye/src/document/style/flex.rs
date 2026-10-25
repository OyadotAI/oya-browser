/** Supported flex longhands preserve definite pixels, percentages and automatic main sizes. */
use super::{
    AlignItems, BoxSizing, ComputedStyle, FlexBasis, FlexDirection, FlexWrap, JustifyContent,
    constants::ORDER_MAX, unsupported, values::number,
};
use crate::protocol::EngineResult;
/** New flex values are explicit; unrelated properties remain owned by the legacy declaration strategy. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: crate::document::RenderContext,
) -> EngineResult<bool> {
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
                "row" | "row-reverse" => FlexDirection::Row,
                "column" | "column-reverse" => FlexDirection::Column,
                _ => return Err(unsupported()),
            };
            style.flex_reverse = value.ends_with("-reverse");
        }
        "flex-wrap" => {
            style.flex_wrap = match value {
                "nowrap" => FlexWrap::NoWrap,
                "wrap" | "wrap-reverse" => FlexWrap::Wrap,
                _ => return Err(unsupported()),
            };
            style.wrap_reverse = value == "wrap-reverse";
        }
        // Shorthands no other module expands: each word goes to the longhand it names.
        "flex-flow" => {
            for word in value.split_whitespace() {
                let key = if word.contains("wrap") {
                    "flex-wrap"
                } else {
                    "flex-direction"
                };
                apply(style, key, word, context)?;
            }
        }
        "place-items" | "place-self" => {
            let (align_key, justify_key) = if key == "place-items" {
                ("align-items", "justify-items")
            } else {
                ("align-self", "justify-self")
            };
            let mut words = value.split_whitespace();
            let first = words.next().ok_or_else(unsupported)?;
            let second = words.next().unwrap_or(first);
            apply(style, align_key, first, context)?;
            if justify_key == "justify-items" {
                style.justify_items = if second == "legacy" {
                    AlignItems::Stretch
                } else {
                    align(second)?
                };
            } else {
                apply(style, justify_key, second, context)?;
            }
        }
        "place-content" => {
            let words = value.split_whitespace().collect::<Vec<_>>();
            let align = words.first().ok_or_else(unsupported)?;
            let justify = words.get(1).unwrap_or(align);
            style.align_content = content(align)?;
            style.justify_content = content(justify)?;
        }
        "flex-basis" => style.flex_basis = basis_in(value, style.font_size, context)?,
        "flex-grow" => style.flex_grow = number(value)?,
        "flex-shrink" => style.flex_shrink = number(value)?,
        "justify-content" => style.justify_content = content(value)?,
        "align-content" => style.align_content = content(value)?,
        "grid-auto-flow" => {
            let words = value.split_whitespace().collect::<Vec<_>>();
            if words.is_empty()
                || words
                    .iter()
                    .any(|w| !["row", "column", "dense"].contains(w))
            {
                return Err(unsupported());
            }
            style.grid_flow_column = words.contains(&"column");
            style.grid_flow_dense = words.contains(&"dense");
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
/** A `justify-content` or `align-content` value. An overflow keyword is accepted and ignored:
 * `unsafe` is what Blink does by default, and `safe` only differs once content overflows.
 * ponytail: `safe` overflow is treated as unsafe; keep the flag if overflowing centered
 * content shows up clipped on a real page. */
fn content(value: &str) -> EngineResult<JustifyContent> {
    let value = value
        .strip_prefix("unsafe ")
        .or_else(|| value.strip_prefix("safe "))
        .unwrap_or(value)
        .trim();
    Ok(match value {
        "normal" | "stretch" => JustifyContent::Stretch,
        "flex-start" | "baseline" | "first baseline" => JustifyContent::Start,
        "start" => JustifyContent::LogicalStart,
        "left" => JustifyContent::Left,
        "center" => JustifyContent::Center,
        "flex-end" | "last baseline" => JustifyContent::End,
        "end" => JustifyContent::LogicalEnd,
        "right" => JustifyContent::Right,
        "space-between" => JustifyContent::SpaceBetween,
        "space-around" => JustifyContent::SpaceAround,
        "space-evenly" => JustifyContent::SpaceEvenly,
        _ => return Err(unsupported()),
    })
}
/** Cross-axis keywords shared by `align-items` and the per-item `align-self` and `justify-self`. */
fn align(value: &str) -> EngineResult<AlignItems> {
    Ok(match value {
        "start" | "flex-start" | "self-start" | "left" => AlignItems::Start,
        "baseline" | "first baseline" => AlignItems::Baseline,
        "center" => AlignItems::Center,
        "end" | "flex-end" | "self-end" | "last baseline" | "right" => AlignItems::End,
        "stretch" | "normal" => AlignItems::Stretch,
        _ => return Err(unsupported()),
    })
}
/** Whether `value` is a valid `flex-basis`, judged with a default font: the `flex` shorthand
checks its tokens before the cascade has the element's font. */
pub(super) fn basis(value: &str) -> EngineResult<FlexBasis> {
    basis_in(
        value,
        crate::constants::CSS_ROOT_FONT_SIZE,
        crate::document::RenderContext::default(),
    )
}
/** A `flex-basis` with any length unit resolved to pixels, as Blink's `ConvertLengthOrAuto` does;
percentages stay unresolved until layout knows whether the main extent is definite. Without the
units, `flex: 0 0 4em` was dropped whole and the item kept its width. */
pub(super) fn basis_in(
    value: &str,
    font: f64,
    context: crate::document::RenderContext,
) -> EngineResult<FlexBasis> {
    // ponytail: `content` and the intrinsic keywords size from content like `auto` with an
    // automatic width; an authored width then wins where `content` would ignore it.
    if [
        "auto",
        "content",
        "max-content",
        "fit-content",
        "min-content",
    ]
    .contains(&value)
    {
        return Ok(FlexBasis::Auto);
    }
    if let Some(value) = value.strip_suffix('%') {
        return number(value).map(FlexBasis::Percent);
    }
    super::values::contextual_length(value, font, context).map(FlexBasis::Pixels)
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

/** A grid size the kept expression is test-evaluated at, so a malformed one is refused now. */
const TRACK_EXPR_PROBE: f64 = 1000.0;

thread_local! {
    /** Expressions of the grid template being parsed, collected by `track_expr_size`. */
    static TRACK_EXPRS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
/** Starts collecting the math-function tracks of one `grid-template-*` value. */
pub(super) fn begin_track_exprs() {
    TRACK_EXPRS.with(|exprs| exprs.borrow_mut().clear());
}
/** The math-function tracks collected since `begin_track_exprs`, in index order. */
pub(super) fn take_track_exprs() -> Vec<String> {
    TRACK_EXPRS.with(|exprs| std::mem::take(&mut *exprs.borrow_mut()))
}
/**
 * A track size that is a math function whose `min()`/`max()` compare percentages with lengths,
 * as `min(1128px, calc(100% - 48px))`. Only the grid's size can decide it, so every non-percent
 * unit is fixed to pixels now and the expression is kept for layout, as Blink keeps a
 * `CalculationValue` in the track's `Length`. None when the token is no such expression.
 */
pub(super) fn track_expr_size(
    token: &str,
    font: f64,
    context: crate::document::RenderContext,
) -> Option<super::TrackSize> {
    if !["calc(", "min(", "max(", "clamp("]
        .iter()
        .any(|function| token.starts_with(function))
    {
        return None;
    }
    let tokens = super::calc::tokens(token).ok()?;
    let mut text = String::new();
    for token in tokens {
        match token {
            super::calc::Token::Number(number) if number.ends_with('%') => text.push_str(&number),
            super::calc::Token::Number(number) => match super::values::signed_number(&number) {
                Ok(_) => text.push_str(&number),
                Err(_) => {
                    let px = super::values::unit(&number, font, context)
                        .ok()?
                        .pixels()
                        .ok()?;
                    text.push_str(&format!("{px}px"));
                }
            },
            super::calc::Token::Operator(op) => text.push_str(&format!(" {} ", op as char)),
            super::calc::Token::Open(name) => text.push_str(&format!("{name}(")),
            super::calc::Token::Close => text.push(')'),
            super::calc::Token::Comma => text.push(','),
            super::calc::Token::Word(word) => text.push_str(&format!(" {word} ")),
        }
    }
    resolve_track_expr(&text, TRACK_EXPR_PROBE)?;
    TRACK_EXPRS.with(|exprs| {
        let mut exprs = exprs.borrow_mut();
        let index = u16::try_from(exprs.len()).ok()?;
        exprs.push(text);
        Some(super::TrackSize::Expr(index))
    })
}
/** A kept track expression in pixels once the grid's size in that axis is `basis`. */
pub(super) fn resolve_track_expr(text: &str, basis: f64) -> Option<f64> {
    let tokens = super::calc::tokens(text)
        .ok()?
        .into_iter()
        .map(|token| match token {
            super::calc::Token::Number(number) if number.ends_with('%') => {
                let percent = super::values::signed_number(number.trim_end_matches('%')).ok()?;
                Some(super::calc::Token::Number(format!(
                    "{}px",
                    percent / 100.0 * basis
                )))
            }
            other => Some(other),
        })
        .collect::<Option<Vec<_>>>()?;
    let value = super::calc::evaluate(&tokens, 0.0, crate::document::RenderContext::default())
        .ok()?
        .pixels()
        .ok()?;
    value.is_finite().then_some(value)
}
