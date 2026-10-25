/** `text-box-trim`, `text-box-edge` and their `text-box` shorthand (CSS Inline 3), parsed as
 * Blink's ComputedStyle keeps them: which block edges trim, and the font edges they trim to. */
use super::ComputedStyle;
use crate::protocol::EngineResult;

/** `text-box-edge` over values: the font's ascent (`text`, and `auto`, which copies the
 * initial `line-fit-edge`), its cap height, or its x-height. */
pub(crate) const OVER_TEXT: u8 = 0;
/** Over edge at the cap height. */
pub(crate) const OVER_CAP: u8 = 1;
/** Over edge at the x-height. */
pub(crate) const OVER_EX: u8 = 2;
/** `text-box-edge` under values: the font's descent (`text`, `auto`). */
pub(crate) const UNDER_TEXT: u8 = 0;
/** Under edge at the alphabetic baseline. */
pub(crate) const UNDER_ALPHABETIC: u8 = 1;

/** Whether a style trims nothing, so serialization leaves the field out. */
pub(crate) fn untrimmed(trim: &[bool; 2]) -> bool {
    !trim[0] && !trim[1]
}
/** Whether a style keeps the initial `text-box-edge: auto`. */
pub(crate) fn auto_edge(edge: &[u8; 2]) -> bool {
    *edge == [OVER_TEXT, UNDER_TEXT]
}

/** Applies `text-box-trim`, `text-box-edge` or `text-box`; false for any other property. */
pub(super) fn apply(style: &mut ComputedStyle, key: &str, value: &str) -> EngineResult<bool> {
    let value = value.trim().to_ascii_lowercase();
    match key {
        "text-box-trim" => style.text_box_trim = trim(&value).ok_or_else(|| invalid(key))?,
        "text-box-edge" => style.text_box_edge = edge(&value).ok_or_else(|| invalid(key))?,
        "text-box" => {
            let (trim, edge) = shorthand(&value).ok_or_else(|| invalid(key))?;
            style.text_box_trim = trim;
            style.text_box_edge = edge;
        }
        _ => return Ok(false),
    }
    Ok(true)
}
/** The error an unparsable value raises, dropping the declaration. */
fn invalid(_key: &str) -> crate::protocol::EngineError {
    super::unsupported()
}
/** `none | trim-start | trim-end | trim-both` as (start, end). */
fn trim(value: &str) -> Option<[bool; 2]> {
    Some(match value {
        "none" => [false, false],
        "trim-start" => [true, false],
        "trim-end" => [false, true],
        "trim-both" => [true, true],
        _ => return None,
    })
}
/** `auto | [text | cap | ex] [text | alphabetic]?`; one value names both edges when it is
 * `text`, and only the over edge otherwise (its under edge is then `text`). */
fn edge(value: &str) -> Option<[u8; 2]> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let over = |part: &str| match part {
        "text" => Some(OVER_TEXT),
        "cap" => Some(OVER_CAP),
        "ex" => Some(OVER_EX),
        _ => None,
    };
    let under = |part: &str| match part {
        "text" => Some(UNDER_TEXT),
        "alphabetic" => Some(UNDER_ALPHABETIC),
        _ => None,
    };
    match parts.as_slice() {
        ["auto"] => Some([OVER_TEXT, UNDER_TEXT]),
        [first] => Some([over(first)?, UNDER_TEXT]),
        [first, second] => Some([over(first)?, under(second)?]),
        _ => None,
    }
}
/** `normal | <text-box-trim> || <text-box-edge>`: a missing trim is `trim-both`, a missing
 * edge `auto`. */
fn shorthand(value: &str) -> Option<([bool; 2], [u8; 2])> {
    if value == "normal" {
        return Some(([false, false], [OVER_TEXT, UNDER_TEXT]));
    }
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let (first, rest) = parts.split_first()?;
    match trim(first) {
        Some(_) if *first == "none" => None,
        Some(trimmed) if rest.is_empty() => Some((trimmed, [OVER_TEXT, UNDER_TEXT])),
        Some(trimmed) => Some((trimmed, edge(&rest.join(" "))?)),
        None => {
            let last = parts.last()?;
            match trim(last) {
                Some(trimmed) if *last != "none" && parts.len() > 1 => {
                    Some((trimmed, edge(&parts[..parts.len() - 1].join(" "))?))
                }
                _ => Some(([true, true], edge(value)?)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorthand_defaults_trim_both_and_auto_edge() {
        assert_eq!(
            shorthand("ex alphabetic"),
            Some(([true, true], [OVER_EX, UNDER_ALPHABETIC]))
        );
        assert_eq!(
            shorthand("trim-end"),
            Some(([false, true], [OVER_TEXT, UNDER_TEXT]))
        );
        assert_eq!(
            shorthand("trim-start cap alphabetic"),
            Some(([true, false], [OVER_CAP, UNDER_ALPHABETIC]))
        );
        assert_eq!(
            shorthand("normal"),
            Some(([false, false], [OVER_TEXT, UNDER_TEXT]))
        );
        assert_eq!(shorthand("none text"), None);
    }

    #[test]
    fn one_edge_value_sets_the_over_edge() {
        assert_eq!(edge("cap"), Some([OVER_CAP, UNDER_TEXT]));
        assert_eq!(edge("alphabetic"), None);
        assert_eq!(edge("ideographic"), None);
    }
}
