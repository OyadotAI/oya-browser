/**
 * Container queries: `container-type`, `container-name` and `@container` width conditions. A
 * condition reads the nearest qualifying ancestor's content-box width, which only layout knows,
 * so the cascade evaluates them in a second pass over a first layout (see `widths`).
 */
use super::{
    ComputedStyle, ContainerType,
    constants::CONTAINER_NAMES,
    layout_props::name as identifier,
    media::{self, Truth},
    unsupported,
};
use crate::{
    document::{Document, RenderContext},
    protocol::EngineResult,
};
use std::collections::BTreeMap;
/** Laid-out content-box widths of container boxes by identity, from the first layout. */
pub(super) type Widths = BTreeMap<usize, f64>;
/** One `@container` prelude: an optional container name and a width condition Eye can evaluate. */
#[derive(Clone, Debug)]
pub(super) struct Query {
    /// The container name the query selects by; None takes the nearest size container.
    name: Option<String>,
    /// The lowercase condition text.
    condition: String,
}
/** A size container among an element's ancestors, nearest first. */
#[derive(Clone, Debug)]
pub(super) struct Ancestor {
    /// The container's identity, the key into `Widths`.
    pub local_id: usize,
    /// Names it answers to.
    pub names: Vec<String>,
}
impl Query {
    /**
     * Parses a prelude such as `card (min-width: 400px)`. None when the condition reads anything
     * but width, so the caller keeps treating the block as undecidable and masks its hiding rules.
     */
    pub(super) fn parse(prelude: &str) -> Option<Self> {
        let prelude = prelude.trim().to_ascii_lowercase();
        let bare = prelude.starts_with('(') || prelude.starts_with("not ");
        let (name, condition) = if bare {
            (None, prelude.as_str())
        } else {
            let end = prelude.find(['(', ' ']).unwrap_or(prelude.len());
            let (name, rest) = prelude.split_at(end);
            (Some(name.to_string()), rest)
        };
        if name
            .as_deref()
            .is_some_and(|name| !identifier(name) || ["none", "and", "or", "not"].contains(&name))
        {
            return None;
        }
        let condition = condition.trim().to_string();
        let decidable = [0.0, crate::constants::CSS_MAX_SCALAR]
            .iter()
            .all(|width| media::container(&condition, Some(*width)) != Truth::Unknown);
        (!condition.is_empty() && decidable).then_some(Self { name, condition })
    }
    /**
     * The query against the nearest matching ancestor. CSS makes it false without one, but Eye
     * could have missed a container a dropped rule declared, so it stays undecided: the rule does
     * not apply and its hiding rules mask, as for a container not laid out yet.
     */
    pub(super) fn truth(&self, ancestry: &[Ancestor], widths: Option<&Widths>) -> Truth {
        let Some(container) = ancestry.iter().find(|ancestor| {
            self.name
                .as_ref()
                .is_none_or(|name| ancestor.names.contains(name))
        }) else {
            return Truth::Unknown;
        };
        let width = widths.and_then(|widths| widths.get(&container.local_id).copied());
        media::container(&self.condition, width)
    }
}
/** `normal`, `inline-size` or `size`; `scroll-state` alone is no size container. */
pub(super) fn kind(value: &str) -> EngineResult<ContainerType> {
    Ok(match value {
        "normal" | "scroll-state" => ContainerType::Normal,
        "inline-size" => ContainerType::InlineSize,
        "size" => ContainerType::Size,
        _ => return Err(unsupported()),
    })
}
/** `none` or a list of custom identifiers, bounded. */
pub(super) fn names(value: &str) -> EngineResult<Vec<String>> {
    if value == "none" {
        return Ok(Vec::new());
    }
    let names: Vec<String> = value.split_whitespace().map(Into::into).collect();
    if names.len() > CONTAINER_NAMES || names.iter().any(|n| !identifier(n) || n == "none") {
        return Err(unsupported());
    }
    Ok(names)
}
/** `container: name [/ type]`, as its two longhands. */
pub(super) fn shorthand(value: &str) -> Option<Vec<(String, String)>> {
    let (names, kind) = value.split_once('/').unwrap_or((value, "normal"));
    Some(vec![
        ("container-name".into(), names.trim().into()),
        ("container-type".into(), kind.trim().into()),
    ])
}
/** A style's size-container record, when it is one. */
pub(super) fn ancestor(local_id: usize, style: &ComputedStyle) -> Option<Ancestor> {
    (style.container_type != ContainerType::Normal).then(|| Ancestor {
        local_id,
        names: style.container_names.clone(),
    })
}
/**
 * Lays the first-pass styles out once and reads every container's content-box width. None when
 * no box is a container or the layout fails; the queries then stay undecided, as before.
 */
pub(super) fn widths(document: &Document, context: RenderContext) -> Option<Widths> {
    if !document
        .nodes
        .iter()
        .any(|node| node.style.container_type != ContainerType::Normal)
    {
        return None;
    }
    let display = crate::document::render_with_context(document, context).ok()?;
    let mut first = BTreeMap::new();
    for item in &display.items {
        first.entry(item.local_id).or_insert(item.width);
    }
    let mut widths = Widths::new();
    for node in &document.nodes {
        let Some(width) = first.get(&node.local_id) else {
            continue;
        };
        if node.style.container_type == ContainerType::Normal {
            continue;
        }
        // ponytail: a percentage padding on the container itself is left out of its content width.
        let style = &node.style;
        let sides = if style.border > 0.0 {
            [style.border; 4]
        } else {
            style.border_widths
        };
        let edges = style.padding[1] + style.padding[3] + sides[1] + sides[3];
        widths.insert(node.local_id, (width - edges).max(0.0));
    }
    Some(widths)
}
