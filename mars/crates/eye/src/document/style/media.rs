/** `@media` and `@supports` conditions in three-valued logic; unknown features are never guessed true. */
use super::syntax::{split_top, tokens};
use crate::{
    constants::CSS_ROOT_FONT_SIZE,
    document::{Appearance, RenderContext},
};
/** A condition's value; Unknown evaluates as false but keeps hiding rules masked. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Truth {
    /// Definitely holds.
    True,
    /// Definitely does not hold.
    False,
    /// Depends on a feature Eye does not model.
    Unknown,
}
impl Truth {
    /** Boolean to definite truth. */
    pub(super) fn of(value: bool) -> Self {
        if value { Self::True } else { Self::False }
    }
    /** Kleene negation. */
    fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
    /** Kleene conjunction. */
    pub(super) fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::True, Self::True) => Self::True,
            _ => Self::Unknown,
        }
    }
    /** Kleene disjunction. */
    fn or(self, other: Self) -> Self {
        self.not().and(other.not()).not()
    }
}
/** The `@media` conditions a parse evaluated, each once with its answer. A parse depends on
its render context only through these, so it holds under any context that answers every one
the same way: a viewport a few pixels shorter (the window's first resize) keeps YouTube's
3.3 MB parse instead of starting it over. */
#[derive(Clone, Default)]
pub(super) struct MediaSeen(std::collections::BTreeMap<Box<str>, Truth>);

impl MediaSeen {
    /** Note that `condition` answered `truth`. */
    pub fn record(&mut self, condition: &str, truth: Truth) {
        if !self.0.contains_key(condition) {
            self.0.insert(condition.into(), truth);
        }
    }
    /** Whether `context` answers every noted condition as the parse saw it. */
    pub fn holds(&self, context: RenderContext) -> bool {
        self.0
            .iter()
            .all(|(condition, truth)| media(condition, context) == *truth)
    }
}

/** A media query list: any query holding makes the list hold; an empty list holds. */
pub(super) fn media(source: &str, context: RenderContext) -> Truth {
    let source = source.to_ascii_lowercase();
    let queries = split_top(&source, b',');
    if queries.is_empty() {
        return Truth::True;
    }
    queries
        .into_iter()
        .map(|query| query_truth(query, context))
        .fold(Truth::False, Truth::or)
}
/** `[not|only] type [and condition]` or a bare condition. */
fn query_truth(query: &str, context: RenderContext) -> Truth {
    let parts = tokens(query);
    let (negate, rest) = match parts.as_slice() {
        ["not", rest @ ..] if rest.first().is_some_and(|part| !part.starts_with('(')) => {
            (true, rest)
        }
        ["only", rest @ ..] => (false, rest),
        rest => (false, rest),
    };
    let truth = match rest {
        [kind, "and", conditions @ ..] if !kind.starts_with('(') => {
            media_type(kind).and(condition(conditions, &|inner| feature(inner, context)))
        }
        [kind] if !kind.starts_with('(') => media_type(kind),
        conditions => condition(conditions, &|inner| feature(inner, context)),
    };
    if negate { truth.not() } else { truth }
}
/** Screen and all hold; print and other known types do not. */
fn media_type(kind: &str) -> Truth {
    match kind {
        "all" | "screen" => Truth::True,
        "print" | "speech" | "tty" | "tv" | "projection" | "handheld" | "braille" | "embossed"
        | "aural" => Truth::False,
        // Media Queries 4: an unknown media type is a valid identifier that matches nothing.
        unknown if unknown.chars().all(|c| c.is_ascii_alphabetic() || c == '-') => Truth::False,
        _ => Truth::Unknown,
    }
}
/** `not C`, `C and C ...` or `C or C ...` over parenthesized leaves. */
pub(super) fn condition(parts: &[&str], leaf: &dyn Fn(&str) -> Truth) -> Truth {
    match parts {
        [] => Truth::Unknown,
        ["not", rest @ ..] => condition(rest, leaf).not(),
        [first, rest @ ..] => {
            let mut truth = group(first, leaf);
            let mut iter = rest.chunks(2);
            for pair in iter.by_ref() {
                match pair {
                    ["and", next] => truth = truth.and(group(next, leaf)),
                    ["or", next] => truth = truth.or(group(next, leaf)),
                    _ => return Truth::Unknown,
                }
            }
            truth
        }
    }
}
/** One parenthesized group: a nested condition or a leaf test. A function such as `style()`
 * or `selector()` is a leaf of its own, which the caller's leaf test reads whole. */
