/**
 * `@container` conditions against one query container: size features (`width`, `inline-size`,
 * `height`, `block-size`, `aspect-ratio`, `orientation`) against its laid-out content box, and
 * `style(--name[: value])` against its custom properties. Anything else stays Unknown, so the
 * rule does not apply and its hiding rules keep masking.
 */
use super::{
    media::{self, Truth},
    syntax::{find_top, tokens},
};
use crate::document::RenderContext;

/// The function that opens a style query, matched ASCII case-insensitively.
const STYLE: &str = "style(";
/// Words that make a condition read the container's size rather than only its style.
const SIZE_FEATURES: [&str; 6] = [
    "width",
    "height",
    "inline-size",
    "block-size",
    "aspect-ratio",
    "orientation",
];

/** What a query can read of the container it resolved to. */
pub(super) struct Probe<'a> {
    /// Content-box width of a laid-out size container; None when it cannot answer.
    pub width: Option<f64>,
    /// Content-box height of a laid-out `container-type: size` box; None otherwise.
    pub height: Option<f64>,
    /// A custom property's computed value on the container, None when it has none.
    pub custom: &'a dyn Fn(&str) -> Option<String>,
}

/** The condition's truth for one container, in the same three-valued logic as `@media`. */
pub(super) fn truth(condition: &str, probe: &Probe<'_>) -> Truth {
    let source = outside_styles(condition, true);
    let parts = tokens(source.trim());
    media::condition(&parts, &|inner| leaf(inner, probe))
}

/** Whether the condition reads a size feature, so only a size container can answer it (a
 * style-only query is answered by the parent, as every element is a style container). */
pub(super) fn reads_size(condition: &str) -> bool {
    let sizeless = outside_styles(condition, false);
    SIZE_FEATURES.iter().any(|word| sizeless.contains(word))
}

/** One parenthesized test: a `style()` query or a size feature. */
fn leaf(inner: &str, probe: &Probe<'_>) -> Truth {
    let style = inner
        .get(..STYLE.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(STYLE));
    match inner.strip_suffix(')') {
        Some(body) if style => custom(&body[STYLE.len()..], probe),
        _ => size(inner, probe),
    }
}

/**
 * `--name` holds when the container has a value for it; `--name: value` when that value equals
 * the given one token for token. Queries on standard properties are not modelled (Unknown).
 */
fn custom(body: &str, probe: &Probe<'_>) -> Truth {
    let (name, wanted) = match body.split_once(':') {
        Some((name, value)) => (name.trim(), Some(value)),
        None => (body.trim(), None),
    };
    if !name.starts_with("--") || !super::layout_props::name(name) {
        return Truth::Unknown;
    }
    // ponytail: a `var()` inside the wanted value is compared as written, not substituted.
    let actual = (probe.custom)(name);
    Truth::of(match wanted {
        None => actual.is_some(),
        Some(wanted) => {
            actual.is_some_and(|actual| actual.split_whitespace().eq(wanted.split_whitespace()))
        }
    })
}

/** A size feature against the container's content box; Unknown when it lacks an axis it reads. */
fn size(inner: &str, probe: &Probe<'_>) -> Truth {
    let inner = inner
        .replace("inline-size", "width")
        .replace("block-size", "height");
    let both = inner.contains("aspect-ratio") || inner.contains("orientation");
    let reads_width = both || inner.contains("width");
    let reads_height = both || inner.contains("height");
    if inner.contains("device") || !(reads_width || reads_height) {
        return Truth::Unknown;
    }
    let axis = |known: Option<f64>, read: bool| if read { known } else { Some(0.0) };
    let (Some(width), Some(height)) = (
        axis(probe.width, reads_width),
        axis(probe.height, reads_height),
    ) else {
        return Truth::Unknown;
    };
    let context = RenderContext {
        width,
        height,
        ..RenderContext::default()
    };
    media::feature(&inner, context)
}

/**
 * The condition lowercased outside its `style()` bodies, which keep their case (custom property
 * names and values are case-sensitive); `bodies` false leaves the bodies out altogether.
 */
fn outside_styles(source: &str, bodies: bool) -> String {
    let lower = source.to_ascii_lowercase();
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    while let Some(found) = lower[at..].find(STYLE) {
        let open = at + found + STYLE.len();
        let close = find_top(source, open, b")");
        out.push_str(&lower[at..open]);
        if bodies {
            out.push_str(&source[open..close]);
        }
        at = close;
    }
    out.push_str(&lower[at..]);
    out
}
