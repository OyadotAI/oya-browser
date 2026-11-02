/** css-multicol properties: `column-count`, `column-width`, `columns`, `column-fill`,
`column-rule-*`, `column-rule` and `column-span`, with the css-break `break-*` properties
columns honor, computed into one `Multicol` value. */
use super::{
    BorderStyle, ComputedStyle,
    constants::{BORDER_MEDIUM, BORDER_THICK, BORDER_THIN},
    syntax::tokens,
    unsupported,
    values::{contextual_color, contextual_length},
};
use crate::{document::RenderContext, protocol::EngineResult};
use serde::{Deserialize, Serialize};

/** The smallest used `column-width`: Blink clamps a zero width to one pixel so the column
count formula never divides by zero. */
const MIN_COLUMN_WIDTH: f64 = 1.0;

/// A box's multi-column properties; the default is a box that is no multicol container.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Multicol {
    /// `column-count`, None for `auto`.
    pub count: Option<u32>,
    /// `column-width` in pixels, None for `auto`.
    pub width: Option<f64>,
    /// css-multicol-2 `column-height` in pixels, None for `auto`.
    pub height: Option<f64>,
    /// css-multicol-2 `column-wrap`: Some(true) for `wrap`, Some(false) for `nowrap`, None
    /// for `auto`, which wraps exactly when `column-height` is set.
    pub wrap: Option<bool>,
    /// `column-fill: auto`: columns fill one after another instead of balancing.
    pub fill_auto: bool,
    /// The rule's line style; None for `none` or `hidden`, which paint no rule.
    pub rule_style: Option<BorderStyle>,
    /// `column-rule-width` in pixels; None is the initial `medium`.
    pub rule_width: Option<f64>,
    /// `column-rule-color`; None is `currentcolor`, resolved when the rule paints.
    pub rule_color: Option<String>,
    /// `column-span: all`: the box spans every column of its multicol container.
    pub span_all: bool,
    /// `break-inside: avoid` (or `avoid-column`): the box stays in one column when it fits.
    pub avoid_inside: bool,
    /// `break-before: column` (or `page`, `always`, ...): a forced break before the box.
    pub break_before: bool,
    /// `break-after: column` (or `page`, `always`, ...): a forced break after the box.
    pub break_after: bool,
    /// `break-before: avoid` and `break-after: avoid` (any `avoid*`): no break at that edge.
    pub avoid_edges: [bool; 2],
    /// `contain: layout` (or `strict`, `content`) or a `filter`: the box contains its fixed and
    /// layout descendants, so an inner `column-span: all` box spans nothing.
    pub spanner_barrier: bool,
    /// `contain: size` (or `strict`): the box is monolithic, never broken across columns.
    pub contain_size: bool,
}
impl Multicol {
    /** Whether the box establishes a multi-column container (css-multicol-1 2: either
    `column-count` or `column-width` is not `auto`). */
    pub fn container(&self) -> bool {
        self.count.is_some() || self.width.is_some() || self.height.is_some()
    }
    /** Whether columns past the count wrap into a new row below (css-multicol-2). */
    pub fn wraps(&self) -> bool {
        self.wrap.unwrap_or(self.height.is_some())
    }
    /** The used rule width: zero when no rule paints, as a `none` border style zeroes a border. */
    pub fn used_rule_width(&self) -> f64 {
        self.rule_style
            .map_or(0.0, |_| self.rule_width.unwrap_or(BORDER_MEDIUM))
    }
}

