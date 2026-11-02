/** The CSS font shorthand as the canvas font setter reads it, and the canonical text its getter returns. */
use super::*;

/** Keyword sizes of CSS Fonts' medium scale, in CSS pixels. */
const SIZE_KEYWORDS: [(&str, f64); 8] = [
    ("xx-small", 9.0),
    ("x-small", 10.0),
    ("small", 13.0),
    ("medium", 16.0),
    ("large", 18.0),
    ("x-large", 24.0),
    ("xx-large", 32.0),
    ("xxx-large", 48.0),
];
/** CSS pixels per inch, the anchor every absolute length unit converts through. */
const CSS_PX_PER_INCH: f64 = 96.0;
/** Points per inch, for the pt unit. */
const POINTS_PER_INCH: f64 = 72.0;
/** Centimetres per inch, for the cm unit. */
const CM_PER_INCH: f64 = 2.54;
/** Millimetres per inch, for the mm unit. */
const MM_PER_INCH: f64 = 25.4;
/** Quarter-millimetres per inch, for the q unit. */
const Q_PER_INCH: f64 = 101.6;
/** A pica is twelve points, which is sixteen CSS pixels. */
const PICA_PX: f64 = 16.0;
/** Absolute length units and their CSS pixel factors. */
const UNITS: [(&str, f64); 7] = [
    ("px", 1.0),
    ("pt", CSS_PX_PER_INCH / POINTS_PER_INCH),
    ("pc", PICA_PX),
    ("in", CSS_PX_PER_INCH),
    ("cm", CSS_PX_PER_INCH / CM_PER_INCH),
    ("mm", CSS_PX_PER_INCH / MM_PER_INCH),
    ("q", CSS_PX_PER_INCH / Q_PER_INCH),
];
/** Relative size keywords scale the parent font by this ratio. */
const RELATIVE_SCALE: f64 = 1.2;
/** Percentages are hundredths of the parent font size. */
const PERCENT: f64 = 100.0;
/** Normal weight, which the getter omits. */
const NORMAL_WEIGHT: u16 = 400;
/** Bold weight, which the getter spells as the keyword. */
const BOLD_WEIGHT: u16 = 700;
/** The canvas parent weight is normal, so bolder and lighter step from 400 to these. */
const BOLDER_WEIGHT: u16 = 700;
/** See BOLDER_WEIGHT. */
const LIGHTER_WEIGHT: u16 = 100;
/** Generic family keywords; the host draws each one with the closest of its three faces. */
const GENERICS: [&str; 6] = [
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
];
/** Font-style keywords the shorthand accepts before its size. */
const STYLES: [&str; 2] = ["italic", "oblique"];
/** Font-stretch keywords the shorthand accepts before its size. */
const STRETCHES: [&str; 9] = [
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
    "normal",
];
/** CSS-wide keywords are never valid in a font value. */
const CSS_WIDE: [&str; 4] = ["inherit", "initial", "unset", "revert"];
/** Fractional digits the getter keeps for a pixel size. */
const DECIMALS: usize = 4;
/** The keyword that is valid in every prefix position. */
const NORMAL: &str = "normal";
/** The small-caps variant keyword. */
const SMALL_CAPS: &str = "small-caps";

/** One lexical part of the shorthand; quotes keep family names whole. */
#[derive(Debug, PartialEq)]
enum Token {
    /** An unquoted run of characters. */
    Word(String),
    /** A quoted family name, with escapes resolved. */
    Quoted(String),
    /** Separates families. */
    Comma,
    /** Separates the size from its line height. */
    Slash,
}