fn group(part: &str, leaf: &dyn Fn(&str) -> Truth) -> Truth {
    let Some(inner) = part.strip_prefix('(').and_then(|p| p.strip_suffix(')')) else {
        let function = part.ends_with(')') && part.contains('(');
        return if function { leaf(part) } else { Truth::Unknown };
    };
    let inner = inner.trim();
    let parts = tokens(inner);
    if inner.starts_with('(') || parts.first() == Some(&"not") {
        return condition(&parts, leaf);
    }
    leaf(inner)
}
/** `name: value`, range comparisons, or a bare boolean feature. */
pub(super) fn feature(inner: &str, context: RenderContext) -> Truth {
    if let Some((name, value)) = inner.split_once(':') {
        return named(name.trim(), value.trim(), context);
    }
    if inner.contains(['<', '>', '=']) {
        return range(inner, context);
    }
    match inner {
        "color" | "hover" | "any-hover" | "pointer" | "any-pointer" | "grid" => {
            Truth::of(inner != "grid")
        }
        "monochrome" | "inverted-colors" | "prefers-reduced-motion" | "prefers-contrast" => {
            Truth::False
        }
        _ => Truth::Unknown,
    }
}
/** `name: value` features, with min-/max- prefixes for ranges. */
fn named(name: &str, value: &str, context: RenderContext) -> Truth {
    let (name, compare): (&str, fn(f64, f64) -> bool) =
        if let Some(name) = name.strip_prefix("min-") {
            (name, |actual, wanted| actual >= wanted)
        } else if let Some(name) = name.strip_prefix("max-") {
            (name, |actual, wanted| actual <= wanted)
        } else {
            (name, |actual, wanted| actual == wanted)
        };
    if let Some(actual) = dimension(name, context) {
        return length(value).map_or(Truth::Unknown, |wanted| Truth::of(compare(actual, wanted)));
    }
    if name == "aspect-ratio" {
        return ratio(value).map_or(Truth::Unknown, |wanted| {
            Truth::of(compare(context.width / context.height, wanted))
        });
    }
    discrete(name, value, context)
}
/** Keyword features of the desktop host. */
fn discrete(name: &str, value: &str, context: RenderContext) -> Truth {
    let holds = match name {
        "prefers-color-scheme" => match value {
            "light" => context.appearance == Appearance::Light,
            "dark" => context.appearance == Appearance::Dark,
            _ => return Truth::Unknown,
        },
        "orientation" => match value {
            "portrait" => context.height >= context.width,
            "landscape" => context.width > context.height,
            _ => return Truth::Unknown,
        },
        "hover" | "any-hover" => value == "hover",
        "pointer" | "any-pointer" => value == "fine",
        "prefers-reduced-motion"
        | "prefers-contrast"
        | "prefers-reduced-transparency"
        | "prefers-reduced-data" => value == "no-preference",
        "forced-colors" | "inverted-colors" => value == "none",
        "display-mode" => value == "browser",
        "scripting" => value == "enabled",
        "update" => value == "fast",
        "color" => value.parse::<f64>().is_ok_and(|bits| bits <= 8.0),
        "monochrome" => value == "0",
        _ => return Truth::Unknown,
    };
    Truth::of(holds)
}
/** Viewport extents for width and height features. */
fn dimension(name: &str, context: RenderContext) -> Option<f64> {
    match name {
        "width" | "device-width" => Some(context.width),
        "height" | "device-height" => Some(context.height),
        _ => None,
    }
}
/** Media lengths: px, em and rem against the initial font size, and the absolute units
 * (`in`, `cm`, `pt`, ...) as declarations resolve them. */
