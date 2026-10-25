/** The `content` property of `::before` and `::after`: strings, `attr()`, quotes; counters render empty. */
use std::collections::BTreeMap;

/** Generated text one box carries at most, in characters. */
pub(super) const GENERATED_CHARS: usize = 256;

/** The generated text, or None for `none`, `normal` and values Eye cannot render. Icon-font
 * code points (the private use area) are dropped, since no installed face draws them. Control
 * characters stay: CSS Text 3 section 4.1.1 says they render visibly, which text layout does. */
pub(super) fn content(value: &str, attributes: &BTreeMap<String, String>) -> Option<String> {
    let value = value.trim();
    if ["none", "normal", "inherit", "initial", "unset"]
        .contains(&value.to_ascii_lowercase().as_str())
    {
        return None;
    }
    let mut out = String::new();
    let mut rest = value;
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
        } else if c == '"' || c == '\'' {
            let (text, after) = string(&rest[1..], c)?;
            out.push_str(&text);
            rest = after;
        } else {
            let end = function_end(rest);
            let token = rest[..end].to_ascii_lowercase();
            match token.as_str() {
                "open-quote" => out.push('\u{201c}'),
                "close-quote" => out.push('\u{201d}'),
                "no-open-quote" | "no-close-quote" => {}
                _ if token.starts_with("attr(") => {
                    let name = token["attr(".len()..].trim_end_matches(')').trim();
                    out.push_str(attributes.get(name).map_or("", String::as_str));
                }
                _ if token.starts_with("counter") => {}
                _ => return None,
            }
            rest = &rest[end..];
        }
    }
    let text = out
        .chars()
        .filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c))
        .take(GENERATED_CHARS)
        .collect::<String>();
    Some(text)
}
/** A quoted string up to its closing quote, with CSS escapes decoded, and the text after it. */
fn string(source: &str, quote: char) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = source.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if c == quote {
            return Some((out, &source[index + 1..]));
        }
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        while let Some((_, h)) = chars.peek().copied() {
            if h.is_ascii_hexdigit() && hex.len() < 6 {
                hex.push(h);
                chars.next();
            } else {
                break;
            }
        }
        if hex.is_empty() {
            if let Some((_, escaped)) = chars.next() {
                out.push(escaped);
            }
        } else {
            if chars.peek().is_some_and(|(_, c)| *c == ' ') {
                chars.next();
            }
            out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
        }
    }
    None
}
/** Byte length of one keyword or function token, through its closing parenthesis. */
fn function_end(source: &str) -> usize {
    let mut depth = 0usize;
    for (index, c) in source.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index + 1;
                }
            }
            c if c.is_whitespace() && depth == 0 => return index,
            _ => {}
        }
    }
    source.len()
}
