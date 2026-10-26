/** Source URLs come only from real scanner trivia, never from strings, regexps, or template text. */
use super::*;

/** Rescanning uses the normal source/token bounds and lexical goal decisions before inspecting trivia. */
pub(in crate::v9) fn source_url(source: &str) -> EngineResult<Option<String>> {
    let tokens = tokenize(source)?;
    let mut result = None;
    let mut end = 0;
    for token in tokens {
        trivia(&source[end..token.location.byte_offset], &mut result);
        end = token.end;
    }
    Ok(result.filter(|value| !value.is_empty()))
}

/** Token spans exclude literal contents; comments inside each gap are consumed in their actual order. */
fn trivia(mut text: &str, result: &mut Option<String>) {
    while !text.is_empty() {
        text = text.trim_start_matches(|c| whitespace(c) || line_terminator(c));
        if let Some(rest) = text.strip_prefix("/*") {
            text = rest.find("*/").map_or("", |end| &rest[end + 2..]);
        } else if text.starts_with("//")
            || text.starts_with("<!--")
            || text.starts_with("-->")
            || text.starts_with("#!")
        {
            let end = text.find(line_terminator).unwrap_or(text.len());
            if let Some(value) = directive(&text[..end]) {
                *result = Some(value.to_owned());
            }
            text = &text[end..];
        } else {
            break;
        }
    }
}

/** V8's magic comment requires the marker, one separating whitespace, an exact name, and a single URL token. */
fn directive(text: &str) -> Option<&str> {
    let text = text
        .strip_prefix("//#")
        .or_else(|| text.strip_prefix("//@"))?;
    let space = text.chars().next().filter(|c| whitespace(*c))?;
    let value = text[space.len_utf8()..].strip_prefix("sourceURL=")?;
    let value = value.trim_start_matches(whitespace);
    let end = value.find(whitespace).unwrap_or(value.len());
    Some(if value[end..].chars().all(whitespace) {
        &value[..end]
    } else {
        ""
    })
}
