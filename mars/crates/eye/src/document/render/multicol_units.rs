/** What may not break inside a multicol container's single column: text lines, monolithic
boxes (replaced elements, controls, table rows, atomic inlines, scroll containers, size
containment, `break-inside: avoid`) and avoided edges, plus the forced breaks. */
use super::multicol_balance::Unit;
use super::sizing::LayoutContext;
use crate::document::{DisplayItem, Node};

/** Half the extent of the unit that keeps a `break-*: avoid` edge from breaking. */
const GLUE: f64 = 0.05;
/** Tags of replaced and control boxes, which never break across columns. */
const MONOLITHIC: [&str; 11] = [
    "img", "canvas", "svg", "input", "button", "select", "textarea", "video", "iframe", "object",
    "tr",
];

/** The unbreakable units: every text line and every monolithic box. */
pub(super) fn units(
    context: &LayoutContext<'_>,
    items: &[DisplayItem],
    (start, owner): (usize, usize),
    anchors: &[Option<f64>],
) -> Vec<Unit> {
    let lines = |item: &DisplayItem| {
        let advance = item.line_height.unwrap_or(0.0);
        item.lines
            .iter()
            .map(move |line| Unit {
                top: line.y,
                bottom: line.y + advance,
            })
            .collect::<Vec<_>>()
    };
    let mut units = lines(&items[owner]);
    let free = (items[start..].iter().zip(anchors)).filter(|(_, anchor)| anchor.is_none());
    for (item, _) in free {
        units.extend(lines(item));
        if monolithic(context, item) {
            units.push(Unit {
                top: item.y,
                bottom: item.y + item.height,
            });
        }
        units.extend(glue(context, item));
    }
    units
}

/** A sliver of unit across an edge with `break-before`/`break-after: avoid`, which chains the
boxes on both sides into one unbreakable run when they are unbreakable themselves. */
fn glue(context: &LayoutContext<'_>, item: &DisplayItem) -> Vec<Unit> {
    let Ok(node) = context.node(item.local_id) else {
        return Vec::new();
    };
    let edges = [item.y, item.y + item.height];
    (0..edges.len())
        .filter(|side| !item.tag.starts_with("::") && node.style.multicol.avoid_edges[*side])
        .map(|side| Unit {
            top: edges[side] - GLUE,
            bottom: edges[side] + GLUE,
        })
        .collect()
}

/** Boxes that never break: replaced elements, controls, table rows, atomic inlines, scroll
containers, size-contained boxes and `break-inside: avoid` boxes (taken as unbreakable). */
fn monolithic(context: &LayoutContext<'_>, item: &DisplayItem) -> bool {
    MONOLITHIC.contains(&item.tag.as_str())
        || !item.tag.starts_with("::") && context.node(item.local_id).is_ok_and(monolithic_node)
}

/** Whether a node's box never breaks, by its style or tag. */
fn monolithic_node(node: &Node) -> bool {
    use crate::document::style::Overflow;
    let style = &node.style;
    let scrolls = !matches!(style.overflow, Overflow::Visible | Overflow::Clip);
    let held = style.multicol.avoid_inside || style.multicol.contain_size;
    MONOLITHIC.contains(&node.tag.as_str()) || style.inline_block || scrolls || held
}

/** For each item, the top of the outermost monolithic box inside the container that holds it
(None when none does): such content moves whole with that box and adds no break points. */
pub(super) fn anchors(
    context: &LayoutContext<'_>,
    container: &Node,
    items: &[DisplayItem],
) -> Vec<Option<f64>> {
    let mut tops = std::collections::HashMap::new();
    for item in items.iter().filter(|item| !item.tag.starts_with("::")) {
        tops.entry(item.local_id).or_insert(item.y);
    }
    let anchor = |item: &DisplayItem| {
        let node = context.node(item.local_id).ok()?;
        if super::flow::out_of_flow(node) {
            // Its containing block may sit above the monolithic box; it moves on its own.
            return None;
        }
        let mut held = None;
        let mut current = node.parent;
        while let Some(local) = current.filter(|local| *local != container.local_id) {
            let node = context.node(local).ok()?;
            if monolithic_node(node) {
                held = tops.get(&local).copied().or(held);
            }
            current = node.parent;
        }
        held
    };
    items.iter().map(anchor).collect()
}

/** Forced break positions: before a `break-before: column` box and after a `break-after`
one, ascending. */
pub(super) fn forced(context: &LayoutContext<'_>, items: &[DisplayItem]) -> Vec<f64> {
    let mut breaks = Vec::new();
    for item in items.iter().filter(|item| !item.tag.starts_with("::")) {
        let Ok(node) = context.node(item.local_id) else {
            continue;
        };
        let multicol = &node.style.multicol;
        if multicol.break_before {
            breaks.push(item.y - node.style.margin[0]);
        }
        if multicol.break_after {
            breaks.push(item.y + item.height + node.style.margin[2]);
        }
    }
    breaks.sort_by(f64::total_cmp);
    breaks
}
