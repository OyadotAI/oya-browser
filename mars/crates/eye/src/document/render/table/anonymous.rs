/** CSS 2.1 17.2.1's anonymous table objects (Blink's LayoutTable::CreateAnonymousWithParent and
 * its row and cell siblings): a run of table parts whose parent is not the box they need gets an
 * anonymous table (inline-table in an inline parent), a run of cells outside a row an anonymous
 * row, and a run of non-cells inside a row one anonymous cell. Eye builds no box tree, so the
 * wrappers are inserted as nodes into a copy of the document that only layout sees; their items
 * are dropped afterwards, since an anonymous box paints nothing of its own.
 *
 * ponytail: a parent with a `display: contents` child, a flex or grid container, and a row's or
 * table's own leading text are left as they are. */
use super::super::flow::{Document, Node};
use crate::document::style::TableBox;
use std::collections::{BTreeMap, HashMap, HashSet};

/** What a box is to the table fixup. */
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Table,
    Group,
    Row,
    Cell,
    Column,
    Caption,
    Other,
}
/** A child after the fixup: a document node by index, or a new anonymous box around some. */
enum Child {
    Old(usize),
    New(Wrapper),
}
/** An anonymous box: its kind, whether it is an inline-table, and what it wraps. */
struct Wrapper {
    kind: Kind,
    inline: bool,
    children: Vec<Child>,
}

/** The document with its anonymous table boxes inserted, and the first identity they take (every
 * anonymous box's is at least it), or None when no table part is misparented. */
pub(in super::super) fn fixup(document: &Document) -> Option<(Document, usize)> {
    if document.nodes.iter().all(|node| kind(node) == Kind::Other) {
        return None;
    }
    let mut kids: BTreeMap<Option<usize>, Vec<usize>> = BTreeMap::new();
    for (index, node) in document.nodes.iter().enumerate() {
        kids.entry(node.parent).or_default().push(index);
    }
    let positions = super::super::sizing::positions(document);
    let mut plans = Vec::new();
    for (parent, children) in &kids {
        let at = |local: usize| positions.get(local).copied().flatten();
        let owner = parent.and_then(at).map(|index| &document.nodes[index]);
        if let Some(arranged) = arrange_parent(document, owner, children) {
            plans.push((*parent, arranged));
        }
    }
    if plans.is_empty() {
        return None;
    }
    let first = document.nodes.iter().map(|node| node.local_id).max()? + 1;
    Some((emit(document, plans, first), first))
}

/** A parent's children rearranged, or None when nothing changes or the parent is skipped. */
fn arrange_parent(
    document: &Document,
    owner: Option<&Node>,
    children: &[usize],
) -> Option<Vec<Child>> {
    let nodes = &document.nodes;
    let skipped = children.iter().any(|&i| nodes[i].style.display_contents)
        || owner.is_some_and(|p| p.style.display_flex || p.style.display_grid)
        || owner.is_none()
            && (document.formatting.body.style.display_flex
                || document.formatting.body.style.display_grid);
    // A table's own leading text has no node to wrap, so such a table stays as it is.
    let texted = owner.is_some_and(|p| kind(p) == Kind::Table && !p.text.trim().is_empty());
    if skipped || texted {
        return None;
    }
    let (own, inline) = match owner {
        Some(node) => (kind(node), inline_box(node)),
        // Top-level nodes are the body's children; a `display: table` body holds its parts.
        None if document.formatting.body.style.table_box == TableBox::Table => (Kind::Table, false),
        None => (Kind::Other, false),
    };
    let arranged = arrange(
        nodes,
        own,
        inline,
        children.iter().map(|&i| Child::Old(i)).collect(),
    );
    arranged
        .iter()
        .any(|child| matches!(child, Child::New(_)))
        .then_some(arranged)
}
/** One parent's children with the wrappers its kind needs, nested wrappers arranged too. */
fn arrange(nodes: &[Node], own: Kind, inline: bool, children: Vec<Child>) -> Vec<Child> {
    match own {
        // A column in a row takes no slot (the grid skips it), so it is left unwrapped.
        Kind::Row => wrap(
            nodes,
            children,
            |k| !matches!(k, Kind::Cell | Kind::Column),
            (Kind::Cell, false),
        ),
        Kind::Group => wrap(nodes, children, |k| !proper_row(k), (Kind::Row, false)),
        Kind::Table => wrap(
            nodes,
            children,
            |k| !matches!(k, Kind::Row | Kind::Group | Kind::Column | Kind::Caption),
            (Kind::Row, false),
        ),
        // A column group holds columns, which take no boxes.
        Kind::Column => children,
        _ => wrap(nodes, children, part, (Kind::Table, inline)),
    }
}
/** A table part a table box must hold: anything but a table itself or a non-table box. */
fn part(kind: Kind) -> bool {
    !matches!(kind, Kind::Table | Kind::Other)
}
/** A row group takes rows only. */
fn proper_row(kind: Kind) -> bool {
    kind == Kind::Row
}
/** Wraps each run of children `runs` accepts in one new box of `made`; a hidden or out-of-flow
 * child joins an open run rather than ending it. */
