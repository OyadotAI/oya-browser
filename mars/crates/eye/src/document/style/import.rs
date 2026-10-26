/** `@import` rules a sheet starts with, for the runtime to fetch and cascade before the sheet. */
use super::syntax::split_top;

/// Imports one sheet may name; later ones are ignored, so a sheet cannot fan out without bound.
const SHEET_IMPORTS: usize = 16;

/**
 * The `(href, media)` of each valid `@import` at the start of `sheet`, per CSS Cascade 5: an
 * import counts only before every other valid rule except `@charset` and `@layer` statements.
 * Rules the parser drops (a statement at-rule with no valid form, a style rule whose selector
 * Eye cannot parse) do not end the imports, while a stray token runs into the next block as one
 * invalid rule, swallowing any `@import` inside it. `media` is empty for all media; `layer` and
 * `supports()` conditions are not read.
 */
pub fn css_imports(sheet: &str) -> Vec<(String, String)> {
    let mut imports = Vec::new();
    let mut rest = sheet;
    while imports.len() < SHEET_IMPORTS {
        rest = skip_trivia(rest);
        if rest.is_empty() {
            break;
        }
        let Some((rule, next)) = next_rule(rest) else {
            break;
        };
        match rule {
            Rule::Import(prelude) => imports.extend(import(prelude)),
            Rule::Inert => {}
            Rule::Valid => break,
        }
        rest = next;
    }
    imports
}

/** What one top-level rule means for the imports after it. */
enum Rule<'a> {
    /// An `@import` statement with its prelude after the name.
    Import(&'a str),
    /// A rule that is dropped or allowed before imports.
    Inert,
    /// A rule that ends the import section.
    Valid,
}

/** The first rule of `text` and the text after it; None when it never ends. */
fn next_rule(text: &str) -> Option<(Rule<'_>, &str)> {
    let end = rule_end(text)?;
    let (rule, rest) = (&text[..end], &text[end..]);
    let Some(at) = rule.strip_prefix('@') else {
        let prelude = rule.split('{').next().unwrap_or_default();
        let valid = rule.ends_with('}') && super::selector_parse::parse(prelude.trim()).is_ok();
        return Some((if valid { Rule::Valid } else { Rule::Inert }, rest));
    };
    let name: String = at
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase();
    let body = at[name.len()..].trim_end_matches(';').trim();
    Some((
        match (name.as_str(), rule.ends_with(';')) {
            ("import", true) => Rule::Import(body),
            ("charset" | "layer", true) => Rule::Inert,
            // Statement forms of block at-rules (`@media;`, `@page;`) are invalid and dropped.
            ("media" | "page" | "supports" | "font-face" | "keyframes", true) => Rule::Inert,
            _ => Rule::Valid,
        },
        rest,
    ))
}

/** Byte length of the first rule: an at-rule through its top-level `;` or its block's closing
 * brace; a style rule (whose prelude a `;` does not end) through its block's closing brace. */
fn rule_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let statement = text.starts_with('@');
    let (mut depth, mut at, mut quote) = (0usize, 0, None);
    while at < bytes.len() {
        let byte = bytes[at];
        match (quote, byte) {
            (Some(_), b'\\') => at += 1,
            (Some(q), _) if byte == q => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'{') => depth += 1,
            (None, b'}') if depth <= 1 => return Some(at + 1),
            (None, b'}') => depth -= 1,
            (None, b';') if depth == 0 && statement => return Some(at + 1),
            _ => {}
        }
        at += 1;
    }
    None
}

/** Skips whitespace, comments and the HTML comment markers CSS ignores at the top level. */
fn skip_trivia(mut text: &str) -> &str {
    loop {
        let trimmed = text.trim_start();
        text = if let Some(comment) = trimmed.strip_prefix("/*") {
            comment.find("*/").map_or("", |end| &comment[end + 2..])
        } else if let Some(rest) = trimmed
            .strip_prefix("<!--")
            .or_else(|| trimmed.strip_prefix("-->"))
        {
            rest
        } else {
            return trimmed;
        };
    }
}

/** The href and media of one import prelude: `url(x)` or a string, then a media list. */
fn import(prelude: &str) -> Option<(String, String)> {
    let (href, rest) = if let Some(url) = prelude
        .strip_prefix("url(")
        .or_else(|| prelude.strip_prefix("URL("))
    {
        let close = url.find(')')?;
        (unquote(&url[..close]), &url[close + 1..])
    } else {
        let quote = prelude.chars().next().filter(|c| *c == '"' || *c == '\'')?;
        let close = prelude[1..].find(quote)? + 1;
        (prelude[1..close].to_string(), &prelude[close + 1..])
    };
    let rest = without_comments(rest);
    let media: Vec<&str> = split_top(rest.trim(), b' ')
        .into_iter()
        .filter(|word| !word.starts_with("layer") && !word.starts_with("supports("))
        .collect();
    Some((href, media.join(" ").trim().to_string()))
}

/** `text` with its `/* */` comments removed. */
fn without_comments(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        out.push(' ');
        rest = rest[start + 2..]
            .find("*/")
            .map_or("", |end| &rest[start + 2 + end + 2..]);
    }
    out.push_str(rest);
    out
}

/** A url() argument without its quotes. */
fn unquote(text: &str) -> String {
    text.trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string()
}