/** Applies one multicol declaration, or reports that `key` belongs elsewhere. */
pub(super) fn apply(
    style: &mut ComputedStyle,
    key: &str,
    value: &str,
    context: RenderContext,
) -> EngineResult<bool> {
    match key {
        "column-count" => style.multicol.count = count(value)?,
        "column-width" => style.multicol.width = width(value, style.font_size, context)?,
        "column-height" => style.multicol.height = height(value, style.font_size, context)?,
        "column-wrap" => style.multicol.wrap = wrap(value)?,
        "column-fill" => style.multicol.fill_auto = fill(value)?,
        "column-span" => style.multicol.span_all = span(value)?,
        "column-rule-style" => style.multicol.rule_style = rule_style(value)?,
        "column-rule-width" => {
            style.multicol.rule_width = Some(rule_width(value, style.font_size, context)?)
        }
        "column-rule-color" => style.multicol.rule_color = rule_color(style, value)?,
        "break-inside" | "page-break-inside" | "-webkit-column-break-inside" => {
            style.multicol.avoid_inside = value.starts_with("avoid")
        }
        "break-before" | "page-break-before" | "-webkit-column-break-before" => {
            style.multicol.break_before = forced_break(value);
            style.multicol.avoid_edges[0] = value.starts_with("avoid");
        }
        "break-after" | "page-break-after" | "-webkit-column-break-after" => {
            style.multicol.break_after = forced_break(value);
            style.multicol.avoid_edges[1] = value.starts_with("avoid");
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/** Copies one multicol property's computed value for an explicit `inherit` (or another
CSS-wide keyword resolved to `source`); false when `key` is not one. */
pub(super) fn copy(style: &mut ComputedStyle, source: &ComputedStyle, key: &str) -> bool {
    let (to, from) = (&mut style.multicol, &source.multicol);
    match key {
        "column-count" => to.count = from.count,
        "column-width" => to.width = from.width,
        "column-height" => to.height = from.height,
        "column-wrap" => to.wrap = from.wrap,
        "column-fill" => to.fill_auto = from.fill_auto,
        "column-span" => to.span_all = from.span_all,
        "column-rule-style" => to.rule_style = from.rule_style,
        "column-rule-width" => to.rule_width = from.rule_width,
        "column-rule-color" => to.rule_color = from.rule_color.clone(),
        _ => return false,
    }
    true
}

/** Whether a `break-before`/`break-after` value forces a break in a multicol container. */
fn forced_break(value: &str) -> bool {
    matches!(
        value,
        "column" | "page" | "always" | "left" | "right" | "recto" | "verso"
    )
}

/** `column-count: auto | <integer [1,inf]>`. */
fn count(value: &str) -> EngineResult<Option<u32>> {
    if value == "auto" {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .ok()
        .filter(|count| *count > 0)
        .map(Some)
        .ok_or_else(unsupported)
}

/** `column-width: auto | <length [0,inf]>`; percentages are invalid. */
fn width(value: &str, font: f64, context: RenderContext) -> EngineResult<Option<f64>> {
    if value == "auto" {
        return Ok(None);
    }
    if value.contains('%') || value.starts_with('-') {
        return Err(unsupported());
    }
    let px = contextual_length(value, font, context)?;
    Ok(Some(px.max(MIN_COLUMN_WIDTH)))
}

/** `column-height: auto | <length [0,inf]>`. */
fn height(value: &str, font: f64, context: RenderContext) -> EngineResult<Option<f64>> {
    if value == "auto" {
        return Ok(None);
    }
    if value.contains('%') || value.starts_with('-') {
        return Err(unsupported());
    }
    contextual_length(value, font, context).map(Some)
}

/** `column-wrap: auto | nowrap | wrap`. */
fn wrap(value: &str) -> EngineResult<Option<bool>> {
    match value {
        "auto" => Ok(None),
        "wrap" => Ok(Some(true)),
        "nowrap" => Ok(Some(false)),
        _ => Err(unsupported()),
    }
}

/** The longhands of `columns` and `column-rule`, for CSS-wide keywords. */
pub(super) fn longhands(key: &str) -> Vec<String> {
    let names: &[&str] = match key {
        "columns" => &["column-width", "column-count", "column-height"],
        _ => &[
            "column-rule-width",
            "column-rule-style",
            "column-rule-color",
        ],
    };
    names.iter().map(|name| name.to_string()).collect()
}

/** `columns: [<'column-width'> || <'column-count'>] [/ <'column-height'>]?` as longhands, each
part reset to `auto` when omitted; a bare integer is the count, anything else the width. */
pub(super) fn expand_columns(value: &str) -> Option<Vec<(String, String)>> {
    let (inline, block) = value.split_once('/').unwrap_or((value, "auto"));
    let parts = tokens(inline.trim());
    let (mut count, mut width) = (None, None);
    if parts.is_empty() || parts.len() > 2 || block.trim().is_empty() {
        return None;
    }
    for part in parts.into_iter().filter(|part| *part != "auto") {
        let slot = if part.bytes().all(|byte| byte.is_ascii_digit()) {
            &mut count
        } else {
            &mut width
        };
        if slot.replace(part.to_string()).is_some() {
            return None;
        }
    }
    let auto = || "auto".to_string();
    Some(vec![
        ("column-width".into(), width.unwrap_or_else(auto)),
        ("column-count".into(), count.unwrap_or_else(auto)),
        ("column-height".into(), block.trim().to_string()),
    ])
}

/** `column-rule: <width> || <style> || <color>` as longhands, omitted parts at their initial
values; the longhands validate each part. */
pub(super) fn expand_rule(value: &str) -> Option<Vec<(String, String)>> {
    let (mut width, mut line, mut color) = (None, None, None);
    for part in tokens(value) {
        let slot = if rule_style(part).is_ok() {
            &mut line
        } else if is_width(part) {
            &mut width
        } else {
            &mut color
        };
        if slot.replace(part.to_string()).is_some() {
            return None;
        }
    }
    Some(vec![
        (
            "column-rule-width".into(),
            width.unwrap_or_else(|| "medium".into()),
        ),
        (
            "column-rule-style".into(),
            line.unwrap_or_else(|| "none".into()),
        ),
        (
            "column-rule-color".into(),
            color.unwrap_or_else(|| "currentcolor".into()),
        ),
    ])
}

/** Whether a `column-rule` part is its width: a width keyword, a number or a math function. */
fn is_width(part: &str) -> bool {
    ["thin", "medium", "thick"].contains(&part)
        || part.starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == '-' || c == '+')
        || ["calc(", "min(", "max(", "clamp("]
            .iter()
            .any(|function| part.starts_with(function))
}

/** `column-fill: auto | balance | balance-all`, true for `auto`. */
fn fill(value: &str) -> EngineResult<bool> {
    match value {
        "auto" => Ok(true),
        "balance" | "balance-all" => Ok(false),
        _ => Err(unsupported()),
    }
}

/** `column-span: none | all`. */
fn span(value: &str) -> EngineResult<bool> {
    match value {
        "all" => Ok(true),
        "none" => Ok(false),
        _ => Err(unsupported()),
    }
}

/** The border-style keywords a rule takes; `none` and `hidden` paint none. */
fn rule_style(value: &str) -> EngineResult<Option<BorderStyle>> {
    let style = match value {
        "none" | "hidden" => return Ok(None),
        "solid" => BorderStyle::Solid,
        "dotted" => BorderStyle::Dotted,
        "dashed" => BorderStyle::Dashed,
        "double" => BorderStyle::Double,
        "groove" => BorderStyle::Groove,
        "ridge" => BorderStyle::Ridge,
        "inset" => BorderStyle::Inset,
        "outset" => BorderStyle::Outset,
        _ => return Err(unsupported()),
    };
    Ok(Some(style))
}

/** `thin | medium | thick | <length [0,inf]>`, as border widths take. */
fn rule_width(value: &str, font: f64, context: RenderContext) -> EngineResult<f64> {
    match value {
        "thin" => Ok(BORDER_THIN),
        "medium" => Ok(BORDER_MEDIUM),
        "thick" => Ok(BORDER_THICK),
        _ if value.contains('%') || value.starts_with('-') => Err(unsupported()),
        _ => contextual_length(value, font, context),
    }
}

/** A rule color, None for `currentcolor` so the element's final color paints it. */
fn rule_color(style: &ComputedStyle, value: &str) -> EngineResult<Option<String>> {
    if value == "currentcolor" {
        return Ok(None);
    }
    contextual_color(value, style.used_scheme, style.color.as_deref())
}