fn wrap(
    nodes: &[Node],
    children: Vec<Child>,
    runs: impl Fn(Kind) -> bool,
    made: (Kind, bool),
) -> Vec<Child> {
    let mut out = Vec::new();
    let mut run = Vec::new();
    for child in children {
        match child_kind(nodes, &child) {
            None if run.is_empty() => out.push(child),
            None => run.push(child),
            Some(k) if runs(k) => run.push(child),
            Some(_) => {
                close(nodes, &mut run, &mut out, made);
                out.push(child);
            }
        }
    }
    close(nodes, &mut run, &mut out, made);
    out
}
/** Ends a run: its members, arranged for the new box's kind, inside one new box. */
fn close(nodes: &[Node], run: &mut Vec<Child>, out: &mut Vec<Child>, (kind, inline): (Kind, bool)) {
    let members = std::mem::take(run);
    if members
        .iter()
        .all(|child| child_kind(nodes, child).is_none())
        || (kind == Kind::Cell && lone_element(nodes, &members))
    {
        out.extend(members);
        return;
    }
    let children = arrange(nodes, kind, false, members);
    out.push(Child::New(Wrapper {
        kind,
        inline,
        children,
    }));
}
/**
 * Whether a run holds one in-flow, non-table element and nothing else that counts. The table lays such a child
 * out as the cell itself (table.rs `is_cell`), which also gives a percentage height inside it
 * the row's height to resolve against; an anonymous cell around it would leave that height
 * indefinite.
 *
 * ponytail: the element paints as the cell; an anonymous cell plus percentage heights
 * resolved against a cell's final height would replace this.
 */
fn lone_element(nodes: &[Node], members: &[Child]) -> bool {
    let counted: Vec<&Child> = members
        .iter()
        .filter(|child| child_kind(nodes, child).is_some())
        .collect();
    let plain = |node: &Node| {
        node.tag != "#text"
            && kind(node) == Kind::Other
            && node.style.float == crate::document::style::Float::None
    };
    matches!(counted.as_slice(), [Child::Old(index)] if plain(&nodes[*index]))
}
/** A child's kind; None for a hidden, out-of-flow or collapsible white-space-only one, which
 * neither starts nor breaks a run (CSS 2.1 17.2.1 drops such white space between parts). */
fn child_kind(nodes: &[Node], child: &Child) -> Option<Kind> {
    match child {
        Child::New(wrapper) => Some(wrapper.kind),
        Child::Old(index) => {
            let node = &nodes[*index];
            let neutral = node.removed()
                || super::super::flow::out_of_flow(node)
                || (super::super::flow::blank_text(node) && collapses(node));
            (!neutral).then(|| kind(node))
        }
    }
}
/** Whether a node's white space collapses, so white space alone is no content. */
fn collapses(node: &Node) -> bool {
    use crate::document::WhiteSpace;
    !matches!(
        node.style.white_space,
        WhiteSpace::Pre | WhiteSpace::PreWrap
    ) && !node.style.break_spaces
}
/** A node's table kind, by tag for HTML's table elements and by `display` otherwise. */
fn kind(node: &Node) -> Kind {
    match (node.tag.as_str(), node.style.table_box) {
        ("table", _) | (_, TableBox::Table) => Kind::Table,
        ("td" | "th", _) | (_, TableBox::Cell) => Kind::Cell,
        ("tr", _) | (_, TableBox::Row) => Kind::Row,
        ("thead" | "tbody" | "tfoot", _)
        | (_, TableBox::RowGroup | TableBox::HeaderGroup | TableBox::FooterGroup) => Kind::Group,
        ("col" | "colgroup", _) | (_, TableBox::Column) => Kind::Column,
        ("caption", _) | (_, TableBox::Caption) => Kind::Caption,
        _ => Kind::Other,
    }
}
/** An inline box, whose table parts get an inline-table. */
fn inline_box(node: &Node) -> bool {
    node.style.inline && !node.style.inline_block && !node.style.display_contents
}

/** The copy: each wrapper node right before the first node it holds, members reparented, and the
 * parts an inline-table now holds back to block level. */
