/**
 * `attr()` in property values (CSS Values 5): the element's attribute substituted before the
 * value computes, typed by `type(<syntax>)`, a unit or `raw-string`, else as a string. A value
 * the substitution leaves invalid takes the fallback, or without one becomes `unset` (invalid at
 * computed-value time). `content` reads its own `attr()` (content.rs).
 */
use super::cascade::Chosen;
use super::syntax::find_top;
use std::collections::BTreeMap;

/// Substituted bytes one value may grow to; past it the declaration is invalid.
const ATTR_OUTPUT_BYTES: usize = 4096;

/** Substitutes `attr()` in every winning declaration but `content`, expanding shorthands. */
pub(super) fn substitute(chosen: &mut Chosen, attributes: &BTreeMap<String, String>) {
    let pending: Vec<String> = chosen
        .iter()
        .filter(|(key, (_, value))| key.as_str() != "content" && uses_attr(value))
        .map(|(key, _)| key.clone())
        .collect();
    for key in pending {
        let Some((priority, value)) = chosen.remove(&key) else {
            continue;
        };
        for (name, value) in resolved(&key, &value, attributes) {
            if chosen
                .get(&name)
                .is_none_or(|(existing, _)| priority >= *existing)
            {
                chosen.insert(name, (priority, value.into()));
            }
        }
    }
}

/** Whether a value references `attr()` and no `var()` (which substitutes first, later). */
fn uses_attr(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("attr(") && !lower.contains("var(")
}

/** The longhands `key: value` stands for once its `attr()`s are substituted and validated,
 * each `unset` when neither the attribute nor a fallback gives a valid value. */
fn resolved(
    key: &str,
    value: &str,
    attributes: &BTreeMap<String, String>,
) -> Vec<(String, String)> {
    let valid = |attributes: Option<&BTreeMap<String, String>>| {
        expanded(value, attributes).and_then(|value| super::declaration::validate(key, &value).ok())
    };
    valid(Some(attributes))
        .or_else(|| valid(None))
        .unwrap_or_else(|| {
            let longhands = super::shorthand::longhands(key).unwrap_or_else(|| vec![key.into()]);
            longhands
                .into_iter()
                .map(|name| (name, "unset".into()))
                .collect()
        })
}

/** `value` with each `attr()` replaced, by the attribute when `attributes` are given and else by
 * the fallback (the retry once the attribute made the value invalid); None when one has
 * neither. */
fn expanded(value: &str, attributes: Option<&BTreeMap<String, String>>) -> Option<String> {
    let mut out = String::new();
    let mut rest = value;
    while let Some(at) = rest.to_ascii_lowercase().find("attr(") {
        out.push_str(&rest[..at]);
        let inner_start = at + "attr(".len();
        let close = find_top(rest, inner_start, b")");
        out.push_str(&reference(rest.get(inner_start..close)?, attributes)?);
        rest = rest.get(close + 1..).unwrap_or("");
        if out.len() > ATTR_OUTPUT_BYTES {
            return None;
        }
    }
    out.push_str(rest);
    Some(out)
}

/** `name [type] [, fallback]`: the attribute as its type asks, else the fallback. */
fn reference(inner: &str, attributes: Option<&BTreeMap<String, String>>) -> Option<String> {
    let comma = find_top(inner, 0, b",");
    let fallback = inner.get(comma + 1..).map(str::trim);
    let mut words = inner[..comma].trim().splitn(2, char::is_whitespace);
    let name = words.next()?.to_ascii_lowercase();
    let kind = words.next().unwrap_or("").trim();
    match attributes.and_then(|attributes| attributes.get(&name)) {
        Some(raw) => typed(raw.trim(), kind).or_else(|| fallback.map(str::to_string)),
        None => fallback.map(str::to_string),
    }
}

/** Whether `raw` parses as the `type(<syntax>)` asks, for the types whose values a shorthand
 * could otherwise misread (a bad color taken for a position); other syntaxes pass as written. */
fn matches_type(raw: &str, kind: &str) -> bool {
    use crate::document::{Appearance, RenderContext};
    let font = crate::constants::CSS_ROOT_FONT_SIZE;
    match kind
        .trim_end_matches(')')
        .trim_start_matches("type(")
        .trim()
    {
        "<color>" => super::values::contextual_color(raw, Appearance::Light, None).is_ok(),
        "<length>" => super::values::dimension(raw, font, RenderContext::default())
            .is_ok_and(|length| length.percent == 0.0),
        "<number>" => raw.trim().parse::<f64>().is_ok_and(f64::is_finite),
        _ => true,
    }
}

/** An attribute's text as `kind` makes it a value: `type()` and `raw-string` keep it as
 * written, a unit follows a number, and no type makes a string. */
fn typed(raw: &str, kind: &str) -> Option<String> {
    let lower = kind.to_ascii_lowercase();
    if lower.starts_with("type(") {
        return matches_type(raw, &lower).then(|| raw.to_string());
    }
    if lower == "raw-string" {
        return Some(raw.to_string());
    }
    if lower.is_empty() {
        return Some(format!(
            "\"{}\"",
            raw.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    let number = raw.parse::<f64>().ok().filter(|n| n.is_finite())?;
    Some(format!("{number}{lower}"))
}