/** A family as the shorthand names it: a generic keyword or a named family. */
#[derive(Clone, Debug, PartialEq)]
enum Family {
    /** One of the generic keywords, lowercase. */
    Generic(&'static str),
    /** A family name as written, with its quotes removed. */
    Named(String),
}

/** The parts before the size, each already in canonical form. */
#[derive(Debug)]
struct Prefix {
    /** Italic or oblique, when given. */
    style: Option<&'static str>,
    /** Whether small-caps was given. */
    small_caps: bool,
    /** Numeric weight, 400 unless a weight was given. */
    weight: u16,
    /** Stretch keyword, normal unless given. */
    stretch: &'static str,
}

impl Default for Prefix {
    /** Every omitted prefix part has its initial value. */
    fn default() -> Self {
        Self {
            style: None,
            small_caps: false,
            weight: NORMAL_WEIGHT,
            stretch: NORMAL,
        }
    }
}

/** A complete valid font value, or None; an invalid value leaves the context's font unchanged. */
pub(super) fn parse(text: &str, parent: f64) -> Option<Font> {
    let tokens = tokenize(text)?;
    let (prefix, size_at) = prefix(&tokens, parent)?;
    let (size, rest) = size(&tokens, size_at, parent)?;
    if !(size.is_finite() && size > 0.0 && size <= f64::from(limits::EDGE)) {
        return None;
    }
    let families = families(&tokens[rest..])?;
    let family = families
        .iter()
        .find_map(|family| match family {
            Family::Generic(name) => Some(host_family(name)),
            Family::Named(_) => None,
        })
        .unwrap_or("sans-serif");
    Some(Font {
        size,
        family: family.into(),
        weight: prefix.weight,
        italic: prefix.style.is_some(),
        text: canonical(&prefix, size, &families),
        letter_spacing: 0.0,
        word_spacing: 0.0,
    })
}

/** Split the value into words, quoted names, commas and slashes; an unterminated quote is invalid. */
fn tokenize(text: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                flush(&mut word, &mut tokens);
                tokens.push(Token::Quoted(quoted(&mut chars, c)?));
            }
            ',' | '/' => {
                flush(&mut word, &mut tokens);
                tokens.push(if c == ',' { Token::Comma } else { Token::Slash });
            }
            c if c.is_whitespace() => flush(&mut word, &mut tokens),
            c => word.push(c),
        }
    }
    flush(&mut word, &mut tokens);
    Some(tokens)
}

/** Emit the pending word, if any. */
fn flush(word: &mut String, tokens: &mut Vec<Token>) {
    if !word.is_empty() {
        tokens.push(Token::Word(std::mem::take(word)));
    }
}

/** Read a quoted name up to its closing quote, resolving backslash escapes. */
fn quoted(chars: &mut std::str::Chars, quote: char) -> Option<String> {
    let mut name = String::new();
    while let Some(c) = chars.next() {
        match c {
            c if c == quote => return Some(name),
            '\\' => name.push(chars.next()?),
            c => name.push(c),
        }
    }
    None
}

/** Consume style, variant, weight and stretch words until one parses as the size. */
fn prefix(tokens: &[Token], parent: f64) -> Option<(Prefix, usize)> {
    let mut prefix = Prefix::default();
    for (index, token) in tokens.iter().enumerate() {
        let Token::Word(word) = token else {
            return None;
        };
        if length(word, parent).is_some() {
            return Some((prefix, index));
        }
        take(&mut prefix, word)?;
    }
    None
}

/** Record one prefix keyword or numeric weight; anything else is not part of a font value. */
fn take(prefix: &mut Prefix, word: &str) -> Option<()> {
    let word = word.to_ascii_lowercase();
    let word = word.as_str();
    if let Some(style) = STYLES.iter().find(|style| **style == word) {
        prefix.style = Some(*style);
    } else if word == SMALL_CAPS {
        prefix.small_caps = true;
    } else if let Some(stretch) = STRETCHES.iter().find(|stretch| **stretch == word) {
        prefix.stretch = *stretch;
    } else {
        prefix.weight = weight(word)?;
    }
    Some(())
}

/** The numeric weight a keyword or number names; relative keywords step from the normal parent. */
fn weight(word: &str) -> Option<u16> {
    match word {
        NORMAL => Some(NORMAL_WEIGHT),
        "bold" => Some(BOLD_WEIGHT),
        "bolder" => Some(BOLDER_WEIGHT),
        "lighter" => Some(LIGHTER_WEIGHT),
        _ => word
            .parse::<u16>()
            .ok()
            .filter(|value| limits::WEIGHT_RANGE.contains(value)),
    }
}

/** The size at a token, with the line height after its optional slash; the line height is validated and ignored. */
fn size(tokens: &[Token], at: usize, parent: f64) -> Option<(f64, usize)> {
    let Token::Word(word) = tokens.get(at)? else {
        return None;
    };
    let px = length(word, parent)?;
    if tokens.get(at + 1) != Some(&Token::Slash) {
        return Some((px, at + 1));
    }
    line_height(tokens.get(at + 2)?)?;
    Some((px, at + 3))
}

