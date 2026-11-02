/**
 * Container queries: `container-type`, `container-name`, `@container` size and style conditions,
 * and the `cq*` units. A size condition or unit reads the nearest qualifying ancestor's content
 * box, which only layout knows, so the cascade evaluates them in a second pass over a first
 * layout (see `sizes`). A style condition reads an ancestor's custom properties, which the
 * cascade already has when it styles a child, so it is decided in the first pass.
 */
use super::{
    ComputedStyle, ContainerType,
    constants::CONTAINER_NAMES,
    container_query::{self, Probe},
    layout_props::name as identifier,
    media::Truth,
    unsupported,
};
use crate::{
    document::{Document, RenderContext},
    protocol::EngineResult,
};
use std::collections::BTreeMap;
/** Laid-out content-box `[width, height]` of container boxes by identity, from the first layout. */
pub(super) type Sizes = BTreeMap<usize, [f64; 2]>;
/** One `@container` prelude: an optional container name and a condition Eye can evaluate. */
#[derive(Clone, Debug)]
pub(super) struct Query {
    /// The container name the query selects by; None takes the nearest eligible container.
    name: Option<String>,
    /// The condition text, lowercased except inside `style()`.
    condition: String,
    /// The condition reads a size feature, so only a size container can answer it.
    size: bool,
}
/** A named or size container among an element's ancestors, nearest first. */
#[derive(Clone, Debug)]
pub(super) struct Ancestor {
    /// The container's identity, the key into `Sizes`.
    pub local_id: usize,
    /// Its custom-property scope, which style queries read.
    pub scope: usize,
    /// Which axes size queries and units may read; `Normal` for a named style container.
    pub kind: ContainerType,
    /// Names it answers to.
    pub names: Vec<String>,
}
/** What a query resolved to: the container's scope and, for a size container, its record. */
type Resolved<'a> = (usize, Option<&'a Ancestor>);
impl Query {
    /**
     * Parses a prelude such as `card (min-width: 400px)` or `style(--theme: dark)`. A condition Eye
     * cannot evaluate, such as `scroll-state()` or a standard property, still parses: it is Unknown
     * for every element, so its rules do not apply and their hiding rules mask. None only when the
     * prelude has no condition or names an invalid container, which Blink drops as well.
     */
    pub(super) fn parse(prelude: &str) -> Option<Self> {
        let prelude = prelude.trim();
        let lower = prelude.to_ascii_lowercase();
        let bare = ["(", "not ", "style(", "scroll-state("]
            .iter()
            .any(|head| lower.starts_with(head));
        let end = if bare {
            0
        } else {
            lower.find(['(', ' ']).unwrap_or(lower.len())
        };
        let name = (!bare).then(|| lower[..end].to_string());
        if name
            .as_deref()
            .is_some_and(|name| !identifier(name) || ["none", "and", "or", "not"].contains(&name))
        {
            return None;
        }
        let condition = prelude[end..].trim().to_string();
        let size = container_query::reads_size(&condition);
        (!condition.is_empty()).then_some(Self {
            name,
            condition,
            size,
        })
    }
    /**
     * The query against its container: the nearest ancestor with its name (a size container
     * when it reads a size), else for a style-only query the parent (`parent`, its scope). CSS
     * makes it false without one, but Eye could have missed a container a dropped rule
     * declared, so it stays undecided: the rule does not apply and its hiding rules mask.
     */
    pub(super) fn truth(
        &self,
        ancestry: &[Ancestor],
        parent: Option<usize>,
        sizes: Option<&Sizes>,
        custom: &dyn Fn(usize, &str) -> Option<String>,
    ) -> Truth {
        let Some((scope, container)) = self.container(ancestry, parent) else {
            return Truth::Unknown;
        };
        let extent = container.and_then(|c| Some((c.kind, *sizes?.get(&c.local_id)?)));
        let probe = Probe {
            width: extent.map(|(_, [width, _])| width),
            height: extent
                .filter(|(kind, _)| *kind == ContainerType::Size)
                .map(|(_, [_, height])| height),
            custom: &|name| custom(scope, name),
        };
        container_query::truth(&self.condition, &probe)
    }
    /** The query container, per CSS Conditional 5: named or sized ones from the ancestry, and
     * the parent for an unnamed style-only query. */
    fn container<'a>(
        &self,
        ancestry: &'a [Ancestor],
        parent: Option<usize>,
    ) -> Option<Resolved<'a>> {
        if self.name.is_none() && !self.size {
            return parent.map(|scope| (scope, None));
        }
        let found = ancestry.iter().find(|ancestor| {
            let named = self
                .name
                .as_ref()
                .is_none_or(|name| ancestor.names.contains(name));
            named && (!self.size || ancestor.kind != ContainerType::Normal)
        })?;
        Some((found.scope, Some(found)))
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
/** A style's container record, when it is a size container or has a name. */
pub(super) fn ancestor(local_id: usize, scope: usize, style: &ComputedStyle) -> Option<Ancestor> {
    let kind = style.container_type;
    (kind != ContainerType::Normal || !style.container_names.is_empty()).then(|| Ancestor {
        local_id,
        scope,
        kind,
        names: style.container_names.clone(),
    })
}
/**
 * The `cq*` unit basis for an element: the content width of its nearest size container and the
 * content height of its nearest `size` container, when laid out. None falls back to the
 * viewport, as CSS does when no container qualifies (and as the first pass must).
 */
pub(super) fn extent(ancestry: &[Ancestor], sizes: Option<&Sizes>) -> [Option<f64>; 2] {
    let read = |eligible: fn(ContainerType) -> bool, axis: usize| {
        let container = ancestry.iter().find(|ancestor| eligible(ancestor.kind))?;
        Some(sizes?.get(&container.local_id)?[axis])
    };
    [
        read(|kind| kind != ContainerType::Normal, 0),
        read(|kind| kind == ContainerType::Size, 1),
    ]
}
/**
 * Lays the first-pass styles out once and reads every container's content box. None when no
 * box is a container or the layout fails; the queries then stay undecided, as before.
 */
pub(super) fn sizes(document: &Document, context: RenderContext) -> Option<Sizes> {
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
        first
            .entry(item.local_id)
            .or_insert([item.width, item.height]);
    }
    let mut sizes = Sizes::new();
    for node in &document.nodes {
        if node.style.container_type == ContainerType::Normal {
            continue;
        }
        if let Some(border_box) = first.get(&node.local_id) {
            sizes.insert(node.local_id, content_box(&node.style, *border_box));
        }
    }
    Some(sizes)
}
/** A border box less its padding and borders. */
fn content_box(style: &ComputedStyle, [width, height]: [f64; 2]) -> [f64; 2] {
    // ponytail: a percentage padding on the container itself is left out of its content box.
    let sides = if style.border > 0.0 {
        [style.border; 4]
    } else {
        style.border_widths
    };
    let across = style.padding[1] + style.padding[3] + sides[1] + sides[3];
    let down = style.padding[0] + style.padding[2] + sides[0] + sides[2];
    [(width - across).max(0.0), (height - down).max(0.0)]
}
