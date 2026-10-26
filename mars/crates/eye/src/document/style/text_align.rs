/** `text-align-last` and `text-justify` (CSS Text 3), parsed to the keywords Blink's
 * ComputedStyle keeps; line layout resolves them per line. */
use crate::protocol::EngineResult;

/** `text-align-last`: empty for `auto`, else the keyword; `match-parent` takes the inherited
 * value with `start` and `end` resolved against the parent's direction. */
pub(super) fn last(value: &str, style: &super::ComputedStyle) -> EngineResult<String> {
    Ok(match value {
        "auto" => "",
        "left" | "center" | "right" | "start" | "end" | "justify" => value,
        "match-parent" => matched(&style.align_last, style.parent_rtl),
        _ => return Err(super::unsupported()),
    }
    .into())
}

/** CSS Text's `match-parent`: the parent's alignment `inherited`, except that `start` and `end`
 * become the physical side they name in the parent's direction, so a child of another
 * direction still aligns where its parent does. */
pub(super) fn matched(inherited: &str, parent_rtl: Option<bool>) -> &str {
    match (inherited, parent_rtl) {
        ("start", Some(false)) | ("end", Some(true)) => "left",
        ("start", Some(true)) | ("end", Some(false)) => "right",
        (other, _) => other,
    }
}

/** `text-autospace`: whether ideograph-alpha spacing applies. `no-autospace` turns it off; the
 * other keywords (`normal`, `auto`, `ideograph-alpha`, `ideograph-numeric`, `punctuation`,
 * `insert`, `replace`) turn on the ideograph boundaries Eye spaces. */
pub(super) fn autospace(value: &str) -> EngineResult<bool> {
    const KEYWORDS: [&str; 7] = [
        "normal",
        "auto",
        "ideograph-alpha",
        "ideograph-numeric",
        "punctuation",
        "insert",
        "replace",
    ];
    match value {
        "no-autospace" => Ok(false),
        _ if value
            .split_whitespace()
            .all(|word| KEYWORDS.contains(&word)) =>
        {
            Ok(true)
        }
        _ => Err(super::unsupported()),
    }
}

/** `hyphens`: true for `none`; `manual` and `auto` both break at soft hyphens. */
pub(super) fn hyphens(value: &str) -> EngineResult<bool> {
    match value {
        "none" => Ok(true),
        "manual" | "auto" => Ok(false),
        _ => Err(super::unsupported()),
    }
}

/** `hyphenate-character`: None for `auto`, else the quoted string with its escapes decoded. */
pub(super) fn hyphenate_character(value: &str) -> EngineResult<Option<String>> {
    if value == "auto" {
        return Ok(None);
    }
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'');
    let parsed = quote.and_then(|quote| super::content::string(&value[1..], quote));
    match parsed {
        Some((text, rest)) if rest.trim().is_empty() => Ok(Some(text)),
        _ => Err(super::unsupported()),
    }
}

/** `text-justify`: `distribute` is the legacy name of `inter-character`. */
pub(super) fn justify(value: &str) -> EngineResult<String> {
    Ok(match value {
        "auto" | "none" | "inter-word" | "inter-character" => value,
        "distribute" => "inter-character",
        _ => return Err(super::unsupported()),
    }
    .into())
}