fn length(value: &str) -> Option<f64> {
    let value = value.trim();
    if super::values::is_math(value) {
        // `calc()` and the other math functions, resolved as a declaration's length is.
        let length = super::values::dimension(value, CSS_ROOT_FONT_SIZE, RenderContext::default());
        return length
            .ok()
            .filter(|length| length.percent == 0.0 && length.px.is_finite())
            .map(|length| length.px);
    }
    let (number, scale) = if let Some(n) = value.strip_suffix("rem") {
        (n, CSS_ROOT_FONT_SIZE)
    } else if let Some(n) = value.strip_suffix("em") {
        (n, CSS_ROOT_FONT_SIZE)
    } else if let Some(n) = value.strip_suffix("px") {
        (n, 1.0)
    } else if ABSOLUTE.iter().any(|unit| value.ends_with(unit)) {
        let length = super::values::unit(value, CSS_ROOT_FONT_SIZE, RenderContext::default());
        return length
            .ok()
            .map(|length| length.px)
            .filter(|px| px.is_finite());
    } else {
        (value, if value == "0" { 1.0 } else { f64::NAN })
    };
    number
        .trim()
        .parse::<f64>()
        .ok()
        .map(|n| n * scale)
        .filter(|n| n.is_finite())
}
/// Absolute length units a media feature may use besides px.
const ABSOLUTE: [&str; 6] = ["in", "cm", "mm", "pt", "pc", "q"];
/** `a/b` or a single number. */
fn ratio(value: &str) -> Option<f64> {
    let (a, b) = value.split_once('/').unwrap_or((value, "1"));
    let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    (b > 0.0 && a.is_finite()).then(|| a / b)
}
/** Level 4 ranges: `width >= 600px`, `600px <= width`, `400px < width < 800px`. */
fn range(inner: &str, context: RenderContext) -> Truth {
    let mut operands = Vec::new();
    let mut operators = Vec::new();
    let mut rest = inner;
    while let Some(at) = rest.find(['<', '>', '=']) {
        operands.push(rest[..at].trim());
        let width = if rest[at + 1..].starts_with('=') {
            2
        } else {
            1
        };
        operators.push(&rest[at..at + width]);
        rest = &rest[at + width..];
    }
    operands.push(rest.trim());
    let value = |text: &str| {
        dimension(text, context)
            .map(Ok)
            .or_else(|| length(text).map(Err))
    };
    let mut truth = Truth::True;
    for (index, operator) in operators.iter().enumerate() {
        let (Some(left), Some(right)) = (value(operands[index]), value(operands[index + 1])) else {
            return Truth::Unknown;
        };
        let (Ok(a) | Err(a), Ok(b) | Err(b)) = (left, right);
        let holds = match *operator {
            "<" => a < b,
            "<=" => a <= b,
            ">" => a > b,
            ">=" => a >= b,
            "=" => a == b,
            _ => return Truth::Unknown,
        };
        truth = truth.and(Truth::of(holds));
    }
    truth
}
/** `@supports`: declarations Eye parses, `selector()` it can match, and boolean combinations. */
pub(super) fn supports(source: &str) -> Truth {
    let parts = tokens(source.trim());
    let leaf = |inner: &str| -> Truth {
        if let Some(selector) = inner
            .strip_prefix("selector(")
            .and_then(|s| s.strip_suffix(')'))
        {
            return Truth::of(super::selector_parse::parse(selector).is_ok());
        }
        let Some((name, value)) = inner.split_once(':') else {
            return Truth::False;
        };
        let name = name.trim();
        if name.starts_with("--") {
            return Truth::True;
        }
        Truth::of(super::declaration::validate(&name.to_ascii_lowercase(), value.trim()).is_ok())
    };
    let truth = if let [single] = parts.as_slice()
        && !single.starts_with('(')
    {
        group(&format!("({single})"), &leaf)
    } else {
        condition(&parts, &leaf)
    };
    match truth {
        Truth::Unknown => Truth::False,
        truth => truth,
    }
}
