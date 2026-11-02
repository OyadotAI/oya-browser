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
/** `medium`, the root's keyword: index 4 of Blink's 1-based keyword sizes. */
pub(super) const MEDIUM_KEYWORD: u8 = 4;
/** Blink's strict-mode keyword sizes for a 16px proportional default and a 13px fixed default
(font_size_functions.cc), xx-small to xxx-large. */
const KEYWORD_SIZES: [[f64; 8]; 2] = [
    [9.0, 10.0, 13.0, 16.0, 18.0, 24.0, 32.0, 48.0],
    [9.0, 10.0, 12.0, 13.0, 16.0, 20.0, 26.0, 39.0],
];
/** The 1-based keyword a `font-size` value names, if it names one. */
pub(super) fn keyword(value: &str) -> Option<u8> {
    let at = SIZE_KEYWORDS[..8].iter().position(|name| *name == value)?;
    Some(at as u8 + 1)
}
/** A keyword's size in px for a proportional or a monospace face. */
pub(super) fn keyword_size(keyword: u8, monospace: bool) -> f64 {
    KEYWORD_SIZES[usize::from(monospace)][usize::from(keyword.clamp(1, 8)) - 1]
}
/** Blink's IsMonospace: the family list is the generic `monospace` alone. */
pub(super) fn monospace(families: &[String]) -> bool {
    families.len() == 1 && families[0] == "monospace"
}
/** Blink's CheckForGenericFamilyChange: a size that is not absolute follows the switch between
`monospace` and any other family, refetched from the table for a keyword and scaled by 13/16
otherwise, so `<pre>` text is 13px and `font-family: serif` inside it is 16px again. */
pub(super) fn generic_change(style: &mut super::ComputedStyle, parent: &super::ComputedStyle) {
    let mono = monospace(&style.font_families);
    if style.font_size_absolute || mono == monospace(&parent.font_families) {
        return;
    }
    let old = style.font_size;
    let fixed = KEYWORD_SIZES[1][3] / KEYWORD_SIZES[0][3];
    style.font_size = match (style.font_keyword, mono) {
        (0, true) => old * fixed,
        (0, false) => old / fixed,
        (keyword, mono) => keyword_size(keyword, mono),
    };
    super::visual::respace(style, old);
}
