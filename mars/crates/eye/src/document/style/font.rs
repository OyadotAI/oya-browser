/** The font shorthand resets its longhands before cascade selection, following Blink ConsumeFont. */
use super::{
    LineHeight, unsupported,
    values::{length, number},
};
use crate::protocol::EngineResult;
/** Size keywords the shorthand recognizes as its required size. */
const SIZE_KEYWORDS: [&str; 10] = [
    "xx-small",
    "x-small",
    "small",
    "medium",
    "large",
    "x-large",
    "xx-large",
    "xxx-large",
    "smaller",
    "larger",
];
/** Optional style, variant, weight and stretch precede the size, optional `/advance` and families. */
pub(super) fn shorthand(source: &str) -> EngineResult<Vec<(String, String)>> {
    if [
        "caption",
        "icon",
        "menu",
        "message-box",
        "small-caption",
        "status-bar",
    ]
    .contains(&source)
    {
        return Err(unsupported());
    }
    let separated = source.replacen('/', " / ", 1);
    let tokens = separated.split_whitespace().collect::<Vec<_>>();
    let size = tokens
        .iter()
        .position(|token| size_token(token))
        .ok_or_else(unsupported)?;
    let (mut weight, mut style) = ("normal", "normal");
    for token in &tokens[..size] {
        match *token {
            "normal" | "small-caps" | "condensed" | "expanded" | "semi-condensed"
            | "semi-expanded" => {}
            "italic" | "oblique" => style = token,
            "bold" | "bolder" | "lighter" => weight = token,
            _ if token.parse::<u16>().is_ok_and(|w| (1..=1000).contains(&w)) => weight = token,
            _ => return Err(unsupported()),
        }
    }
    let (advance, family_start) = if tokens.get(size + 1) == Some(&"/") {
        (*tokens.get(size + 2).ok_or_else(unsupported)?, size + 3)
    } else {
        ("normal", size + 1)
    };
    let family = tokens
        .get(family_start..)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(unsupported)?
        .join(" ");
    Ok(vec![
        ("font-weight".into(), weight.into()),
        ("font-style".into(), style.into()),
        ("font-size".into(), tokens[size].into()),
        ("line-height".into(), advance.into()),
        ("font-family".into(), family),
    ])
}
/** A length, percentage or size keyword. */
fn size_token(token: &str) -> bool {
    SIZE_KEYWORDS.contains(&token)
        || token.ends_with('%')
        || ["px", "em", "rem", "pt", "vw", "vh"].iter().any(|unit| {
            token.ends_with(unit) && token[..token.len() - unit.len()].parse::<f64>().is_ok()
        })
}
/** Percentages compute once; unitless factors remain relative and bounded through inheritance. */
pub(super) fn line_height(value: &str, font: f64) -> EngineResult<LineHeight> {
    let max_factor = crate::constants::CSS_MAX_LINE_HEIGHT_FACTOR;
    if value == "normal" {
        return Ok(LineHeight::Normal);
    }
    if let Some(percent) = value.strip_suffix('%') {
        let advance = font * number(percent)? / 100.0;
        return if advance > 0.0 {
            Ok(LineHeight::Length(advance))
        } else {
            Err(unsupported())
        };
    }
    if let Ok(factor) = number(value) {
        return if factor > 0.0 && factor <= max_factor {
            Ok(LineHeight::Number(factor))
        } else {
            Err(unsupported())
        };
    }
    let advance = length(value, font)?;
    if advance > 0.0 {
        Ok(LineHeight::Length(advance))
    } else {
        Err(unsupported())
    }
}