/** A line height is normal, a number or a length; the canvas font getter never reports it. */
fn line_height(token: &Token) -> Option<()> {
    let Token::Word(word) = token else {
        return None;
    };
    if word == NORMAL || word.parse::<f64>().is_ok() || length(word, 0.0).is_some() {
        return Some(());
    }
    None
}

/** A size word in CSS pixels, or None when it is not a length or size keyword. */
fn length(word: &str, parent: f64) -> Option<f64> {
    let word = word.to_ascii_lowercase();
    let word = word.as_str();
    if let Some((_, px)) = SIZE_KEYWORDS.iter().find(|(name, _)| *name == word) {
        return Some(*px);
    }
    match word {
        "larger" => return Some(parent * RELATIVE_SCALE),
        "smaller" => return Some(parent / RELATIVE_SCALE),
        _ => {}
    }
    let split = word.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))?;
    let (number, unit) = word.split_at(split);
    let value: f64 = number.parse().ok()?;
    let unit = unit.to_ascii_lowercase();
    if unit == "%" {
        return Some(value * parent / PERCENT);
    }
    if unit == "em" {
        return Some(value * parent);
    }
    UNITS
        .iter()
        .find(|(name, _)| *name == unit)
        .map(|(_, factor)| value * factor)
}

/** Each comma-separated family is a quoted name, or one or more identifiers joined by spaces. */
fn families(tokens: &[Token]) -> Option<Vec<Family>> {
    tokens
        .split(|token| *token == Token::Comma)
        .map(family)
        .collect()
}

/** One family: an unquoted generic keyword, a run of identifiers, or a single quoted name. */
fn family(tokens: &[Token]) -> Option<Family> {
    match tokens {
        [Token::Quoted(name)] if !name.is_empty() => Some(Family::Named(name.clone())),
        [Token::Word(word)] if GENERICS.iter().any(|g| g.eq_ignore_ascii_case(word)) => {
            let name = GENERICS.iter().find(|g| g.eq_ignore_ascii_case(word))?;
            Some(Family::Generic(name))
        }
        [Token::Word(_), ..] => {
            let words = tokens
                .iter()
                .map(|token| match token {
                    Token::Word(word) if identifier(word) => Some(word.as_str()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Family::Named(words.join(" ")))
        }
        _ => None,
    }
}

/** An unquoted family word: a CSS identifier that is not a CSS-wide keyword. */
fn identifier(word: &str) -> bool {
    let first = word.chars().next();
    let valid_start =
        first.is_some_and(|c| c.is_alphabetic() || c == '_' || c == '-' || !c.is_ascii());
    valid_start
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii())
        && !CSS_WIDE
            .iter()
            .any(|keyword| keyword.eq_ignore_ascii_case(word))
}

/** The generic the host draws with: serif and monospace stay themselves, everything else is sans-serif. */
fn host_family(name: &str) -> &'static str {
    match name {
        "serif" => "serif",
        "monospace" => "monospace",
        _ => "sans-serif",
    }
}

/** The canonical font text: style, variant, weight, stretch, size in pixels, then the families. */
fn canonical(prefix: &Prefix, size: f64, families: &[Family]) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(style) = prefix.style {
        parts.push(style.into());
    }
    if prefix.small_caps {
        parts.push("small-caps".into());
    }
    match prefix.weight {
        NORMAL_WEIGHT => {}
        BOLD_WEIGHT => parts.push("bold".into()),
        weight => parts.push(weight.to_string()),
    }
    if prefix.stretch != NORMAL {
        parts.push(prefix.stretch.into());
    }
    parts.push(format!("{}px", number(size)));
    parts.push(
        families
            .iter()
            .map(serialize)
            .collect::<Vec<_>>()
            .join(", "),
    );
    parts.join(" ")
}

/** A family as the getter writes it: generics bare, names bare when identifiers, quoted otherwise. */
fn serialize(family: &Family) -> String {
    match family {
        Family::Generic(name) => (*name).into(),
        Family::Named(name)
            if identifier(name) && !name.contains(' ') && !GENERICS.contains(&name.as_str()) =>
        {
            name.clone()
        }
        Family::Named(name) => format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}

/** A pixel size with at most DECIMALS fractional digits and no trailing zeros, as the getter reports it. */
fn number(value: f64) -> String {
    let text = format!("{value:.*}", DECIMALS);
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}