fn emit(document: &Document, plans: Vec<(Option<usize>, Vec<Child>)>, first: usize) -> Document {
    let mut out = Emitted {
        next: first,
        before: HashMap::new(),
        parents: HashMap::new(),
        blocks: HashSet::new(),
        pending: Vec::new(),
    };
    let styles: HashMap<usize, &crate::document::ComputedStyle> = document
        .nodes
        .iter()
        .map(|n| (n.local_id, &n.style))
        .collect();
    for (parent, children) in plans {
        let style = parent
            .and_then(|local| styles.get(&local).copied())
            .unwrap_or(&document.formatting.body.style);
        out.children(document, children, parent, style);
    }
    let mut copy = document.clone();
    copy.nodes = Vec::with_capacity(document.nodes.len() + out.next - first);
    for (index, node) in document.nodes.iter().enumerate() {
        copy.nodes
            .extend(out.before.remove(&index).unwrap_or_default());
        let mut node = node.clone();
        if let Some(parent) = out.parents.get(&index) {
            node.parent = Some(*parent);
        }
        if out.blocks.contains(&index) {
            node.style.inline = false;
            node.style.inline_block = false;
        }
        copy.nodes.push(node);
    }
    copy
}
/** What emitting the wrappers collects. */
struct Emitted {
    /** The next anonymous identity. */
    next: usize,
    /** Wrapper nodes to insert before each document node, outermost first. */
    before: HashMap<usize, Vec<Node>>,
    /** New parents of wrapped nodes. */
    parents: HashMap<usize, usize>,
    /** Wrapped parts, which lay out as their table's parts, not inline. */
    blocks: HashSet<usize>,
    /** Wrappers opened since the last document node, waiting for their first member. */
    pending: Vec<Node>,
}
impl Emitted {
    /** Emits one parent's arranged children under `parent`, whose style `style` is. */
    fn children(
        &mut self,
        document: &Document,
        children: Vec<Child>,
        parent: Option<usize>,
        style: &crate::document::ComputedStyle,
    ) {
        for child in children {
            match child {
                Child::Old(index) => self.old(document, index, parent),
                Child::New(wrapper) => self.wrapper(document, wrapper, parent, style),
            }
        }
    }
    /** A document node: the pending wrappers go before it, and it moves under its new parent. */
    fn old(&mut self, document: &Document, index: usize, parent: Option<usize>) {
        if !self.pending.is_empty() {
            self.before.insert(index, std::mem::take(&mut self.pending));
        }
        let node = &document.nodes[index];
        if parent != node.parent
            && let Some(parent) = parent
        {
            self.parents.insert(index, parent);
            // A part an inline parent had made inline-level is now inside its own table.
            if !matches!(node.style.table_box, TableBox::None | TableBox::Table) {
                self.blocks.insert(index);
            }
        }
    }
    /** A wrapper: a node inheriting from its parent, then its own children under it. */
    fn wrapper(
        &mut self,
        document: &Document,
        wrapper: Wrapper,
        parent: Option<usize>,
        style: &crate::document::ComputedStyle,
    ) {
        let id = self.next;
        self.next += 1;
        let Ok(mut node) = crate::document::markup::node("div", id, parent, Default::default())
        else {
            return;
        };
        node.style = anonymous_style(style, &wrapper);
        // Source white space around the run stays around the box that now holds it.
        node.space_leading =
            edge_old(&wrapper, false).is_some_and(|i| document.nodes[i].space_leading);
        node.space_after = edge_old(&wrapper, true).is_some_and(|i| document.nodes[i].space_after);
        let own = node.style.clone();
        self.pending.push(node);
        self.children(document, wrapper.children, Some(id), &own);
    }
}
/** The first (or last) document node a wrapper holds, however deep. */
fn edge_old(wrapper: &Wrapper, last: bool) -> Option<usize> {
    let child = if last {
        wrapper.children.last()
    } else {
        wrapper.children.first()
    };
    match child? {
        Child::Old(index) => Some(*index),
        Child::New(inner) => edge_old(inner, last),
    }
}
/** An anonymous box's style: what it inherits from its parent, with its table display. */
fn anonymous_style(
    parent: &crate::document::ComputedStyle,
    wrapper: &Wrapper,
) -> crate::document::ComputedStyle {
    let mut style = crate::document::style::inherited_style(parent);
    style.table_box = match wrapper.kind {
        Kind::Row => TableBox::Row,
        Kind::Cell => TableBox::Cell,
        _ => TableBox::Table,
    };
    style.inline = wrapper.inline;
    style.inline_block = wrapper.inline;
    style
}
